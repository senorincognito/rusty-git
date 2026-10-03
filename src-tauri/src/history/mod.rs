use std::collections::{HashMap, HashSet};

use git2::{BranchType, Commit, Oid, Repository, Sort};

pub mod drop;
pub mod rebase;
pub mod rename;
#[cfg(test)]
pub(crate) mod test_support;

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// True when `oid` is already on the upstream of the checked-out branch, i.e. rewriting it
/// would change published history.
pub(crate) fn is_pushed(repo: &Repository, oid: Oid) -> bool {
    let Ok(head) = repo.head() else { return false };
    if !head.is_branch() {
        return false;
    }
    let upstream_tip = head
        .shorthand()
        .ok()
        .and_then(|n| repo.find_branch(n, BranchType::Local).ok())
        .and_then(|b| b.upstream().ok())
        .and_then(|u| u.get().target());
    match upstream_tip {
        Some(up) => up == oid || repo.graph_descendant_of(up, oid).unwrap_or(false),
        None => false,
    }
}

fn head_oid(repo: &Repository) -> Option<Oid> {
    repo.head().ok()?.peel_to_commit().ok().map(|c| c.id())
}

/// `oid` is HEAD or one of its ancestors.
fn on_current_branch(repo: &Repository, oid: Oid) -> bool {
    match head_oid(repo) {
        Some(head) => head == oid || repo.graph_descendant_of(head, oid).unwrap_or(false),
        None => false,
    }
}

/// The commits that must be rebuilt to change `target`: the target itself plus every commit
/// on the way to HEAD that descends from it, parents before children.
fn rewrite_plan(repo: &Repository, target: &Commit) -> Result<Vec<Oid>, String> {
    let head = head_oid(repo).ok_or("There are no commits yet")?;
    let mut walk = repo.revwalk().map_err(err)?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE).map_err(err)?;
    walk.push(head).map_err(err)?;
    // Nothing that is only reachable through the target's parents can descend from it.
    for p in target.parent_ids() {
        walk.hide(p).map_err(err)?;
    }

    let mut affected: HashSet<Oid> = HashSet::new();
    let mut plan = Vec::new();
    for oid in walk {
        let oid = oid.map_err(err)?;
        let commit = repo.find_commit(oid).map_err(err)?;
        if oid == target.id() || commit.parent_ids().any(|p| affected.contains(&p)) {
            affected.insert(oid);
            plan.push(oid);
        }
    }
    Ok(plan)
}

/// Rebuilds `oldest` and every later commit on the way to HEAD with the same trees, authors and dates,
/// giving the commits in `edits` their new message (and the current user as committer, like
/// `--amend`), then moves the branch (or a detached HEAD) to the new tip. Files and index are
/// untouched because no tree changes. Returns old id -> new id for every rebuilt commit.
fn rebuild_with_messages(
    repo: &Repository,
    oldest: &Commit,
    edits: &HashMap<Oid, String>,
    note: &str,
) -> Result<HashMap<Oid, Oid>, String> {
    let old_head = head_oid(repo).ok_or("There are no commits yet")?;
    let mut rebuilt: HashMap<Oid, Oid> = HashMap::new();
    for oid in rewrite_plan(repo, oldest)? {
        let c = repo.find_commit(oid).map_err(err)?;
        let parents = c
            .parent_ids()
            .map(|p| repo.find_commit(*rebuilt.get(&p).unwrap_or(&p)).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let parent_refs: Vec<&Commit> = parents.iter().collect();

        let (text, committer) = match edits.get(&oid) {
            // The editor becomes the committer (falling back to the old one).
            Some(m) => (m.clone(), repo.signature().unwrap_or_else(|_| c.committer().to_owned())),
            None => (String::from_utf8_lossy(c.message_raw_bytes()).into_owned(), c.committer().to_owned()),
        };
        let new =
            repo.commit(None, &c.author(), &committer, &text, &c.tree().map_err(err)?, &parent_refs).map_err(err)?;
        rebuilt.insert(oid, new);
    }

    move_head_to(repo, rebuilt[&old_head], note)?;
    Ok(rebuilt)
}

/// Points the checked-out branch (or a detached HEAD) at `new_head`; the files and index stay as they are.
fn move_head_to(repo: &Repository, new_head: Oid, note: &str) -> Result<(), String> {
    let head = repo.head().map_err(err)?;
    match (head.is_branch(), head.name()) {
        (true, Ok(refname)) => {
            repo.reference(refname, new_head, true, note).map_err(err)?;
        }
        _ => repo.set_head_detached(new_head).map_err(err)?,
    }
    Ok(())
}

async fn blocking<T: Send + 'static>(
    path: String,
    f: impl FnOnce(&Repository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&Repository::discover(&path).map_err(err)?))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::test_support::make;
    use super::*;

    #[test]
    fn pushed_commits_are_detected() {
        let dir = std::env::temp_dir().join(format!("gc-pushed-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let a = make(&repo, "A", "a", &[], Some("refs/heads/main"));
        repo.set_head("refs/heads/main").unwrap();
        let b = make(&repo, "A", "b", &[a], Some("refs/heads/main"));
        assert!(!is_pushed(&repo, a)); // no upstream yet

        repo.remote("origin", "https://example.com/r.git").unwrap();
        repo.reference("refs/remotes/origin/main", a, true, "test").unwrap();
        repo.find_branch("main", BranchType::Local).unwrap().set_upstream(Some("origin/main")).unwrap();
        assert!(is_pushed(&repo, a));
        assert!(!is_pushed(&repo, b)); // local only

        let _ = std::fs::remove_dir_all(&dir);
    }
}
