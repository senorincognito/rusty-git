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

    let edits = HashMap::from([(target_oid, message)]);
    let rebuilt = rebuild_with_messages(repo, &target, &edits, &format!("rename commit {}", &id[..7.min(id.len())]))?;
    Ok(rebuilt[&target_oid])
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

/// Above this many commits the rebase editor would be unwieldy; pick a later base instead.
const MAX_REBASE_COMMITS: usize = 500;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RebaseCommit {
    pub id: String,
    pub short_id: String,
    /// Full message, to pre-fill the editor.
    pub message: String,
    pub author: String,
    /// Unix seconds.
    pub time: i64,
    /// Already on the upstream: changing it rewrites published history.
    pub pushed: bool,
    pub is_merge: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RebasePlan {
    /// HEAD when the plan was made; applying refuses if the branch moved since.
    pub head_id: String,
    /// The commits after the base on the current branch, newest first.
    pub commits: Vec<RebaseCommit>,
}

/// The commits an interactive rebase onto `base` covers: every commit after it on the current
/// branch's own (first-parent) line of history, newest first. `base` itself is not included, like
/// `git rebase -i <base>`.
fn rebase_plan(repo: &Repository, base_id: &str) -> Result<RebasePlan, String> {
    let base = Oid::from_str(base_id).map_err(err)?;
    let head = repo.head().and_then(|h| h.peel_to_commit()).map_err(|_| "There are no commits yet".to_string())?;
    let mut commits = Vec::new();
    let mut cursor = head.clone();
    while cursor.id() != base {
        if commits.len() == MAX_REBASE_COMMITS {
            return Err(format!(
                "More than {MAX_REBASE_COMMITS} commits follow this one; start the rebase from a later commit"
            ));
        }
        let id = cursor.id().to_string();
        commits.push(RebaseCommit {
            short_id: id[..7].to_string(),
            id,
            message: String::from_utf8_lossy(cursor.message_raw_bytes()).trim_end().to_string(),
            author: cursor.author().name().unwrap_or("").to_string(),
            time: cursor.time().seconds(),
            pushed: is_pushed(repo, cursor.id()),
            is_merge: cursor.parent_count() > 1,
        });
        cursor = cursor.parent(0).map_err(|_| {
            "Only commits on the current branch's own line of history can start an interactive rebase".to_string()
        })?;
    }
    if commits.is_empty() {
        return Err("There are no commits after this one on the current branch".into());
    }
    Ok(RebasePlan { head_id: head.id().to_string(), commits })
}

#[derive(serde::Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RebaseAction {
    /// Keep the commit as it is.
    Pick,
    /// Keep the commit but give it a new message.
    Reword,
    /// Meld the commit into the one before it (older), appending its message.
    Squash,
    /// Remove the commit and its changes from the branch.
    Drop,
}

#[derive(serde::Deserialize, Debug)]
pub struct RebaseStep {
    pub id: String,
    pub action: RebaseAction,
    /// The new message of a `Reword`.
    #[serde(default)]
    pub message: Option<String>,
}

/// One commit of the result: a commit of the plan plus the commits squashed into it.
struct RebaseGroup {
    /// The commit the group starts from (the oldest member) and the commits squashed into it, oldest first.
    head: Oid,
    squashed: Vec<Oid>,
    /// The message of the head: its own, or the reworded one.
    message: String,
    squashed_messages: Vec<String>,
    reworded: bool,
    dropped: bool,
}

impl RebaseGroup {
    fn changed(&self) -> bool {
        self.reworded || self.dropped || !self.squashed.is_empty()
    }

    /// The head's message followed by the messages of the squashed commits, separated by blank lines.
    fn full_message(&self) -> String {
        let mut text = self.message.trim_end().to_string();
        for m in &self.squashed_messages {
            text.push_str("\n\n");
            text.push_str(m.trim());
        }
        text.push('\n');
        text
    }
}

