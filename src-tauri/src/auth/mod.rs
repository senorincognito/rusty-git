//! Asks the user for credentials when `git` (or the `ssh` it starts) needs a password, token or passphrase.
//!
//! Git has no terminal here, so it would fail with "could not read Username". Instead it is given `GIT_ASKPASS` /
//! `SSH_ASKPASS`: a small script that runs this very executable as `--askpass "<prompt>"`. That helper process
//! connects to a loopback socket the app listens on, the app emits a `credentials-request` event, the UI shows a
//! dialog and `answer_credentials` hands the answer back; the helper prints it for git. Git's own credential helpers
//! (keychain, Credential Manager) still come first, and receive a working login to store afterwards.
//!
//! Only on unix (macOS): Windows ships Git Credential Manager, which has its own sign-in window.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const PORT_VAR: &str = "RUSTY_GIT_ASKPASS_PORT";
const TOKEN_VAR: &str = "RUSTY_GIT_ASKPASS_TOKEN";
/// How long a prompt waits for the user before git is told "no answer".
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Serialize, Clone)]
struct CredentialsRequest {
    id: u64,
    prompt: String,
}

struct Askpass {
    port: u16,
    token: String,
    /// The askpass script git runs; only made on unix.
    script: Option<std::path::PathBuf>,
}

/// Tells the UI that prompt `.1` (request id `.0`) needs an answer.
type Notify = Arc<dyn Fn(u64, &str) + Send + Sync>;

static ASKPASS: OnceLock<Askpass> = OnceLock::new();
static PENDING: LazyLock<Mutex<HashMap<u64, Sender<Option<String>>>>> = LazyLock::new(Default::default);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// An unguessable token without a random-number dependency: `RandomState` is seeded by the OS.
fn random_token() -> String {
    let state = std::collections::hash_map::RandomState::new();
    (0..2)
        .map(|i| {
            let mut h = state.build_hasher();
            h.write_u64(i);
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Called once at startup. If anything fails, git simply keeps failing without a prompt, as before.
pub fn start(app: AppHandle) {
    let _ = try_start(app);
}

fn try_start(app: AppHandle) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let token = random_token();
    #[cfg(unix)]
    let script = {
        // Not in the app data folder: its path has a space on macOS ("Application Support") and git
        // splits the askpass command at spaces.
        let script = std::env::temp_dir().join(format!("rusty-git-askpass-{}.sh", std::process::id()));
        let quoted = std::env::current_exe()?.to_string_lossy().replace('\'', "'\\''");
        std::fs::write(&script, format!("#!/bin/sh\nexec '{quoted}' --askpass \"$1\"\n"))?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700))?;
        Some(script)
    };
    #[cfg(not(unix))]
    let script = None;
    let _ = ASKPASS.set(Askpass { port, token: token.clone(), script });
    serve(
        listener,
        token,
        Arc::new(move |id, prompt| {
            let _ = app.emit("credentials-request", CredentialsRequest { id, prompt: prompt.to_string() });
        }),
    );
    Ok(())
}

/// Accepts helper connections: one line `token<TAB>prompt` in, one line `OK<TAB>answer` or `CANCEL` out.
fn serve(listener: TcpListener, token: String, notify: Notify) {
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (token, notify) = (token.clone(), notify.clone());
            std::thread::spawn(move || handle(stream, &token, &*notify));
        }
    });
}

fn handle(stream: TcpStream, token: &str, notify: &(dyn Fn(u64, &str) + Send + Sync)) {
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.trim_end_matches(['\r', '\n']).splitn(2, '\t');
    let reply = match (parts.next(), parts.next()) {
        (Some(t), Some(prompt)) if t == token => {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let (tx, rx) = channel();
            PENDING.lock().unwrap().insert(id, tx);
            notify(id, prompt);
            let answer = rx.recv_timeout(ANSWER_TIMEOUT).ok().flatten();
            PENDING.lock().unwrap().remove(&id);
            match answer {
                Some(a) => format!("OK\t{}\n", a.replace(['\r', '\n'], "")),
                None => "CANCEL\n".to_string(),
            }
        }
        _ => "CANCEL\n".to_string(),
    };
    let _ = (&stream).write_all(reply.as_bytes());
}

/// The UI's answer to a `credentials-request`; `None` = cancelled.
#[tauri::command]
pub fn answer_credentials(id: u64, value: Option<String>) {
    if let Some(tx) = PENDING.lock().unwrap().remove(&id) {
        let _ = tx.send(value);
    }
}

/// Lets `cmd` (a git process) ask for credentials through the app. A no-op where there is no script (Windows).
pub(crate) fn apply_env(cmd: &mut Command) {
    let Some(a) = ASKPASS.get() else { return };
    let Some(script) = &a.script else { return };
    cmd.env("GIT_ASKPASS", script)
        .env("SSH_ASKPASS", script)
        // ssh only uses SSH_ASKPASS without a terminal and DISPLAY when told to (OpenSSH 8.4+).
        .env("SSH_ASKPASS_REQUIRE", "force")
        .env(PORT_VAR, a.port.to_string())
        .env(TOKEN_VAR, &a.token);
}

/// Asks the app for the answer to `prompt`. `None` = cancelled or the app is unreachable.
fn ask(port: u16, token: &str, prompt: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let prompt = prompt.replace(['\r', '\n', '\t'], " ");
    stream.write_all(format!("{token}\t{prompt}\n").as_bytes()).ok()?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).ok()?;
    reply.trim_end_matches(['\r', '\n']).strip_prefix("OK\t").map(str::to_string)
}

/// Entry point of the helper process (`rusty-git-client --askpass "<prompt>"`). Prints the answer for git;
/// the exit code says whether there was one.
pub fn askpass_main() -> i32 {
    let prompt = std::env::args().nth(2).unwrap_or_default();
    let port = std::env::var(PORT_VAR).ok().and_then(|p| p.parse().ok());
    let token = std::env::var(TOKEN_VAR).ok();
    match (port, token) {
        (Some(port), Some(token)) => match ask(port, &token, &prompt) {
            Some(answer) => {
                println!("{answer}");
                0
            }
            None => 1,
        },
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server whose "UI" answers every prompt with `reply(prompt)`.
    fn server(reply: fn(&str) -> Option<String>) -> (u16, String) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let token = random_token();
        serve(
            listener,
            token.clone(),
            Arc::new(move |id, prompt| {
                let answer = reply(prompt);
                std::thread::spawn(move || answer_credentials(id, answer));
            }),
        );
        (port, token)
    }

    #[test]
    fn prompts_round_trip_through_the_app() {
        let (port, token) = server(|p| (p.starts_with("Username")).then(|| "me".to_string()));
        assert_eq!(ask(port, &token, "Username for 'https://example.com': ").as_deref(), Some("me"));
        // Cancelled in the UI.
        assert_eq!(ask(port, &token, "Password for 'https://me@example.com': "), None);
        // Wrong token: refused without ever reaching the UI.
        assert_eq!(ask(port, "nope", "Username for 'x': "), None);
    }

    #[test]
    fn tokens_differ() {
        assert_ne!(random_token(), random_token());
    }
}
