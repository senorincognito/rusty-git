use git2::{BranchType, Repository};
use serde::Serialize;

const ORIGIN: &str = "origin";

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    /// Remote-tracking branches without the "origin/" prefix, sorted case-insensitively.
    pub branches: Vec<String>,
    /// The remote branch (without "origin/") that the checked-out local branch tracks, if any.
    pub tracked_by_head: Option<String>,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// The `origin` remote, or None if the repo doesn't have one.
fn origin_info(repo: &Repository) -> Result<Option<RemoteInfo>, String> {
    let remote = match repo.find_remote(ORIGIN) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let url = remote.url().unwrap_or("").to_string();

    let prefix = format!("{ORIGIN}/");
    let mut branches = Vec::new();
    for item in repo.branches(Some(BranchType::Remote)).map_err(err)? {
        let (branch, _) = item.map_err(err)?;
        let Some(name) = branch.name().ok().flatten() else { continue };
        if let Some(short) = name.strip_prefix(&prefix) {
            if short != "HEAD" {
                branches.push(short.to_string());
            }
        }
    }
    branches.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));

    Ok(Some(RemoteInfo { name: ORIGIN.into(), url, branches, tracked_by_head: head_upstream(repo) }))
}

/// The origin branch the checked-out local branch tracks ("main" for "origin/main").
fn head_upstream(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let local = repo.find_branch(head.shorthand().ok()?, BranchType::Local).ok()?;
    let upstream = local.upstream().ok()?;
    let name = upstream.name().ok().flatten()?;
    name.strip_prefix(&format!("{ORIGIN}/")).map(str::to_string)
}

/// Commits that exist only on `origin/<name>`: not in HEAD and not in any local branch.
fn unmerged_remote_commits(repo: &Repository, name: &str) -> Result<usize, String> {
    let full = format!("refs/remotes/{ORIGIN}/{name}");
    let tip = repo
        .find_reference(&full)
        .and_then(|r| r.peel_to_commit())
        .map_err(|_| format!("Remote branch \"{ORIGIN}/{name}\" not found"))?
        .id();

    let mut walk = repo.revwalk().map_err(err)?;
    walk.push(tip).map_err(err)?;
    if let Ok(head) = repo.head().and_then(|h| h.peel_to_commit()) {
        walk.hide(head.id()).map_err(err)?;
    }
    for b in repo.branches(Some(BranchType::Local)).map_err(err)?.flatten() {
        if let Some(oid) = b.0.get().target() {
            walk.hide(oid).map_err(err)?;
        }
    }
    Ok(walk.count())
}

/// Removes `origin/<name>` on the server (`git push origin --delete`) with the system git, so
/// the user's credentials apply. git also drops the local remote-tracking ref.
fn delete_remote_branch_checked(repo_path: &str, repo: &Repository, name: &str) -> Result<String, String> {
    let info = origin_info(repo)?.ok_or("No origin remote configured")?;
    if !info.branches.iter().any(|b| b == name) {
        return Err(format!("Remote branch \"{ORIGIN}/{name}\" not found. Fetch and try again."));
    }
    if info.tracked_by_head.as_deref() == Some(name) {
        return Err("This is the upstream of the checked-out branch. Switch branches first.".into());
    }
    crate::sync::run_git(repo_path, &["push", ORIGIN, "--delete", name])
}

fn add_origin(repo: &Repository, url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter the repository URL".into());
    }
    if url.chars().any(char::is_whitespace) {
        return Err("The URL must not contain spaces".into());
    }
    match repo.remote(ORIGIN, url) {
        Ok(_) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            Err("A remote named origin already exists".into())
        }
        Err(e) => Err(err(e)),
    }
}

async fn blocking<T: Send + 'static>(
    path: String,
    f: impl FnOnce(&Repository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&Repository::discover(&path).map_err(err)?))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_origin(path: String) -> Result<Option<RemoteInfo>, String> {
    blocking(path, origin_info).await
}

/// Number of commits only reachable through `origin/<name>` (0 when it holds nothing unique).
#[tauri::command]
pub async fn count_unmerged_remote_commits(path: String, name: String) -> Result<usize, String> {
    blocking(path, move |r| unmerged_remote_commits(r, &name)).await
}

