use std::process::{Command, Stdio};

use git2::{BranchType, Repository};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Current local branch; None when HEAD is detached or unborn.
    pub branch: Option<String>,
    /// Upstream as "origin/main", if the branch tracks one.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub has_remote: bool,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

fn sync_status(repo: &Repository) -> Result<SyncStatus, String> {
    let has_remote = !repo.remotes().map_err(err)?.is_empty();
    let mut status = SyncStatus { branch: None, upstream: None, ahead: 0, behind: 0, has_remote };

    let Ok(head) = repo.head() else { return Ok(status) };
    if !head.is_branch() {
        return Ok(status);
    }
    let Ok(name) = head.shorthand().map(str::to_string) else { return Ok(status) };
    status.branch = Some(name.clone());

    if let Ok(local) = repo.find_branch(&name, BranchType::Local) {
        // No upstream configured -> find_upstream fails; that's a normal state.
        if let Ok(up) = local.upstream() {
            status.upstream = up.name().ok().flatten().map(str::to_string);
            if let (Some(l), Some(u)) = (head.target(), up.get().target()) {
                let (ahead, behind) = repo.graph_ahead_behind(l, u).map_err(err)?;
                status.ahead = ahead;
                status.behind = behind;
            }
        }
    }
    Ok(status)
}

/// Runs the system `git` so the user's credential helpers, SSH agent and config all apply.
pub(crate) fn run_git(path: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(path)
        .args(args)
        // Fail instead of waiting for a password on a terminal that doesn't exist.
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "git executable not found. Install Git and make sure it is on your PATH.".to_string()
        } else {
            e.to_string()
        }
    })?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    // git reports progress and most messages on stderr.
    let text = format!("{}\n{}", stdout.trim(), stderr.trim()).trim().to_string();
    if out.status.success() {
        Ok(text)
    } else if text.is_empty() {
        Err(format!("git {} failed ({})", args.first().unwrap_or(&""), out.status))
    } else {
        Err(text)
    }
}

fn open(path: &str) -> Result<Repository, String> {
    Repository::discover(path).map_err(err)
}

fn fetch(path: &str) -> Result<String, String> {
    if !sync_status(&open(path)?)?.has_remote {
        return Err("No remotes configured for this repository".into());
    }
    run_git(path, &["fetch", "--all", "--prune"])
}

fn pull(path: &str) -> Result<String, String> {
    let s = sync_status(&open(path)?)?;
    if s.branch.is_none() {
        return Err("Check out a branch before pulling".into());
    }
    if s.upstream.is_none() {
        return Err("The current branch has no upstream to pull from. Push it first.".into());
    }
    // Fast-forward only: never creates a surprise merge commit or leaves a conflicted tree.
    run_git(path, &["pull", "--ff-only"])
}

fn push(path: &str) -> Result<String, String> {
    let repo = open(path)?;
    let s = sync_status(&repo)?;
    if s.branch.is_none() {
        return Err("Cannot push a detached HEAD. Check out a branch first.".into());
    }
    if s.upstream.is_some() {
        return run_git(path, &["push"]);
    }
    // First push of a new branch: publish it and start tracking it.
    let remotes = repo.remotes().map_err(err)?;
    // StringArray items are Result<Option<&str>>; skip entries that aren't valid UTF-8.
    let names: Vec<&str> = remotes.iter().flatten().flatten().collect();
    let remote = names
        .iter()
        .copied()
        .find(|r| *r == "origin")
        .or_else(|| names.first().copied())
        .ok_or("No remotes configured for this repository")?;
    run_git(path, &["push", "--set-upstream", remote, "HEAD"])
}

