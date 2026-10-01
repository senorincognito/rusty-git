use git2::{BranchType, Remote, Repository};
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    /// Remote-tracking branches without the "<remote>/" prefix, sorted case-insensitively.
    pub branches: Vec<String>,
    /// The remote branch (without "<remote>/") that the checked-out local branch tracks, if any.
    pub tracked_by_head: Option<String>,
    /// The remote new branches are published to (see [`target_remote`]).
    pub is_target: bool,
    /// Local branches that track a branch of this remote.
    pub tracking_branches: usize,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// Every remote of the repository, sorted by name.
fn list_remotes(repo: &Repository) -> Result<Vec<RemoteInfo>, String> {
    let mut names: Vec<String> = repo.remotes().map_err(err)?.iter().flatten().flatten().map(str::to_string).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    names.iter().filter_map(|n| remote_info(repo, n).transpose()).collect()
}

/// The named remote, or None if the repo doesn't have it.
fn remote_info(repo: &Repository, name: &str) -> Result<Option<RemoteInfo>, String> {
    let remote = match repo.find_remote(name) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let url = remote.url().unwrap_or("").to_string();

    let prefix = format!("{name}/");
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

    let tracking_branches = tracking_count(repo, name);
    Ok(Some(RemoteInfo {
        name: name.into(),
        url,
        branches,
        tracked_by_head: head_upstream(repo, name),
        is_target: target_remote(repo).as_deref() == Some(name),
        tracking_branches,
    }))
}

const TARGET_KEY: &str = "rustygit.targetremote";

/// The remote that new branches are published to (`push --set-upstream`): the one the user chose,
/// else `origin`, else the first remote. None when the repository has no remotes.
pub(crate) fn target_remote(repo: &Repository) -> Option<String> {
    let names: Vec<String> = repo.remotes().ok()?.iter().flatten().flatten().map(str::to_string).collect();
    let chosen = repo.config().ok().and_then(|c| c.get_string(TARGET_KEY).ok());
    chosen
        .filter(|c| names.contains(c))
        .or_else(|| names.iter().find(|n| *n == "origin").cloned())
        .or_else(|| names.first().cloned())
}

/// How many local branches have an upstream on this remote.
fn tracking_count(repo: &Repository, remote: &str) -> usize {
    let Ok(branches) = repo.branches(Some(BranchType::Local)) else { return 0 };
    branches
        .flatten()
        .filter(|(b, _)| {
            b.get()
                .name()
                .ok()
                .and_then(|n| repo.branch_upstream_remote(n).ok())
                .and_then(|buf| buf.as_str().map(|s| s == remote).ok())
                .unwrap_or(false)
        })
        .count()
}

/// The branch of `remote` that the checked-out local branch tracks ("main" for "origin/main").
fn head_upstream(repo: &Repository, remote: &str) -> Option<String> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let local = repo.find_branch(head.shorthand().ok()?, BranchType::Local).ok()?;
    let upstream = local.upstream().ok()?;
    let name = upstream.name().ok().flatten()?;
    name.strip_prefix(&format!("{remote}/")).map(str::to_string)
}

