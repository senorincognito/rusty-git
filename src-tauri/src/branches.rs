use git2::build::CheckoutBuilder;
use git2::{Branch, BranchType, ObjectType, Repository};
use serde::Serialize;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub name: String,
    /// The branch HEAD currently points at.
    pub is_head: bool,
    /// Upstream as "origin/main", if configured.
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// Local branches, sorted alphabetically (case-insensitive).
fn local_branches(repo: &Repository) -> Result<Vec<BranchInfo>, String> {
    let mut out = Vec::new();
    for item in repo.branches(Some(BranchType::Local)).map_err(err)? {
        let (branch, _) = item.map_err(err)?;
        let Some(name) = branch.name().ok().flatten().map(str::to_string) else { continue };

        let mut info = BranchInfo {
            name,
            is_head: branch.is_head(),
            upstream: None,
            ahead: 0,
            behind: 0,
        };
        if let Ok(up) = branch.upstream() {
            info.upstream = up.name().ok().flatten().map(str::to_string);
            if let (Some(l), Some(u)) = (branch.get().target(), up.get().target()) {
                if let Ok((ahead, behind)) = repo.graph_ahead_behind(l, u) {
                    info.ahead = ahead;
                    info.behind = behind;
                }
            }
        }
        out.push(info);
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(out)
}

#[tauri::command]
pub async fn get_local_branches(path: String) -> Result<Vec<BranchInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        local_branches(&Repository::discover(&path).map_err(err)?)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Creates `name` at the current commit and checks it out, like `git checkout -b`.
/// The working tree and index are untouched, so uncommitted changes carry over.
fn create_and_checkout(repo: &Repository, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a branch name".into());
    }
    if !Branch::name_is_valid(name).map_err(err)? || name.starts_with('-') {
        return Err(format!("\"{name}\" is not a valid branch name"));
    }
    let head = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|_| "Make a first commit before creating branches".to_string())?;

    match repo.branch(name, &head, false) {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            return Err(format!("A branch named \"{name}\" already exists"));
        }
        // e.g. "feature" when "feature/x" exists (or the reverse): refs can't nest like that.
        Err(e) if e.code() == git2::ErrorCode::Directory => {
            return Err(format!("\"{name}\" conflicts with an existing branch name"));
        }
        Err(e) => return Err(err(e)),
    }
    repo.set_head(&format!("refs/heads/{name}")).map_err(err)
}

