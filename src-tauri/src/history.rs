use std::collections::{HashMap, HashSet};

use git2::{BranchType, Commit, Oid, Repository, RepositoryState, ResetType, Sort};
use serde::Serialize;

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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RenameInfo {
    /// Full current message, to pre-fill the editor.
    pub message: String,
    /// The commit is already on the upstream: renaming it needs a force push.
    pub pushed: bool,
    /// How many later commits will be rewritten along with it.
    pub later_commits: usize,
}

fn rename_info(repo: &Repository, id: &str) -> Result<RenameInfo, String> {
    let oid = Oid::from_str(id).map_err(err)?;
    let commit = repo.find_commit(oid).map_err(err)?;
    if !on_current_branch(repo, oid) {
        return Err("Only commits on the current branch can be renamed".into());
    }
    let plan = rewrite_plan(repo, &commit)?;
    Ok(RenameInfo {
        message: String::from_utf8_lossy(commit.message_raw_bytes()).trim_end().to_string(),
        pushed: is_pushed(repo, oid),
        later_commits: plan.len().saturating_sub(1),
    })
}

/// Changes the message of `id` and rebuilds every later commit on the current branch on top
/// of it (same trees, authors and timestamps), then moves the branch to the new tip, like a
/// `reword` in an interactive rebase. Files and index are untouched because no tree changes.
/// Returns the id of the renamed commit.
fn rename_commit(repo: &Repository, id: &str, message: &str) -> Result<Oid, String> {
    let message = git2::message_prettify(message, None).map_err(err)?;
    if message.trim().is_empty() {
        return Err("Commit message is empty".into());
    }
    if repo.state() != RepositoryState::Clean {
        return Err("Finish the merge, rebase or other operation in progress first".into());
    }
    let target_oid = Oid::from_str(id).map_err(err)?;
    let target = repo.find_commit(target_oid).map_err(err)?;
    if !on_current_branch(repo, target_oid) {
        return Err("Only commits on the current branch can be renamed".into());
    }
    if String::from_utf8_lossy(target.message_raw_bytes()).trim() == message.trim() {
        return Err("The message is unchanged".into());
    }

    let old_head = head_oid(repo).ok_or("There are no commits yet")?;
    let mut rebuilt: HashMap<Oid, Oid> = HashMap::new();
    for oid in rewrite_plan(repo, &target)? {
        let c = repo.find_commit(oid).map_err(err)?;
        let parents = c
            .parent_ids()
            .map(|p| repo.find_commit(*rebuilt.get(&p).unwrap_or(&p)).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let parent_refs: Vec<&Commit> = parents.iter().collect();

        let (text, committer) = if oid == target_oid {
            // Like --amend: the renamer becomes the committer (falling back to the old one).
            let who = repo.signature().unwrap_or_else(|_| c.committer().to_owned());
            (message.clone(), who)
        } else {
            (String::from_utf8_lossy(c.message_raw_bytes()).into_owned(), c.committer().to_owned())
        };
        let new = repo
            .commit(None, &c.author(), &committer, &text, &c.tree().map_err(err)?, &parent_refs)
            .map_err(err)?;
        rebuilt.insert(oid, new);
    }

    let new_head = rebuilt[&old_head];
    let head = repo.head().map_err(err)?;
    let note = format!("rename commit {}", &id[..7.min(id.len())]);
    match (head.is_branch(), head.name()) {
        (true, Ok(refname)) => {
            repo.reference(refname, new_head, true, &note).map_err(err)?;
        }
        _ => repo.set_head_detached(new_head).map_err(err)?,
    }
    Ok(rebuilt[&target_oid])
}

/// What dropping a commit involves, after checking that it is possible: the commit, its first
/// parent, and the commits after it (oldest first) that have to be re-created on top of that parent.
struct DropPlan<'r> {
    target: Commit<'r>,
    parent: Commit<'r>,
    later: Vec<Commit<'r>>,
}