/// Deletes a branch on the origin server. Network call; may take a moment.
#[tauri::command]
pub async fn delete_remote_branch(path: String, name: String) -> Result<String, String> {
    let p = path.clone();
    blocking(path, move |r| delete_remote_branch_checked(&p, r, &name)).await
}

#[tauri::command]
pub async fn add_origin_remote(path: String, url: String) -> Result<(), String> {
    blocking(path, move |r| add_origin(r, &url)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    #[test]
    fn add_and_describe_origin() {
        let dir = std::env::temp_dir().join(format!("gc-remotes-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();

        assert!(origin_info(&repo).unwrap().is_none());

        assert!(add_origin(&repo, "   ").is_err());
        assert!(add_origin(&repo, "https://example.com/a b.git").is_err());
        add_origin(&repo, "  https://example.com/a.git ").unwrap();
        assert_eq!(
            add_origin(&repo, "https://example.com/other.git").unwrap_err(),
            "A remote named origin already exists"
        );

        let info = origin_info(&repo).unwrap().unwrap();
        assert_eq!(info.url, "https://example.com/a.git");
        assert!(info.branches.is_empty());

        // Remote-tracking refs: sorted, HEAD hidden, other remotes ignored.
        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let c = repo.commit(Some("refs/heads/main"), &sig, &sig, "c", &tree, &[]).unwrap();
        for r in [
            "refs/remotes/origin/main",
            "refs/remotes/origin/Beta",
            "refs/remotes/origin/alpha",
            "refs/remotes/origin/HEAD",
            "refs/remotes/upstream/zzz",
        ] {
            repo.reference(r, c, true, "test").unwrap();
        }
        let info = origin_info(&repo).unwrap().unwrap();
        assert_eq!(info.branches, ["alpha", "Beta", "main"]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn git(dir: &std::path::Path, args: &[&str]) -> String {
        crate::sync::run_git(dir.to_str().unwrap(), args)
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"))
    }

    #[test]
    fn delete_remote_branch_flow() {
        let base = std::env::temp_dir().join(format!("gc-delremote-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let origin = base.join("origin.git");
        let a = base.join("a");
        std::fs::create_dir_all(&a).unwrap();
        git(&base, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        git(&a, &["init", "-q", "-b", "main"]);
        git(&a, &["remote", "add", "origin", origin.to_str().unwrap()]);
        let commit = |msg: &str| {
            std::fs::write(a.join("f.txt"), msg).unwrap();
            git(&a, &["add", "-A"]);
            git(&a, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "-m", msg]);
        };
        commit("one");
        git(&a, &["push", "-q", "-u", "origin", "main"]);
        git(&a, &["checkout", "-q", "-b", "feature"]);
        commit("two");
        git(&a, &["push", "-q", "origin", "feature"]);
        git(&a, &["checkout", "-q", "main"]);

        let repo = Repository::open(&a).unwrap();
        let ap = a.to_str().unwrap();
        let info = origin_info(&repo).unwrap().unwrap();
        assert_eq!(info.branches, ["feature", "main"]);
        assert_eq!(info.tracked_by_head.as_deref(), Some("main"));

        // The work also lives in the local "feature" branch, so nothing would be lost.
        assert_eq!(unmerged_remote_commits(&repo, "feature").unwrap(), 0);
        git(&a, &["branch", "-q", "-D", "feature"]);
        assert_eq!(unmerged_remote_commits(&repo, "feature").unwrap(), 1);
        assert!(unmerged_remote_commits(&repo, "nope").is_err());

        // The upstream of the checked-out branch is protected; unknown branches are rejected.
        let e = delete_remote_branch_checked(ap, &repo, "main").unwrap_err();
        assert!(e.contains("upstream"), "{e}");
        assert!(delete_remote_branch_checked(ap, &repo, "nope").unwrap_err().contains("not found"));

        delete_remote_branch_checked(ap, &repo, "feature").unwrap();
        let on_server = git(&a, &["ls-remote", "--heads", "origin"]);
        assert!(on_server.contains("refs/heads/main") && !on_server.contains("feature"), "{on_server}");
        let info = origin_info(&Repository::open(&a).unwrap()).unwrap().unwrap();
        assert_eq!(info.branches, ["main"]);

        let _ = std::fs::remove_dir_all(&base);
    }
}