#[tauri::command]
pub async fn create_branch(path: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        create_and_checkout(&Repository::discover(&path).map_err(err)?, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Switches to an existing local branch, like `git switch`. Safe: fails instead of
/// overwriting uncommitted changes that the target branch would touch.
fn checkout_branch(repo: &Repository, name: &str) -> Result<(), String> {
    let branch = repo
        .find_branch(name, BranchType::Local)
        .map_err(|_| format!("Branch \"{name}\" not found"))?;
    if branch.is_head() {
        return Ok(());
    }
    let refname = branch
        .get()
        .name()
        .map(str::to_string)
        .map_err(|_| "Branch name is not valid UTF-8".to_string())?;
    let target = branch.get().peel(ObjectType::Commit).map_err(err)?;

    // Update files first; HEAD moves only if that succeeded.
    let mut opts = CheckoutBuilder::new();
    opts.safe();
    repo.checkout_tree(&target, Some(&mut opts)).map_err(|e| {
        if e.code() == git2::ErrorCode::Conflict {
            "Your local changes would be overwritten by this checkout. Commit or discard them first."
                .to_string()
        } else {
            err(e)
        }
    })?;
    repo.set_head(&refname).map_err(err)
}

#[tauri::command]
pub async fn checkout_local_branch(path: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        checkout_branch(&Repository::discover(&path).map_err(err)?, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    #[test]
    fn lists_branches_alphabetically_and_flags_head() {
        let dir = std::env::temp_dir().join(format!("gc-branches-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();

        // No commits yet: no branches exist.
        assert!(local_branches(&repo).unwrap().is_empty());

        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let c = repo.commit(Some("refs/heads/main"), &sig, &sig, "c", &tree, &[]).unwrap();
        repo.set_head("refs/heads/main").unwrap();
        let commit = repo.find_commit(c).unwrap();
        for name in ["zeta", "Beta", "alpha", "feature/x"] {
            repo.branch(name, &commit, false).unwrap();
        }

        let list = local_branches(&repo).unwrap();
        let names: Vec<_> = list.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["alpha", "Beta", "feature/x", "main", "zeta"]);
        assert_eq!(list.iter().filter(|b| b.is_head).map(|b| b.name.as_str()).collect::<Vec<_>>(), ["main"]);
        assert!(list.iter().all(|b| b.upstream.is_none()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_and_checkout_branch() {
        let dir = std::env::temp_dir().join(format!("gc-newbranch-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();

        // Unborn branch: nothing to branch from.
        assert!(create_and_checkout(&repo, "x").unwrap_err().contains("first commit"));

        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let c = repo.commit(Some("refs/heads/main"), &sig, &sig, "c", &tree, &[]).unwrap();
        repo.set_head("refs/heads/main").unwrap();

        assert!(create_and_checkout(&repo, "  ").is_err());
        for bad in ["a b", "a..b", "-x", "x.lock", "HEAD", "a~1", "/x"] {
            assert!(create_and_checkout(&repo, bad).is_err(), "{bad} should be rejected");
        }
        assert!(create_and_checkout(&repo, "main").unwrap_err().contains("already exists"));

        // Uncommitted work survives the checkout.
        std::fs::write(dir.join("wip.txt"), "work").unwrap();
        create_and_checkout(&repo, " feature/login ").unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Ok("feature/login"));
        assert_eq!(repo.head().unwrap().target(), Some(c));
        assert_eq!(std::fs::read_to_string(dir.join("wip.txt")).unwrap(), "work");

        // "feature" would collide with the existing "feature/login" ref directory.
        assert!(create_and_checkout(&repo, "feature").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn commit_files(repo: &Repository, refname: &str, parents: &[git2::Oid], files: &[(&str, &str)]) -> git2::Oid {
        let sig = Signature::now("t", "t@example.com").unwrap();
        let mut tb = repo.treebuilder(None).unwrap();
        for (name, content) in files {
            tb.insert(name, repo.blob(content.as_bytes()).unwrap(), 0o100644).unwrap();
        }
        let tree = repo.find_tree(tb.write().unwrap()).unwrap();
        let ps: Vec<_> = parents.iter().map(|o| repo.find_commit(*o).unwrap()).collect();
        let refs: Vec<_> = ps.iter().collect();
        repo.commit(Some(refname), &sig, &sig, "c", &tree, &refs).unwrap()
    }

    #[test]
    fn checkout_switches_branches_safely() {
        let dir = std::env::temp_dir().join(format!("gc-checkout-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).ok();

        let c1 = commit_files(&repo, "refs/heads/main", &[], &[("a.txt", "one")]);
        repo.set_head("refs/heads/main").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force())).unwrap();
        commit_files(&repo, "refs/heads/feature", &[c1], &[("a.txt", "two"), ("c.txt", "x")]);

        assert!(checkout_branch(&repo, "nope").unwrap_err().contains("not found"));
        checkout_branch(&repo, "main").unwrap(); // already current: no-op

        checkout_branch(&repo, "feature").unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Ok("feature"));
        assert_eq!((read("a.txt").as_deref(), read("c.txt").as_deref()), (Some("two"), Some("x")));

        checkout_branch(&repo, "main").unwrap();
        assert_eq!(repo.head().unwrap().shorthand(), Ok("main"));
        assert_eq!((read("a.txt").as_deref(), read("c.txt")), (Some("one"), None));

        // A change to a file the other branch also changes blocks the switch.
        std::fs::write(dir.join("a.txt"), "local edit").unwrap();
        let e = checkout_branch(&repo, "feature").unwrap_err();
        assert!(e.contains("overwritten"), "{e}");
        assert_eq!(repo.head().unwrap().shorthand(), Ok("main"));
        assert_eq!(read("a.txt").as_deref(), Some("local edit"));

        // Unrelated untracked files simply come along.
        std::fs::write(dir.join("a.txt"), "one").unwrap();
        std::fs::write(dir.join("notes.txt"), "mine").unwrap();
        checkout_branch(&repo, "feature").unwrap();
        assert_eq!(read("notes.txt").as_deref(), Some("mine"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