/// Only commits on the branch's own line of history can be dropped, and only while the commits after
/// them form a straight line: replaying a merge commit is ambiguous, so it is not attempted.
fn drop_plan<'r>(repo: &'r Repository, id: &str) -> Result<DropPlan<'r>, String> {
    if repo.state() != RepositoryState::Clean {
        return Err("Finish the merge, rebase or other operation in progress first".into());
    }
    let head = repo.head().map_err(|_| "There are no commits yet".to_string())?;
    if !head.is_branch() {
        return Err("Check out a branch first: a detached HEAD has no branch to rewrite".into());
    }
    let target_id = Oid::from_str(id).map_err(err)?;
    let not_droppable = "Only commits on the current branch's own line of history, with no merge commit after them, \
                         can be dropped";

    let mut later: Vec<Commit<'r>> = Vec::new();
    let mut cursor = head.peel_to_commit().map_err(err)?;
    while cursor.id() != target_id {
        if cursor.parent_count() != 1 {
            return Err(not_droppable.into());
        }
        let parent = cursor.parent(0).map_err(err)?;
        later.push(cursor);
        cursor = parent;
    }
    let parent = cursor.parent(0).map_err(|_| "The first commit of a branch can't be dropped".to_string())?;
    later.reverse();
    Ok(DropPlan { target: cursor, parent, later })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DropInfo {
    pub short_id: String,
    pub summary: String,
    /// The commit itself is already on the upstream: dropping it rewrites published history.
    pub pushed: bool,
    /// A merge commit: the branch goes back to its first parent.
    pub is_merge: bool,
    /// Commits after it on the branch, which are re-created (new ids) on top of its parent.
    pub later_commits: usize,
    /// How many of those are already on the upstream.
    pub later_pushed: usize,
}

fn drop_info(repo: &Repository, id: &str) -> Result<DropInfo, String> {
    let plan = drop_plan(repo, id)?;
    Ok(DropInfo {
        short_id: plan.target.id().to_string()[..7].to_string(),
        summary: plan.target.summary().ok().flatten().unwrap_or("").to_string(),
        pushed: is_pushed(repo, plan.target.id()),
        is_merge: plan.target.parent_count() > 1,
        later_commits: plan.later.len(),
        later_pushed: plan.later.iter().filter(|c| is_pushed(repo, c.id())).count(),
    })
}

/// Removes a commit from the current branch. The commits after it are re-created on top of its
/// parent by applying each one's changes (a 3-way merge, same author, message and dates), all in
/// memory: if any of them depends on the dropped commit and would conflict, nothing is changed.
/// Only when every commit applies cleanly is the branch moved, and the index and working directory
/// with it (`git reset --hard`). That is why the working directory must be clean: the commit's
/// changes leave the files on disk, and uncommitted work would be lost with them. The old commits
/// stay in the reflog for a while.
fn drop_commit(repo: &Repository, id: &str) -> Result<(), String> {
    let plan = drop_plan(repo, id)?;
    if !crate::changes::status_of(repo)?.is_empty() {
        return Err("The working directory has uncommitted changes. Commit or stash them first: dropping a commit \
                    resets the files to the new history."
            .into());
    }

    let mut new_tip = plan.parent.clone();
    for commit in &plan.later {
        let mut merged = repo.cherrypick_commit(commit, &new_tip, 0, None).map_err(err)?;
        if merged.has_conflicts() {
            let files: Vec<String> = merged
                .conflicts()
                .map_err(err)?
                .flatten()
                .filter_map(|c| c.our.or(c.their).or(c.ancestor))
                .map(|e| String::from_utf8_lossy(&e.path).into_owned())
                .collect();
            return Err(format!(
                "Dropping this commit would conflict: the later commit {} depends on it ({}). Nothing was changed.",
                &commit.id().to_string()[..7],
                files.join(", ")
            ));
        }
        let tree = repo.find_tree(merged.write_tree_to(repo).map_err(err)?).map_err(err)?;
        let message = String::from_utf8_lossy(commit.message_raw_bytes()).into_owned();
        let rebuilt = repo
            .commit(None, &commit.author(), &commit.committer(), &message, &tree, &[&new_tip])
            .map_err(err)?;
        new_tip = repo.find_commit(rebuilt).map_err(err)?;
    }

    repo.reset(new_tip.as_object(), ResetType::Hard, None).map_err(err)
}

async fn blocking<T: Send + 'static>(
    path: String,
    f: impl FnOnce(&Repository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&Repository::discover(&path).map_err(err)?))
        .await
        .map_err(|e| e.to_string())?
}