/// Commits that exist only on `<remote>/<name>`: not in HEAD and not in any local branch.
fn unmerged_remote_commits(repo: &Repository, remote: &str, name: &str) -> Result<usize, String> {
    let full = format!("refs/remotes/{remote}/{name}");
    let tip = repo
        .find_reference(&full)
        .and_then(|r| r.peel_to_commit())
        .map_err(|_| format!("Remote branch \"{remote}/{name}\" not found"))?
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

/// Removes `<remote>/<name>` on the server (`git push origin --delete`) with the system git, so
/// the user's credentials apply. git also drops the local remote-tracking ref.
fn delete_remote_branch_checked(repo_path: &str, repo: &Repository, remote: &str, name: &str) -> Result<String, String> {
    let info = remote_info(repo, remote)?.ok_or_else(|| format!("No remote named \"{remote}\""))?;
    if !info.branches.iter().any(|b| b == name) {
        return Err(format!("Remote branch \"{remote}/{name}\" not found. Fetch and try again."));
    }
    if info.tracked_by_head.as_deref() == Some(name) {
        return Err("This is the upstream of the checked-out branch. Switch branches first.".into());
    }
    crate::sync::run_git(repo_path, &["push", remote, "--delete", name])
}

/// Renames `<remote>/<name>` on the server: pushes the same commit under the new name and deletes
/// the old one in a single atomic push. The old name is only deleted if it still points where
/// we last saw it, so commits somebody else pushed meanwhile are not thrown away. Local branches
/// that tracked the old name are pointed at the new one.
fn rename_remote_branch_checked(
    repo_path: &str,
    repo: &Repository,
    remote: &str,
    name: &str,
    new_name: &str,
) -> Result<String, String> {
    let new_name = crate::branches::validate_branch_name(new_name)?;
    let info = remote_info(repo, remote)?.ok_or_else(|| format!("No remote named \"{remote}\""))?;
    if !info.branches.iter().any(|b| b == name) {
        return Err(format!("Remote branch \"{remote}/{name}\" not found. Fetch and try again."));
    }
    if info.tracked_by_head.as_deref() == Some(name) {
        return Err("This is the upstream of the checked-out branch. Switch branches first.".into());
    }
    if new_name == name {
        return Err("The name is unchanged".into());
    }
    if info.branches.iter().any(|b| b == new_name) {
        return Err(format!("A branch named \"{remote}/{new_name}\" already exists"));
    }
    // "feature" and "feature/x" can't both exist as refs.
    if info.branches.iter().any(|b| b.starts_with(&format!("{new_name}/")) || new_name.starts_with(&format!("{b}/"))) {
        return Err(format!("\"{new_name}\" conflicts with an existing branch name on {remote}"));
    }

    let tip = repo
        .find_reference(&format!("refs/remotes/{remote}/{name}"))
        .and_then(|r| r.peel_to_commit())
        .map_err(err)?
        .id();
    let lease = format!("--force-with-lease=refs/heads/{name}:{tip}");
    let create = format!("{tip}:refs/heads/{new_name}");
    let delete = format!(":refs/heads/{name}");
    let output = crate::sync::run_git(repo_path, &["push", "--atomic", &lease, remote, &create, &delete]).or_else(|e| {
        // Only for servers that can't do atomic pushes (a rejected lease also mentions "atomic",
        // and must NOT be retried: that would create the new name without removing the old one).
        if e.to_lowercase().contains("does not support --atomic") {
            crate::sync::run_git(repo_path, &["push", &lease, remote, &create, &delete])
        } else {
            Err(e)
        }
    })?;

    // Local branches that tracked the old name now track the new one.
    let fresh = Repository::discover(repo_path).map_err(err)?;
    let mut cfg = fresh.config().map_err(err)?;
    let old_merge = format!("refs/heads/{name}");
    let mut repointed = 0;
    for b in fresh.branches(Some(BranchType::Local)).map_err(err)?.flatten() {
        let Some(local) = b.0.name().ok().flatten() else { continue };
        let (remote_key, merge_key) = (format!("branch.{local}.remote"), format!("branch.{local}.merge"));
        if cfg.get_string(&remote_key).ok().as_deref() == Some(remote)
            && cfg.get_string(&merge_key).ok().as_deref() == Some(old_merge.as_str())
        {
            cfg.set_str(&merge_key, &format!("refs/heads/{new_name}")).map_err(err)?;
            repointed += 1;
        }
    }
    Ok(if repointed > 0 {
        format!("{output}\n{repointed} local branch(es) now track {remote}/{new_name}.")
    } else {
        output
    })
}

fn clean_url(url: &str) -> Result<&str, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter the repository URL".into());
    }
    if url.chars().any(char::is_whitespace) {
        return Err("The URL must not contain spaces".into());
    }
    Ok(url)
}

/// Points a remote at a different URL. Remote-tracking branches are kept; the next fetch
/// updates them from the new location.
fn set_remote_url(repo: &Repository, name: &str, url: &str) -> Result<(), String> {
    let url = clean_url(url)?;
    let remote = repo.find_remote(name).map_err(|_| format!("No remote named \"{name}\""))?;
    if remote.url().ok() == Some(url) {
        return Err("The URL is unchanged".into());
    }
    repo.remote_set_url(name, url).map_err(err)
}

