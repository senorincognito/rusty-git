use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

struct Session {
    id: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Session {
    fn stop(mut self) {
        let _ = self.child.kill();
        // Dropping the master closes the pseudo-terminal, which ends the reader thread.
    }
}

/// The one terminal session of the app (it follows the open repository).
#[derive(Default)]
pub struct Terminal(Mutex<Option<Session>>);

#[derive(Serialize, Clone)]
struct TermData {
    id: u64,
    data: String,
}

fn shell() -> CommandBuilder {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = CommandBuilder::new("powershell.exe");
        c.arg("-NoLogo");
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        // Login shell, so PATH from the user's profile is loaded even when launched from the Dock.
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut c = CommandBuilder::new(sh);
        c.arg("-l");
        c
    };
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd
}

fn spawn(
    cwd: &Path,
    cols: u16,
    rows: u16,
    mut cmd: CommandBuilder,
    id: u64,
) -> Result<(Session, Box<dyn Read + Send>), String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;
    cmd.cwd(cwd);
    let child = pair.slave.spawn_command(cmd).map_err(|e| format!("Could not start shell: {e}"))?;
    drop(pair.slave);
    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    Ok((Session { id, master: pair.master, writer, child }, reader))
}

/// Turns raw bytes into text chunks, holding back an incomplete UTF-8 sequence that was
/// split across two reads so multi-byte characters never get mangled.
struct Utf8Chunker {
    pending: Vec<u8>,
}

impl Utf8Chunker {
    fn new() -> Self {
        Self { pending: Vec::new() }
    }

    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(s) => {
                    out.push_str(s);
                    self.pending.clear();
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    out.push_str(std::str::from_utf8(&self.pending[..valid]).unwrap());
                    match e.error_len() {
                        // Genuinely invalid bytes: substitute and keep going.
                        Some(n) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..valid + n);
                        }
                        // Sequence cut off at the end of the buffer: wait for the rest.
                        None => {
                            self.pending.drain(..valid);
                            return out;
                        }
                    }
                }
            }
        }
    }
}

/// Starts (or restarts) the terminal in `path`. `id` is chosen by the frontend so it can
/// ignore output from a previous session.
#[tauri::command]
pub fn term_start(
    app: AppHandle,
    state: State<Terminal>,
    path: String,
    id: u64,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let (session, mut reader) = spawn(Path::new(&path), cols.max(2), rows.max(1), shell(), id)?;
    if let Some(old) = state.0.lock().unwrap().replace(session) {
        old.stop();
    }

    std::thread::spawn(move || {
        let mut chunker = Utf8Chunker::new();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let data = chunker.push(&buf[..n]);
                    if !data.is_empty() {
                        let _ = app.emit("term-data", TermData { id, data });
                    }
                }
            }
        }
        let _ = app.emit("term-exit", id);
    });
    Ok(())
}

#[tauri::command]
pub fn term_write(state: State<Terminal>, id: u64, data: String) -> Result<(), String> {
    match state.0.lock().unwrap().as_mut() {
        Some(s) if s.id == id => {
            s.writer.write_all(data.as_bytes()).and_then(|_| s.writer.flush()).map_err(|e| e.to_string())
        }
        _ => Ok(()), // stale session: drop the input
    }
}

#[tauri::command]
pub fn term_resize(state: State<Terminal>, id: u64, cols: u16, rows: u16) -> Result<(), String> {
    match state.0.lock().unwrap().as_ref() {
        Some(s) if s.id == id => s
            .master
            .resize(PtySize { rows: rows.max(1), cols: cols.max(2), pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string()),
        _ => Ok(()),
    }
}

#[tauri::command]
pub fn term_stop(state: State<Terminal>) {
    if let Some(s) = state.0.lock().unwrap().take() {
        s.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn chunker_keeps_split_characters_intact() {
        let mut c = Utf8Chunker::new();
        let euro = "€".as_bytes(); // 3 bytes
        assert_eq!(c.push(&euro[..1]), "");
        assert_eq!(c.push(&euro[1..2]), "");
        assert_eq!(c.push(&euro[2..]), "€");
        assert_eq!(c.push(b"ab\xFFcd"), "ab\u{FFFD}cd");
        assert_eq!(c.push("ä".as_bytes()), "ä");
    }

    #[test]
    fn shell_runs_in_the_given_directory_and_echoes_input() {
        let dir = std::env::temp_dir().join(format!("gc-term-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let (mut session, mut reader) = spawn(&dir, 100, 30, shell(), 1).unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });

        // Print the working directory name and a marker built from two halves, so the
        // marker only appears in the *output*, not in the echoed command line.
        let leaf = dir.file_name().unwrap().to_string_lossy().to_string();
        #[cfg(windows)]
        let cmd = "(Get-Location).Path; Write-Output ('gc-' + 'done')\r\n";
        #[cfg(not(windows))]
        let cmd = "pwd; echo gc-\"done\"\n";
        session.writer.write_all(cmd.as_bytes()).unwrap();
        session.writer.flush().unwrap();

        let mut seen = String::new();
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline && !seen.contains("gc-done") {
            if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
                let text = String::from_utf8_lossy(&chunk).to_string();
                // ConPTY asks the terminal for the cursor position and waits; a real terminal
                // (xterm.js) answers automatically, so the test must too.
                if text.contains("\x1b[6n") {
                    session.writer.write_all(b"\x1b[1;1R").unwrap();
                    session.writer.flush().unwrap();
                }
                seen.push_str(&text);
            }
        }
        session.stop();
        assert!(seen.contains("gc-done"), "no output marker; got: {seen:?}");
        assert!(seen.contains(&leaf), "cwd not used; got: {seen:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