/// What the rename dialog needs: current message and the consequences of renaming.
#[tauri::command]
pub async fn get_rename_info(path: String, id: String) -> Result<RenameInfo, String> {
    blocking(path, move |r| rename_info(r, &id)).await
}

/// Renames a commit on the current branch. Returns the new id of that commit.
#[tauri::command]
pub async fn rename_commit_message(path: String, id: String, message: String) -> Result<String, String> {
    blocking(path, move |r| rename_commit(r, &id, &message).map(|o| o.to_string())).await
}

/// What a "drop commit" would remove, for the confirmation dialog.
#[tauri::command]
pub async fn get_drop_info(path: String, id: String) -> Result<DropInfo, String> {
    blocking(path, move |r| drop_info(r, &id)).await
}

/// Drops the current branch's latest commit (and its changes). Needs a clean working directory.
#[tauri::command]
pub async fn drop_latest_commit(path: String, id: String) -> Result<(), String> {
    blocking(path, move |r| drop_commit(r, &id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    fn make(repo: &Repository, who: &str, msg: &str, parents: &[Oid], refname: Option<&str>) -> Oid {
        let sig = Signature::now(who, "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let ps: Vec<_> = parents.iter().map(|o| repo.find_commit(*o).unwrap()).collect();
        let refs: Vec<_> = ps.iter().collect();
        repo.commit(refname, &sig, &sig, msg, &tree, &refs).unwrap()
    }

    fn msg(repo: &Repository, oid: Oid) -> String {
        repo.find_commit(oid).unwrap().message().unwrap().trim().to_string()
    }

    #[test]
    fn rename_rewrites_descendants_only() {
        let dir = std::env::temp_dir().join(format!("gc-rename-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Renamer").unwrap();
        cfg.set_str("user.email", "r@example.com").unwrap();

        //   a - b ----- m - c   (main)
        //    \         /
        //     s -------      (side, merged into m)
        let a = make(&repo, "Alice", "a", &[], Some("refs/heads/main"));
        repo.set_head("refs/heads/main").unwrap();
        let b = make(&repo, "Bob", "b", &[a], Some("refs/heads/main"));
        let s = make(&repo, "Sue", "s", &[a], Some("refs/heads/side"));
        let m = make(&repo, "Mia", "m", &[b, s], Some("refs/heads/main"));
        let c = make(&repo, "Cy", "c", &[m], Some("refs/heads/main"));
        let other = make(&repo, "Oz", "o", &[a], Some("refs/heads/other"));

        // Info: how much gets rewritten, and which commits are eligible.
        assert_eq!(rename_info(&repo, &c.to_string()).unwrap().later_commits, 0);
        assert_eq!(rename_info(&repo, &b.to_string()).unwrap().later_commits, 2);
        assert_eq!(rename_info(&repo, &a.to_string()).unwrap().later_commits, 4);
        assert_eq!(rename_info(&repo, &b.to_string()).unwrap().message, "b");
        assert!(rename_info(&repo, &other.to_string()).unwrap_err().contains("current branch"));
        assert!(rename_commit(&repo, &other.to_string(), "x").unwrap_err().contains("current branch"));

        // Bad input.
        assert!(rename_commit(&repo, &b.to_string(), "  ").is_err());
        assert!(rename_commit(&repo, &b.to_string(), "b").unwrap_err().contains("unchanged"));

        // Rename a middle commit: b, m, c are rebuilt; s and the side branch are untouched.
        let new_b = rename_commit(&repo, &b.to_string(), "b, renamed").unwrap();
        assert_eq!(msg(&repo, new_b), "b, renamed");
        let new_head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(new_head.id(), c);
        assert_eq!(msg(&repo, new_head.id()), "c");
        assert_eq!(new_head.author().name(), Ok("Cy"));
        let new_m = new_head.parent(0).unwrap();
        assert_eq!(msg(&repo, new_m.id()), "m");
        assert_eq!(new_m.parent_id(0).unwrap(), new_b); // first parent follows the rename
        assert_eq!(new_m.parent_id(1).unwrap(), s); // merged-in branch is untouched
        let renamed = repo.find_commit(new_b).unwrap();
        assert_eq!(renamed.author().name(), Ok("Bob")); // author kept
        assert_eq!(renamed.committer().name(), Ok("Renamer")); // like --amend
        assert_eq!(repo.find_branch("side", BranchType::Local).unwrap().get().target(), Some(s));
        assert_eq!(repo.head().unwrap().shorthand(), Ok("main")); // still on the branch

        // Rename the tip itself, then the root (rewrites everything, side keeps the old history).
        let tip = repo.head().unwrap().peel_to_commit().unwrap().id();
        rename_commit(&repo, &tip.to_string(), "c, renamed").unwrap();
        assert_eq!(msg(&repo, repo.head().unwrap().peel_to_commit().unwrap().id()), "c, renamed");
        rename_commit(&repo, &a.to_string(), "a, renamed").unwrap();
        let mut walk = repo.revwalk().unwrap();
        walk.push_head().unwrap();
        let messages: Vec<_> = walk.map(|o| msg(&repo, o.unwrap())).collect();
        assert!(messages.contains(&"a, renamed".to_string()));
        assert!(!messages.contains(&"a".to_string()));
        assert_eq!(repo.find_branch("side", BranchType::Local).unwrap().get().target(), Some(s));

        let _ = std::fs::remove_dir_all(&dir);
    }

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

    /// Commits `content` as file `name` on HEAD (real files, so the working directory matters).
    fn commit_file(repo: &Repository, dir: &std::path::Path, name: &str, content: &str, msg: &str) -> Oid {
        std::fs::write(dir.join(name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(name)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("D", "d@example.com").unwrap();
        let parents: Vec<_> = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
        let refs: Vec<_> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap()
    }

    /// Commits an empty-tree merge of \`first\` and \`second\` on HEAD.
    fn merge_commit(repo: &Repository, first: Oid, second: Oid) -> Oid {
        let sig = Signature::now("D", "d@example.com").unwrap();
        let tree = repo.find_commit(first).unwrap().tree().unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "merge",
            &tree,
            &[&repo.find_commit(first).unwrap(), &repo.find_commit(second).unwrap()],
        )
        .unwrap()
    }

    fn new_repo(name: &str) -> (std::path::PathBuf, Repository) {
        let dir = std::env::temp_dir().join(format!("gc-drop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        (dir, repo)
    }

    fn log(repo: &Repository) -> Vec<String> {
        let mut walk = repo.revwalk().unwrap();
        walk.push_head().unwrap();
        walk.map(|o| repo.find_commit(o.unwrap()).unwrap().summary().unwrap().unwrap().to_string()).collect()
    }

    #[test]
    fn dropping_the_tip_and_older_commits() {
        let (dir, repo) = new_repo("any");
        let a = commit_file(&repo, &dir, "a.txt", "one", "a");
        let b = commit_file(&repo, &dir, "b.txt", "bee", "b");
        let c = commit_file(&repo, &dir, "c.txt", "sea", "c");
        let d = commit_file(&repo, &dir, "d.txt", "dee", "d");

        // The plan describes what a drop involves.
        let info = drop_info(&repo, &b.to_string()).unwrap();
        assert_eq!((info.summary.as_str(), info.later_commits, info.pushed, info.is_merge), ("b", 2, false, false));
        assert_eq!(drop_info(&repo, &d.to_string()).unwrap().later_commits, 0);

        // Dropping the middle commit b: c and d are replayed (new ids, same content), b's file is gone.
        drop_commit(&repo, &b.to_string()).unwrap();
        assert_eq!(log(&repo), ["d", "c", "a"]);
        assert!(!dir.join("b.txt").exists());
        assert_eq!(std::fs::read_to_string(dir.join("c.txt")).unwrap(), "sea");
        assert_eq!(std::fs::read_to_string(dir.join("d.txt")).unwrap(), "dee");
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_ne!(head.id(), d, "the later commits were re-created");
        assert_eq!(head.author().name(), Ok("D"));
        assert_eq!(head.message(), Ok("d"));
        assert_eq!(head.parent(0).unwrap().parent_id(0).unwrap(), a);
        assert!(crate::changes::status_of(&repo).unwrap().is_empty());
        assert!(repo.find_commit(b).is_ok(), "the dropped commit stays in the object database (reflog)");

        // Dropping the tip is the simple case: no later commits.
        let tip = repo.head().unwrap().peel_to_commit().unwrap().id();
        drop_commit(&repo, &tip.to_string()).unwrap();
        assert_eq!(log(&repo), ["c", "a"]);
        assert!(!dir.join("d.txt").exists());

        // The first commit, an id that is not on the branch, and a detached HEAD are refused.
        let first = log(&repo).len() - 1;
        let first_id = repo.revwalk().map(|mut w| { w.push_head().unwrap(); w.nth(first).unwrap().unwrap() }).unwrap();
        assert!(drop_commit(&repo, &first_id.to_string()).unwrap_err().contains("first commit"));
        assert!(drop_commit(&repo, &c.to_string()).is_err(), "c was re-created: the old id is no longer on the branch");
        assert!(drop_commit(&repo, "not-an-id").is_err());
        repo.set_head_detached(first_id).unwrap();
        assert!(drop_commit(&repo, &first_id.to_string()).unwrap_err().contains("Check out a branch"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_conflicting_drop_changes_nothing() {
        let (dir, repo) = new_repo("conflict");
        commit_file(&repo, &dir, "x.txt", "1", "a");
        let b = commit_file(&repo, &dir, "x.txt", "2", "b");
        commit_file(&repo, &dir, "x.txt", "3", "c"); // builds on b's change to the same line
        let d = commit_file(&repo, &dir, "other.txt", "fine", "d");

        let e = drop_commit(&repo, &b.to_string()).unwrap_err();
        assert!(e.contains("would conflict") && e.contains("x.txt") && e.contains("Nothing was changed"), "{e}");
        assert_eq!(repo.head().unwrap().target(), Some(d), "the branch did not move");
        assert_eq!(log(&repo), ["d", "c", "b", "a"]);
        assert_eq!(std::fs::read_to_string(dir.join("x.txt")).unwrap(), "3");
        assert!(crate::changes::status_of(&repo).unwrap().is_empty());
        assert_eq!(repo.state(), RepositoryState::Clean);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uncommitted_work_merges_and_pushed_commits() {
        let (dir, repo) = new_repo("rules");
        let a = commit_file(&repo, &dir, "a.txt", "one", "a");
        let branch = repo.head().unwrap().shorthand().unwrap().to_string();
        let b = commit_file(&repo, &dir, "b.txt", "bee", "b");

        // Uncommitted work blocks it, and nothing happens.
        std::fs::write(dir.join("a.txt"), "dirty").unwrap();
        assert!(drop_commit(&repo, &b.to_string()).unwrap_err().contains("uncommitted changes"));
        std::fs::write(dir.join("a.txt"), "one").unwrap();

        // A merge commit after the target makes the rewrite ambiguous; the merge itself can be dropped.
        let sig = Signature::now("D", "d@example.com").unwrap();
        let side_tree = repo.find_commit(a).unwrap().tree().unwrap();
        let side = repo.commit(None, &sig, &sig, "s", &side_tree, &[&repo.find_commit(a).unwrap()]).unwrap();
        let m = merge_commit(&repo, b, side);
        let c = commit_file(&repo, &dir, "c.txt", "sea", "c");
        let e = drop_commit(&repo, &b.to_string()).unwrap_err();
        assert!(e.contains("no merge commit after"), "{e}");
        assert!(drop_commit(&repo, &side.to_string()).is_err(), "a side branch commit is not on the main line");
        let info = drop_info(&repo, &m.to_string()).unwrap();
        assert!(info.is_merge && info.later_commits == 1);

        // Pushed commits are counted so the dialog can warn.
        repo.remote("origin", "https://example.com/r.git").unwrap();
        repo.reference(&format!("refs/remotes/origin/{branch}"), m, true, "test").unwrap();
        repo.find_branch(&branch, BranchType::Local).unwrap().set_upstream(Some(&format!("origin/{branch}"))).unwrap();
        let info = drop_info(&repo, &m.to_string()).unwrap();
        assert!(info.pushed);
        assert_eq!((info.later_commits, info.later_pushed), (1, 0), "c is not pushed yet");
        assert_eq!(drop_info(&repo, &c.to_string()).unwrap().pushed, false);

        // Dropping the merge replays c onto the merge's first parent.
        drop_commit(&repo, &m.to_string()).unwrap();
        assert_eq!(log(&repo), ["c", "b", "a"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