/// Overwrites the upstream with the local branch, e.g. after an amend or rename. Uses
/// --force-with-lease, so it refuses if the remote branch moved since the last fetch (somebody
/// else's commits are never thrown away unseen).
fn force_push(path: &str) -> Result<String, String> {
    let s = sync_status(&open(path)?)?;
    if s.branch.is_none() {
        return Err("Cannot push a detached HEAD. Check out a branch first.".into());
    }
    if s.upstream.is_none() {
        return Err("This branch has not been pushed yet, so there is nothing to overwrite. Use Push instead.".into());
    }
    run_git(path, &["push", "--force-with-lease"])
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_sync_status(path: String) -> Result<SyncStatus, String> {
    blocking(move || sync_status(&open(&path)?)).await
}

#[tauri::command]
pub async fn git_fetch(path: String) -> Result<String, String> {
    blocking(move || fetch(&path)).await
}

#[tauri::command]
pub async fn git_pull(path: String) -> Result<String, String> {
    blocking(move || pull(&path)).await
}

#[tauri::command]
pub async fn git_push(path: String) -> Result<String, String> {
    blocking(move || push(&path)).await
}

#[tauri::command]
pub async fn git_force_push(path: String) -> Result<String, String> {
    blocking(move || force_push(&path)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("gc-sync-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        run_git(dir.to_str().unwrap(), args).unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }

    fn commit_file(dir: &Path, file: &str, msg: &str) {
        fs::write(dir.join(file), msg).unwrap();
        git(dir, &["add", "-A"]);
        git(
            dir,
            &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "-m", msg],
        );
    }

    #[test]
    fn fetch_pull_push_roundtrip() {
        let base = tmp("rt");
        let origin = base.join("origin.git");
        let a = base.join("a");
        let b = base.join("b");
        fs::create_dir_all(&a).unwrap();
        git(&base, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        git(&a, &["init", "-q", "-b", "main"]);
        git(&a, &["remote", "add", "origin", origin.to_str().unwrap()]);

        // No remote-less errors, and a repo without commits has no branch status.
        let (ap, bp) = (a.to_str().unwrap(), b.to_str().unwrap());
        assert!(pull(ap).is_err());

        commit_file(&a, "f.txt", "one");
        let s = sync_status(&open(ap).unwrap()).unwrap();
        assert_eq!((s.upstream.clone(), s.has_remote), (None, true));

        // First push publishes the branch and sets the upstream.
        push(ap).unwrap();
        let s = sync_status(&open(ap).unwrap()).unwrap();
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (0, 0));

        git(&base, &["clone", "-q", origin.to_str().unwrap(), bp]);

        // A pushes a second commit; B only learns about it after fetching.
        commit_file(&a, "f.txt", "two");
        push(ap).unwrap();
        let s = sync_status(&open(bp).unwrap()).unwrap();
        assert_eq!(s.behind, 0);
        fetch(bp).unwrap();
        let s = sync_status(&open(bp).unwrap()).unwrap();
        assert_eq!((s.ahead, s.behind), (0, 1));

        pull(bp).unwrap();
        let s = sync_status(&open(bp).unwrap()).unwrap();
        assert_eq!((s.ahead, s.behind), (0, 0));

        // B commits locally -> ahead 1 -> push brings origin level.
        commit_file(&b, "g.txt", "three");
        assert_eq!(sync_status(&open(bp).unwrap()).unwrap().ahead, 1);
        push(bp).unwrap();
        assert_eq!(sync_status(&open(bp).unwrap()).unwrap().ahead, 0);

        // A has diverged by committing without pulling: ff-only pull must refuse.
        commit_file(&a, "h.txt", "four");
        fetch(ap).unwrap();
        assert!(pull(ap).is_err());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn force_push_overwrites_but_respects_the_lease() {
        let base = tmp("force");
        let origin = base.join("origin.git");
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(&a).unwrap();
        git(&base, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        git(&a, &["init", "-q", "-b", "main"]);
        git(&a, &["remote", "add", "origin", origin.to_str().unwrap()]);
        let (ap, bp) = (a.to_str().unwrap(), b.to_str().unwrap());
        let amend = |dir: &Path, msg: &str| {
            git(dir, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "--amend", "-m", msg]);
        };
        let head = |dir: &Path| git(dir, &["rev-parse", "HEAD"]);
        let remote_main = || git(&base, &["--git-dir", origin.to_str().unwrap(), "rev-parse", "main"]);

        // Nothing to force before the branch has been published.
        commit_file(&a, "f.txt", "one");
        assert!(force_push(ap).unwrap_err().contains("not been pushed"));
        push(ap).unwrap();
        git(&base, &["clone", "-q", origin.to_str().unwrap(), bp]);

        // After an amend a normal push is rejected, a force push goes through.
        amend(&a, "one, amended");
        assert!(push(ap).is_err());
        force_push(ap).unwrap();
        assert_eq!(remote_main(), head(&a));

        // Somebody else pushes; our next force push must not clobber it unseen.
        git(&b, &["fetch", "-q"]);
        git(&b, &["reset", "-q", "--hard", "origin/main"]);
        commit_file(&b, "g.txt", "theirs");
        push(bp).unwrap();
        let theirs = remote_main();
        amend(&a, "one, amended again");
        assert!(force_push(ap).is_err());
        assert_eq!(remote_main(), theirs);

        // Once we have fetched and seen their commit, the overwrite is allowed.
        fetch(ap).unwrap();
        force_push(ap).unwrap();
        assert_eq!(remote_main(), head(&a));

        let _ = fs::remove_dir_all(&base);
    }
}