/// Applies an interactive rebase made of `pick`, `reword`, `squash` and `drop` steps (commits without a step
/// are picked). A squashed commit is melded into the commit before it in the list (the next older one,
/// following a chain of squashes to the commit that starts it); the result keeps that commit's author and
/// parents, takes the content of the newest commit of the group, and its message is the older message
/// followed by the squashed ones. Every commit after the oldest change is rebuilt on top, then the branch
/// moves; without a drop the files and the index stay as they are because the tip's content never changes.
///
/// A `drop` changes content: from the first dropped commit on, the later commits are replayed in memory with
/// `cherrypick_commit` onto the rebuilt history (3-way merges). If one of them depends on a dropped commit the
/// whole rebase is abandoned with nothing changed; otherwise a hard reset moves the branch, index and files, so
/// the working directory must be clean. Merge commits can't be replayed that way, so they are refused after a drop.
///
/// Refuses if HEAD is no longer `head_id`, so a plan shown earlier can't be applied to a branch that moved meanwhile.
fn apply_rebase(repo: &Repository, base_id: &str, head_id: &str, steps: &[RebaseStep]) -> Result<(), String> {
    if repo.state() != RepositoryState::Clean {
        return Err("Finish the merge, rebase or other operation in progress first".into());
    }
    let plan = rebase_plan(repo, base_id)?;
    if plan.head_id != head_id {
        return Err("The branch has changed since the rebase was opened. Close it and start again.".into());
    }

    let mut chosen: HashMap<&str, &RebaseStep> = HashMap::new();
    for step in steps {
        if !plan.commits.iter().any(|c| c.id == step.id) {
            return Err(format!("Commit {} is not part of this rebase", &step.id[..7.min(step.id.len())]));
        }
        chosen.insert(step.id.as_str(), step);
    }

    // Oldest first, grouping every squashed commit with the commit it melds into.
    let mut groups: Vec<RebaseGroup> = Vec::new();
    for c in plan.commits.iter().rev() {
        let oid = Oid::from_str(&c.id).map_err(err)?;
        let step = chosen.get(c.id.as_str());
        match step.map_or(RebaseAction::Pick, |s| s.action) {
            RebaseAction::Squash => {
                if c.is_merge {
                    return Err(format!("{} is a merge commit and can't be squashed", c.short_id));
                }
                let target = groups.last_mut().ok_or_else(|| {
                    format!(
                        "{} is the oldest commit of this rebase: there is nothing before it to squash into",
                        c.short_id
                    )
                })?;
                if target.dropped {
                    return Err(format!("{} can't be squashed into a commit that is dropped", c.short_id));
                }
                target.squashed.push(oid);
                target.squashed_messages.push(c.message.clone());
            }
            RebaseAction::Drop => groups.push(RebaseGroup {
                head: oid,
                squashed: Vec::new(),
                message: c.message.clone(),
                squashed_messages: Vec::new(),
                reworded: false,
                dropped: true,
            }),
            action => {
                let mut message = c.message.clone();
                let mut reworded = false;
                if action == RebaseAction::Reword {
                    let new = git2::message_prettify(step.and_then(|s| s.message.as_deref()).unwrap_or(""), None)
                        .map_err(err)?;
                    if new.trim().is_empty() {
                        return Err(format!("The message of {} is empty", c.short_id));
                    }
                    // Giving a commit its own message back changes nothing.
                    if new.trim() != c.message.trim() {
                        message = new;
                        reworded = true;
                    }
                }
                groups.push(RebaseGroup {
                    head: oid,
                    squashed: Vec::new(),
                    message,
                    squashed_messages: Vec::new(),
                    reworded,
                    dropped: false,
                });
            }
        }
    }
    let first_changed = groups
        .iter()
        .position(RebaseGroup::changed)
        .ok_or("Nothing to change: reword, squash or drop at least one commit")?;

    let old_head = Oid::from_str(&plan.head_id).map_err(err)?;
    let has_drop = groups.iter().any(|g| g.dropped);
    if has_drop && !crate::changes::status_of(repo)?.is_empty() {
        return Err("The working directory has uncommitted changes. Commit or stash them first: dropping a commit \
                    resets the files to the new history."
            .into());
    }

    let mut rebuilt: HashMap<Oid, Oid> = HashMap::new();
    // Once a commit is dropped the later trees can no longer be reused: they are replayed instead.
    let mut replaying = false;
    for group in &groups[first_changed..] {
        let head = repo.find_commit(group.head).map_err(err)?;
        let parents = head
            .parent_ids()
            .map(|p| repo.find_commit(*rebuilt.get(&p).unwrap_or(&p)).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;

        if group.dropped {
            replaying = true;
            // Whatever was built on this commit is built on its (rebuilt) parent instead.
            let below = parents.first().ok_or("The first commit of a branch can't be dropped")?.id();
            rebuilt.insert(group.head, below);
            continue;
        }

        let mut members = vec![group.head];
        members.extend(&group.squashed);
        let tree = if replaying {
            replay_group(repo, &members, &parents)?
        } else {
            // Each commit contains the ones before it: the group's content is that of its newest commit.
            let newest = repo.find_commit(*members.last().unwrap()).map_err(err)?;
            newest.tree().map_err(err)?
        };

        let parent_refs: Vec<&Commit> = parents.iter().collect();
        let (text, committer) = if group.changed() {
            // Like --amend: the person rebasing becomes the committer (falling back to the old one).
            (group.full_message(), repo.signature().unwrap_or_else(|_| head.committer().to_owned()))
        } else {
            (String::from_utf8_lossy(head.message_raw_bytes()).into_owned(), head.committer().to_owned())
        };
        let new = repo.commit(None, &head.author(), &committer, &text, &tree, &parent_refs).map_err(err)?;
        for m in members {
            rebuilt.insert(m, new);
        }
    }

    let new_head = rebuilt[&old_head];
    if has_drop {
        let tip = repo.find_commit(new_head).map_err(err)?;
        return repo.reset(tip.as_object(), ResetType::Hard, None).map_err(err);
    }
    let reworded = groups.iter().filter(|g| g.reworded).count();
    let squashed: usize = groups.iter().map(|g| g.squashed.len()).sum();
    let note = format!("interactive rebase: {reworded} reworded, {squashed} squashed");
    move_head_to(repo, new_head, &note)
}

/// The tree of a group's commits (`members`, oldest first) replayed onto `parents[0]`, the rebuilt history
/// below them: each commit's own changes are 3-way merged in turn. A conflict abandons the whole rebase.
fn replay_group<'r>(repo: &'r Repository, members: &[Oid], parents: &[Commit<'r>]) -> Result<git2::Tree<'r>, String> {
    let mut base = parents.first().ok_or("The first commit of a branch can't be replayed")?.clone();
    let mut tree = base.tree().map_err(err)?;
    for (n, oid) in members.iter().enumerate() {
        let commit = repo.find_commit(*oid).map_err(err)?;
        if commit.parent_count() != 1 {
            return Err(format!(
                "{} is a merge commit and can't be replayed after a dropped commit",
                &oid.to_string()[..7]
            ));
        }
        let mut merged = repo.cherrypick_commit(&commit, &base, 0, None).map_err(err)?;
        if merged.has_conflicts() {
            let files: Vec<String> = merged
                .conflicts()
                .map_err(err)?
                .flatten()
                .filter_map(|c| c.our.or(c.their).or(c.ancestor))
                .map(|e| String::from_utf8_lossy(&e.path).into_owned())
                .collect();
            return Err(format!(
                "The commit {} depends on a commit that is dropped ({}). Nothing was changed.",
                &oid.to_string()[..7],
                files.join(", ")
            ));
        }
        tree = repo.find_tree(merged.write_tree_to(repo).map_err(err)?).map_err(err)?;
        if n + 1 < members.len() {
            // The next member is merged on top of this result: give it a commit to stand on.
            let sig = commit.committer().to_owned();
            let step = repo.commit(None, &sig, &sig, "squash step", &tree, &[&base]).map_err(err)?;
            base = repo.find_commit(step).map_err(err)?;
        }
    }
    Ok(tree)
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
        let rebuilt =
            repo.commit(None, &commit.author(), &commit.committer(), &message, &tree, &[&new_tip]).map_err(err)?;
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

/// The commits an interactive rebase onto `id` would cover (newest first).
#[tauri::command]
pub async fn get_rebase_plan(path: String, id: String) -> Result<RebasePlan, String> {
    blocking(path, move |r| rebase_plan(r, &id)).await
}

/// Applies an interactive rebase of pick / reword / squash steps (see [`apply_rebase`]).
#[tauri::command]
pub async fn apply_rebase_cmd(
    path: String,
    base_id: String,
    head_id: String,
    steps: Vec<RebaseStep>,
) -> Result<(), String> {
    blocking(path, move |r| apply_rebase(r, &base_id, &head_id, &steps)).await
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
        cfg.set_str("core.autocrlf", "false").unwrap(); // the result must not depend on the machine's git config

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
    fn interactive_rebase_rewords_several_commits() {
        let (dir, repo) = new_repo("reword");
        let base = commit_file(&repo, &dir, "a.txt", "1", "base");
        let b = commit_file(&repo, &dir, "a.txt", "2", "b");
        let c = commit_file(&repo, &dir, "a.txt", "3", "c");
        let d = commit_file(&repo, &dir, "a.txt", "4", "d");
        repo.config().unwrap().set_str("user.name", "Editor").unwrap();
        repo.config().unwrap().set_str("user.email", "e@example.com").unwrap();

        // The plan covers the commits after the base, newest first; the base itself is not part of it.
        let plan = rebase_plan(&repo, &base.to_string()).unwrap();
        let ids: Vec<String> = plan.commits.iter().map(|c| c.id.clone()).collect();
        assert_eq!(ids, [d.to_string(), c.to_string(), b.to_string()]);
        assert!(rebase_plan(&repo, &d.to_string()).unwrap_err().contains("no commits after"));

        let edit =
            |id: Oid, m: &str| RebaseStep { id: id.to_string(), action: RebaseAction::Reword, message: Some(m.into()) };
        let head = plan.head_id.clone();
        let bs = base.to_string();
        // Refusals: nothing changed, an empty message, a commit outside the plan, a stale plan.
        assert!(apply_rebase(&repo, &bs, &head, &[edit(c, "c")]).unwrap_err().contains("Nothing to change"));
        assert!(apply_rebase(&repo, &bs, &head, &[edit(c, "  ")]).unwrap_err().contains("empty"));
        assert!(apply_rebase(&repo, &bs, &head, &[edit(base, "x")]).unwrap_err().contains("not part"));
        assert!(apply_rebase(&repo, &bs, &b.to_string(), &[edit(c, "x")]).unwrap_err().contains("changed since"));

        // Reword b and d (c in between is rebuilt with its old message).
        apply_rebase(&repo, &bs, &head, &[edit(d, "D!"), edit(c, "c"), edit(b, "B!\n\nbody")]).unwrap();
        assert_eq!(log(&repo), ["D!", "c", "B!", "base"]);
        let new_head = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(new_head.tree_id(), repo.find_commit(d).unwrap().tree_id(), "same content");
        assert_eq!(new_head.committer().name(), Ok("Editor"));
        let new_c = new_head.parent(0).unwrap();
        assert_eq!(new_c.committer().name(), Ok("D"), "an untouched commit keeps its committer");
        assert_eq!(new_c.parent(0).unwrap().message(), Ok("B!\n\nbody\n"));
        assert_eq!(new_c.parent(0).unwrap().parent_id(0).unwrap(), base, "the base is untouched");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "4");

        // A commit beside the current line of history can't be a base.
        repo.branch("side", &repo.find_commit(b).unwrap(), false).unwrap();
        let other = repo.find_commit(base).unwrap();
        let sig = Signature::now("D", "d@example.com").unwrap();
        let side = repo
            .commit(
                Some("refs/heads/side"),
                &sig,
                &sig,
                "side",
                &other.tree().unwrap(),
                &[&repo.find_commit(b).unwrap()],
            )
            .unwrap();
        assert!(rebase_plan(&repo, &side.to_string()).unwrap_err().contains("own line"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn interactive_rebase_squashes_into_the_commit_before() {
        let (dir, repo) = new_repo("squash");
        let base = commit_file(&repo, &dir, "a.txt", "1", "base");
        let b = commit_file(&repo, &dir, "a.txt", "2", "b\n\nabout b");
        let c = commit_file(&repo, &dir, "a.txt", "3", "c");
        commit_file(&repo, &dir, "a.txt", "4", "d");
        let e = commit_file(&repo, &dir, "a.txt", "5", "e");
        repo.config().unwrap().set_str("user.name", "Editor").unwrap();
        repo.config().unwrap().set_str("user.email", "e@example.com").unwrap();
        let bs = base.to_string();
        let plan = rebase_plan(&repo, &bs).unwrap();
        let head = plan.head_id.clone();
        let step = |id: Oid, action: RebaseAction, m: Option<&str>| RebaseStep {
            id: id.to_string(),
            action,
            message: m.map(str::to_string),
        };

        // Refusals: the oldest commit has nothing before it; nothing changes when only picks are given.
        let e1 = apply_rebase(&repo, &bs, &head, &[step(b, RebaseAction::Squash, None)]).unwrap_err();
        assert!(e1.contains("oldest commit"), "{e1}");
        let e2 = apply_rebase(&repo, &bs, &head, &[step(c, RebaseAction::Pick, None)]).unwrap_err();
        assert!(e2.contains("Nothing to change"), "{e2}");

        // Squash c into b, and e into d (which is itself picked): b+c, d+e remain.
        apply_rebase(&repo, &bs, &head, &[step(c, RebaseAction::Squash, None), step(e, RebaseAction::Squash, None)])
            .unwrap();
        assert_eq!(log(&repo), ["d", "b", "base"]);
        let tip = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.message(), Ok("d\n\ne\n"), "the squashed message is appended");
        assert_eq!(tip.tree_id(), repo.find_commit(e).unwrap().tree_id(), "the content of the newest commit");
        let first = tip.parent(0).unwrap();
        assert_eq!(first.message(), Ok("b\n\nabout b\n\nc\n"));
        assert_eq!(first.tree_id(), repo.find_commit(c).unwrap().tree_id());
        assert_eq!(first.parent_id(0).unwrap(), base);
        assert_eq!(first.author().name(), Ok("D"), "the older commit's author stays");
        assert_eq!(first.committer().name(), Ok("Editor"));
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "5");

        // A chain: squash two in a row into a reworded commit, with a later commit rebuilt on top.
        let (dir2, repo2) = new_repo("squash-chain");
        let base = commit_file(&repo2, &dir2, "a.txt", "1", "base");
        let b = commit_file(&repo2, &dir2, "a.txt", "2", "b");
        let c = commit_file(&repo2, &dir2, "a.txt", "3", "c");
        let d = commit_file(&repo2, &dir2, "a.txt", "4", "d");
        let e = commit_file(&repo2, &dir2, "a.txt", "5", "e");
        let bs = base.to_string();
        let head = rebase_plan(&repo2, &bs).unwrap().head_id;
        apply_rebase(
            &repo2,
            &bs,
            &head,
            &[
                step(b, RebaseAction::Reword, Some("B")),
                step(c, RebaseAction::Squash, None),
                step(d, RebaseAction::Squash, None),
            ],
        )
        .unwrap();
        assert_eq!(log(&repo2), ["e", "B", "base"]);
        let tip = repo2.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.committer().name(), Ok("D"), "an untouched commit keeps its committer");
        assert_eq!(tip.tree_id(), repo2.find_commit(e).unwrap().tree_id());
        assert_eq!(tip.parent(0).unwrap().message(), Ok("B\n\nc\n\nd\n"));

        // A merge commit can't be squashed (its second parent would be lost), but squashing into one is fine.
        let (dir3, repo3) = new_repo("squash-merge");
        let base = commit_file(&repo3, &dir3, "a.txt", "1", "base");
        let side = {
            let sig = Signature::now("D", "d@example.com").unwrap();
            repo3
                .commit(
                    None,
                    &sig,
                    &sig,
                    "side",
                    &repo3.find_commit(base).unwrap().tree().unwrap(),
                    &[&repo3.find_commit(base).unwrap()],
                )
                .unwrap()
        };
        let m = merge_commit(&repo3, base, side);
        let after = commit_file(&repo3, &dir3, "a.txt", "2", "after");
        let bs = base.to_string();
        let head = rebase_plan(&repo3, &bs).unwrap().head_id;
        let e = apply_rebase(&repo3, &bs, &head, &[step(m, RebaseAction::Squash, None)]).unwrap_err();
        assert!(e.contains("merge commit"), "{e}");
        apply_rebase(&repo3, &bs, &head, &[step(after, RebaseAction::Squash, None)]).unwrap();
        let tip = repo3.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.parent_count(), 2, "the merge keeps both parents");
        assert_eq!(tip.message(), Ok("merge\n\nafter\n"));
        assert_eq!(tip.tree_id(), repo3.find_commit(after).unwrap().tree_id());

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
        let _ = std::fs::remove_dir_all(&dir3);
    }

    #[test]
    fn interactive_rebase_drops_commits() {
        let (dir, repo) = new_repo("rebase-drop");
        let base = commit_file(&repo, &dir, "base.txt", "0", "base");
        let a = commit_file(&repo, &dir, "a.txt", "a", "add a");
        let b = commit_file(&repo, &dir, "b.txt", "b", "add b");
        let c = commit_file(&repo, &dir, "c.txt", "c", "add c");
        let d = commit_file(&repo, &dir, "a.txt", "a2", "change a");
        repo.config().unwrap().set_str("user.name", "Editor").unwrap();
        repo.config().unwrap().set_str("user.email", "e@example.com").unwrap();
        let bs = base.to_string();
        let head = rebase_plan(&repo, &bs).unwrap().head_id;
        let step = |id: Oid, action: RebaseAction, m: Option<&str>| RebaseStep {
            id: id.to_string(),
            action,
            message: m.map(str::to_string),
        };

        // Dropping a commit that a later one builds on conflicts: nothing changes at all.
        let e = apply_rebase(&repo, &bs, &head, &[step(a, RebaseAction::Drop, None)]).unwrap_err();
        assert!(e.contains("depends on") && e.contains("a.txt"), "{e}");
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().id(), d);
        assert_eq!(log(&repo), ["change a", "add c", "add b", "add a", "base"]);

        // Squashing into a dropped commit is refused; so is a dirty working directory.
        let e =
            apply_rebase(&repo, &bs, &head, &[step(b, RebaseAction::Drop, None), step(c, RebaseAction::Squash, None)])
                .unwrap_err();
        assert!(e.contains("dropped"), "{e}");
        std::fs::write(dir.join("dirty.txt"), "x").unwrap();
        let e = apply_rebase(&repo, &bs, &head, &[step(b, RebaseAction::Drop, None)]).unwrap_err();
        assert!(e.contains("uncommitted changes"), "{e}");
        std::fs::remove_file(dir.join("dirty.txt")).unwrap();

        // Drop b (independent of the rest) and reword c: later commits are replayed on top, files follow.
        apply_rebase(
            &repo,
            &bs,
            &head,
            &[step(b, RebaseAction::Drop, None), step(c, RebaseAction::Reword, Some("C!"))],
        )
        .unwrap();
        assert_eq!(log(&repo), ["change a", "C!", "add a", "base"]);
        assert!(!dir.join("b.txt").exists(), "the dropped commit's file is gone");
        assert_eq!(std::fs::read_to_string(dir.join("a.txt")).unwrap(), "a2");
        assert_eq!(std::fs::read_to_string(dir.join("c.txt")).unwrap(), "c");
        let tip = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.author().name(), Ok("D"), "the author is kept");
        assert!(crate::changes::status_of(&repo).unwrap().is_empty(), "the working directory matches the new tip");
        let rewritten_a = tip.parent(0).unwrap().parent(0).unwrap();
        assert_eq!(rewritten_a.parent_id(0).unwrap(), base, "commits before the first change are untouched");
        assert_eq!(rewritten_a.id(), a);

        // Drop everything above the base, including the tip.
        let bs2 = base.to_string();
        let head2 = rebase_plan(&repo, &bs2).unwrap();
        let steps: Vec<RebaseStep> = head2
            .commits
            .iter()
            .map(|c| RebaseStep { id: c.id.clone(), action: RebaseAction::Drop, message: None })
            .collect();
        apply_rebase(&repo, &bs2, &head2.head_id, &steps).unwrap();
        assert_eq!(log(&repo), ["base"]);
        assert!(!dir.join("a.txt").exists() && !dir.join("c.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dropping_in_a_rebase_refuses_merges_after_it_but_not_before() {
        let (dir, repo) = new_repo("rebase-drop-merge");
        let base = commit_file(&repo, &dir, "base.txt", "0", "base");
        let side = {
            let sig = Signature::now("D", "d@example.com").unwrap();
            repo.commit(
                None,
                &sig,
                &sig,
                "side",
                &repo.find_commit(base).unwrap().tree().unwrap(),
                &[&repo.find_commit(base).unwrap()],
            )
            .unwrap()
        };
        let x = commit_file(&repo, &dir, "x.txt", "x", "x");
        let m = merge_commit(&repo, x, side);
        let y = commit_file(&repo, &dir, "y.txt", "y", "y");
        let z = commit_file(&repo, &dir, "z.txt", "z", "z");
        let bs = base.to_string();
        let head = rebase_plan(&repo, &bs).unwrap().head_id;
        let step = |id: Oid, action: RebaseAction| RebaseStep { id: id.to_string(), action, message: None };

        // x -> m -> y -> z: dropping x means replaying the merge m, which isn't supported.
        let e = apply_rebase(&repo, &bs, &head, &[step(x, RebaseAction::Drop)]).unwrap_err();
        assert!(e.contains("merge commit"), "{e}");
        // Dropping y (after the merge) is fine: the merge is before the first change and stays as it is.
        apply_rebase(&repo, &bs, &head, &[step(y, RebaseAction::Drop)]).unwrap();
        assert_eq!(log(&repo).first().map(String::as_str), Some("z"));
        let tip = repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.parent_id(0).unwrap(), m, "the merge commit is untouched");
        assert!(!dir.join("y.txt").exists());
        assert!(dir.join("z.txt").exists());
        let _ = z;

        let _ = std::fs::remove_dir_all(&dir);
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
        let first_id = repo
            .revwalk()
            .map(|mut w| {
                w.push_head().unwrap();
                w.nth(first).unwrap().unwrap()
            })
            .unwrap();
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
        assert!(!drop_info(&repo, &c.to_string()).unwrap().pushed);

        // Dropping the merge replays c onto the merge's first parent.
        drop_commit(&repo, &m.to_string()).unwrap();
        assert_eq!(log(&repo), ["c", "b", "a"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