fn add_remote(repo: &Repository, name: &str, url: &str) -> Result<(), String> {
    let name = name.trim();
    let url = clean_url(url)?;
    if name.is_empty() {
        return Err("Enter a name for the remote".into());
    }
    if !Remote::is_valid_name(name) {
        return Err(format!("\"{name}\" is not a valid remote name"));
    }
    match repo.remote(name, url) {
        Ok(_) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Exists => Err(format!("A remote named \"{name}\" already exists")),
        Err(e) => Err(err(e)),
    }
}

/// Removes a remote from the configuration, together with its remote-tracking branches. Local
/// branches that tracked it stay but lose their upstream. Nothing on the server is touched.
fn delete_remote(repo: &Repository, name: &str) -> Result<(), String> {
    repo.find_remote(name).map_err(|_| format!("No remote named \"{name}\""))?;
    let was_target = target_remote(repo).as_deref() == Some(name);
    repo.remote_delete(name).map_err(err)?;
    // The saved choice would point at a remote that no longer exists.
    if was_target {
        if let Ok(mut cfg) = repo.config() {
            let _ = cfg.remove(TARGET_KEY);
        }
    }
    Ok(())
}

fn set_target(repo: &Repository, name: &str) -> Result<(), String> {
    repo.find_remote(name).map_err(|_| format!("No remote named \"{name}\""))?;
    repo.config().and_then(|mut c| c.set_str(TARGET_KEY, name)).map_err(err)
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
pub async fn get_remotes(path: String) -> Result<Vec<RemoteInfo>, String> {
    blocking(path, list_remotes).await
}

/// Number of commits only reachable through `<remote>/<name>` (0 when it holds nothing unique).
#[tauri::command]
pub async fn count_unmerged_remote_commits(path: String, remote: String, name: String) -> Result<usize, String> {
    blocking(path, move |r| unmerged_remote_commits(r, &remote, &name)).await
}

/// Deletes a branch on a remote server. Network call; may take a moment.
#[tauri::command]
pub async fn delete_remote_branch(path: String, remote: String, name: String) -> Result<String, String> {
    let p = path.clone();
    blocking(path, move |r| delete_remote_branch_checked(&p, r, &remote, &name)).await
}

/// Renames a branch on a remote server (see [`rename_remote_branch_checked`]). Network call.
#[tauri::command]
pub async fn rename_remote_branch(path: String, remote: String, name: String, new_name: String) -> Result<String, String> {
    let p = path.clone();
    blocking(path, move |r| rename_remote_branch_checked(&p, r, &remote, &name, &new_name)).await
}

#[tauri::command]
pub async fn add_remote_cmd(path: String, name: String, url: String) -> Result<(), String> {
    blocking(path, move |r| add_remote(r, &name, &url)).await
}

#[tauri::command]
pub async fn set_remote_url_cmd(path: String, name: String, url: String) -> Result<(), String> {
    blocking(path, move |r| set_remote_url(r, &name, &url)).await
}

#[tauri::command]
pub async fn delete_remote_cmd(path: String, name: String) -> Result<(), String> {
    blocking(path, move |r| delete_remote(r, &name)).await
}

#[tauri::command]
pub async fn set_target_remote(path: String, name: String) -> Result<(), String> {
    blocking(path, move |r| set_target(r, &name)).await
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

        assert!(remote_info(&repo, "origin").unwrap().is_none());

        assert!(add_remote(&repo, "origin", "   ").is_err());
        assert!(add_remote(&repo, "origin", "https://example.com/a b.git").is_err());
        add_remote(&repo, "origin", "  https://example.com/a.git ").unwrap();
        assert_eq!(
            add_remote(&repo, "origin", "https://example.com/other.git").unwrap_err(),
            "A remote named \"origin\" already exists"
        );

        let info = remote_info(&repo, "origin").unwrap().unwrap();
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
        let info = remote_info(&repo, "origin").unwrap().unwrap();
        assert_eq!(info.branches, ["alpha", "Beta", "main"]);

        // Changing the URL keeps the remote-tracking branches.
        assert!(set_remote_url(&repo, "origin", " ").is_err());
        assert!(set_remote_url(&repo, "origin", "a b").is_err());
        assert_eq!(set_remote_url(&repo, "origin", "https://example.com/a.git").unwrap_err(), "The URL is unchanged");
        set_remote_url(&repo, "origin", " git@example.com:me/b.git ").unwrap();
        let info = remote_info(&Repository::open(&dir).unwrap(), "origin").unwrap().unwrap();
        assert_eq!(info.url, "git@example.com:me/b.git");
        assert_eq!(info.branches, ["alpha", "Beta", "main"]);
        let empty = tempdir_repo("gc-seturl-none");
        assert!(set_remote_url(&empty, "origin", "https://example.com/x.git").unwrap_err().contains("No remote named"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn several_remotes_target_and_delete() {
        let dir = std::env::temp_dir().join(format!("gc-multiremote-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        assert!(list_remotes(&repo).unwrap().is_empty());
        assert_eq!(target_remote(&repo), None);

        // Names are validated and unique.
        assert!(add_remote(&repo, "", "https://example.com/a.git").unwrap_err().contains("name"));
        assert!(add_remote(&repo, "bad name", "https://example.com/a.git").unwrap_err().contains("not a valid"));
        assert!(add_remote(&repo, "x", "").is_err());
        add_remote(&repo, "upstream", "https://example.com/u.git").unwrap();
        assert_eq!(target_remote(&repo).as_deref(), Some("upstream"), "the only remote is the target");
        add_remote(&repo, " origin ", "https://example.com/o.git").unwrap();
        assert!(add_remote(&repo, "origin", "https://example.com/other.git").unwrap_err().contains("already exists"));

        // Without a choice, origin is the target; the choice is remembered; unknown names are refused.
        assert_eq!(target_remote(&repo).as_deref(), Some("origin"));
        assert!(set_target(&repo, "nope").unwrap_err().contains("No remote named"));
        set_target(&repo, "upstream").unwrap();
        let all = list_remotes(&repo).unwrap();
        assert_eq!(all.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["origin", "upstream"]);
        assert_eq!(all.iter().map(|r| r.is_target).collect::<Vec<_>>(), [false, true]);

        // Each remote only lists its own branches and the branches that track it.
        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let c = repo.commit(Some("refs/heads/main"), &sig, &sig, "c", &tree, &[]).unwrap();
        repo.set_head("refs/heads/main").unwrap();
        repo.reference("refs/remotes/origin/main", c, true, "t").unwrap();
        repo.reference("refs/remotes/upstream/dev", c, true, "t").unwrap();
        repo.find_branch("main", BranchType::Local).unwrap().set_upstream(Some("upstream/dev")).unwrap();
        let all = list_remotes(&repo).unwrap();
        assert_eq!(all[0].branches, ["main"]);
        assert_eq!(all[1].branches, ["dev"]);
        assert_eq!((all[0].tracking_branches, all[1].tracking_branches), (0, 1));
        assert_eq!((all[0].tracked_by_head.as_deref(), all[1].tracked_by_head.as_deref()), (None, Some("dev")));

        // Deleting the target forgets the choice; the tracking branch stays but loses its upstream.
        assert!(delete_remote(&repo, "nope").is_err());
        delete_remote(&repo, "upstream").unwrap();
        let all = list_remotes(&repo).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].is_target, "falls back to origin");
        assert!(repo.find_reference("refs/remotes/upstream/dev").is_err(), "its remote-tracking branches go too");
        let main = repo.find_branch("main", BranchType::Local).unwrap();
        assert!(main.upstream().is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempdir_repo(name: &str) -> Repository {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Repository::init(&dir).unwrap()
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
        let info = remote_info(&repo, "origin").unwrap().unwrap();
        assert_eq!(info.branches, ["feature", "main"]);
        assert_eq!(info.tracked_by_head.as_deref(), Some("main"));

        // The work also lives in the local "feature" branch, so nothing would be lost.
        assert_eq!(unmerged_remote_commits(&repo, "origin", "feature").unwrap(), 0);
        git(&a, &["branch", "-q", "-D", "feature"]);
        assert_eq!(unmerged_remote_commits(&repo, "origin", "feature").unwrap(), 1);
        assert!(unmerged_remote_commits(&repo, "origin", "nope").is_err());

        // The upstream of the checked-out branch is protected; unknown branches are rejected.
        let e = delete_remote_branch_checked(ap, &repo, "origin", "main").unwrap_err();
        assert!(e.contains("upstream"), "{e}");
        assert!(delete_remote_branch_checked(ap, &repo, "origin", "nope").unwrap_err().contains("not found"));

        delete_remote_branch_checked(ap, &repo, "origin", "feature").unwrap();
        let on_server = git(&a, &["ls-remote", "--heads", "origin"]);
        assert!(on_server.contains("refs/heads/main") && !on_server.contains("feature"), "{on_server}");
        let info = remote_info(&Repository::open(&a).unwrap(), "origin").unwrap().unwrap();
        assert_eq!(info.branches, ["main"]);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rename_remote_branch_flow() {
        let base = std::env::temp_dir().join(format!("gc-renameremote-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let origin = base.join("origin.git");
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        git(&base, &["init", "-q", "--bare", "-b", "main", origin.to_str().unwrap()]);
        git(&a, &["init", "-q", "-b", "main"]);
        git(&a, &["remote", "add", "origin", origin.to_str().unwrap()]);
        let commit = |dir: &std::path::Path, msg: &str| {
            std::fs::write(dir.join("f.txt"), msg).unwrap();
            git(dir, &["add", "-A"]);
            git(dir, &["-c", "user.name=T", "-c", "user.email=t@example.com", "commit", "-q", "-m", msg]);
        };
        commit(&a, "one");
        git(&a, &["push", "-q", "-u", "origin", "main"]);
        git(&a, &["checkout", "-q", "-b", "feature"]);
        commit(&a, "two");
        git(&a, &["push", "-q", "origin", "feature"]);
        git(&a, &["checkout", "-q", "main"]);
        git(&a, &["branch", "-q", "--track", "track", "origin/feature"]); // a local branch tracking it

        let ap = a.to_str().unwrap();
        let heads = || git(&a, &["ls-remote", "--heads", "origin"]);

        // Rules, checked before anything is pushed.
        let repo = Repository::open(&a).unwrap();
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "main", "primary").unwrap_err().contains("upstream"));
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "nope", "x").unwrap_err().contains("not found"));
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "feature", "main").unwrap_err().contains("already exists"));
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "feature", "feature").unwrap_err().contains("unchanged"));
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "feature", "bad name").unwrap_err().contains("not a valid"));
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "feature", "main/x").unwrap_err().contains("conflicts"));
        assert!(heads().contains("refs/heads/feature"));

        // Rename: new name on the server, old one gone, tracking branches follow.
        let msg = rename_remote_branch_checked(ap, &repo, "origin", "feature", "renamed").unwrap();
        assert!(msg.contains("1 local branch"), "{msg}");
        let on_server = heads();
        assert!(on_server.contains("refs/heads/renamed") && !on_server.contains("refs/heads/feature"), "{on_server}");
        let info = remote_info(&Repository::open(&a).unwrap(), "origin").unwrap().unwrap();
        assert_eq!(info.branches, ["main", "renamed"]);
        assert_eq!(git(&a, &["config", "--get", "branch.track.merge"]), "refs/heads/renamed");

        // Somebody else pushes to it; renaming from a stale view must not discard their commit.
        git(&base, &["clone", "-q", origin.to_str().unwrap(), b.to_str().unwrap()]);
        git(&b, &["checkout", "-q", "renamed"]);
        commit(&b, "theirs");
        git(&b, &["push", "-q"]);
        let theirs = git(&b, &["rev-parse", "HEAD"]);
        let repo = Repository::open(&a).unwrap();
        assert!(rename_remote_branch_checked(ap, &repo, "origin", "renamed", "again").is_err());
        let on_server = heads();
        assert!(on_server.contains(&theirs) && on_server.contains("refs/heads/renamed") && !on_server.contains("again"), "{on_server}");

        let _ = std::fs::remove_dir_all(&base);
    }
}
