use git2::{Commit, ErrorCode, Oid, Repository, StashFlags, Tree};
use serde::Serialize;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Position in the stash list: 0 is the newest ("stash@{0}").
    pub index: usize,
    pub id: String,
    pub short_id: String,
    /// "WIP on main: abc1234 subject", or "On main: <your message>".
    pub message: String,
    /// Unix seconds.
    pub time: i64,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// All stashes, newest first.
pub(crate) fn list_stashes(repo: &mut Repository) -> Result<Vec<StashEntry>, String> {
    let mut raw: Vec<(usize, String, Oid)> = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        raw.push((index, message.to_string(), *oid));
        true
    })
    .map_err(err)?;

    Ok(raw
        .into_iter()
        .map(|(index, message, oid)| StashEntry {
            index,
            id: oid.to_string(),
            short_id: oid.to_string()[..7].to_string(),
            message,
            time: repo.find_commit(oid).map(|c| c.time().seconds()).unwrap_or(0),
        })
        .collect())
}

/// The position of `oid` in the stash list, if it is a stash commit. Works on a shared
/// reference by looking at the list through a second handle.
pub(crate) fn stash_index_of(repo: &Repository, oid: Oid) -> Option<usize> {
    let mut other = Repository::open(repo.path()).ok()?;
    list_stashes(&mut other).ok()?.iter().find(|s| s.id == oid.to_string()).map(|s| s.index)
}

/// The tree of a stash's untracked files, which git keeps in a separate third parent commit.
pub(crate) fn untracked_tree<'r>(repo: &'r Repository, commit: &Commit<'r>) -> Option<Tree<'r>> {
    if commit.parent_count() == 3 && stash_index_of(repo, commit.id()).is_some() {
        commit.parent(2).ok()?.tree().ok()
    } else {
        None
    }
}

/// Moves every uncommitted change into a new stash and cleans the working directory: staged and
/// unstaged edits, and untracked files (ignored files stay where they are).
pub(crate) fn save_stash(repo: &mut Repository, message: Option<&str>) -> Result<Oid, String> {
    let signature = repo
        .signature()
        .map_err(|_| "Git identity not set. Configure user.name and user.email.".to_string())?;
    if repo.head().and_then(|h| h.peel_to_commit()).is_err() {
        return Err("Make a first commit before stashing".into());
    }
    let message = message.map(str::trim).filter(|m| !m.is_empty());
    match repo.stash_save2(&signature, message, Some(StashFlags::INCLUDE_UNTRACKED)) {
        Ok(oid) => Ok(oid),
        Err(e) if e.code() == ErrorCode::NotFound => Err("There are no changes to stash".into()),
        Err(e) => Err(err(e)),
    }
}

#[tauri::command]
pub async fn get_stashes(path: String) -> Result<Vec<StashEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        list_stashes(&mut Repository::discover(&path).map_err(err)?)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stashes all uncommitted changes, with an optional message. Returns the stash commit id.
#[tauri::command]
pub async fn create_stash(path: String, message: Option<String>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut repo = Repository::discover(&path).map_err(err)?;
        save_stash(&mut repo, message.as_deref()).map(|o| o.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexAddOption, Signature, StatusOptions};
    use std::fs;

    fn setup(name: &str) -> (std::path::PathBuf, Repository) {
        let dir = std::env::temp_dir().join(format!("gc-stash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Stasher").unwrap();
        cfg.set_str("user.email", "s@example.com").unwrap();
        (dir, repo)
    }

    fn commit_all(repo: &Repository, msg: &str) {
        let mut index = repo.index().unwrap();
        index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Stasher", "s@example.com").unwrap();
        let parents: Vec<_> = repo.head().ok().and_then(|h| h.peel_to_commit().ok()).into_iter().collect();
        let refs: Vec<_> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap();
    }

    fn dirty_files(repo: &Repository) -> usize {
        let mut o = StatusOptions::new();
        o.include_untracked(true).recurse_untracked_dirs(true);
        repo.statuses(Some(&mut o)).unwrap().len()
    }

    #[test]
    fn stashing_moves_all_changes_away_and_lists_them_newest_first() {
        let (dir, mut repo) = setup("save");

        // Nothing to stash yet, and no commit to base a stash on.
        assert!(save_stash(&mut repo, None).unwrap_err().contains("first commit"));
        fs::write(dir.join("a.txt"), "one").unwrap();
        fs::write(dir.join("keep.txt"), "keep").unwrap();
        commit_all(&repo, "base");
        assert!(save_stash(&mut repo, None).unwrap_err().contains("no changes"));
        assert!(list_stashes(&mut repo).unwrap().is_empty());

        // A tracked edit, a newly staged file and an untracked file all go into the stash.
        fs::write(dir.join("a.txt"), "two").unwrap();
        fs::write(dir.join("staged.txt"), "staged").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("staged.txt")).unwrap();
        index.write().unwrap();
        fs::write(dir.join("untracked.txt"), "untracked").unwrap();
        assert_eq!(dirty_files(&repo), 3);

        let first = save_stash(&mut repo, Some("  my first stash  ")).unwrap();
        assert_eq!(dirty_files(&repo), 0, "the working directory is clean afterwards");
        assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "one");
        assert!(!dir.join("untracked.txt").exists() && !dir.join("staged.txt").exists());

        // A second stash (without a message) goes on top.
        fs::write(dir.join("a.txt"), "three").unwrap();
        let second = save_stash(&mut repo, None).unwrap();
        let list = list_stashes(&mut repo).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].index, list[0].id.as_str()), (0, second.to_string().as_str()));
        assert_eq!((list[1].index, list[1].id.as_str()), (1, first.to_string().as_str()));
        assert!(list[1].message.contains("my first stash"), "{}", list[1].message);
        assert!(list[0].message.starts_with("WIP on"), "{}", list[0].message);
        assert_eq!(list[0].short_id.len(), 7);

        // Helpers used by the graph and the detail view.
        assert_eq!(stash_index_of(&repo, second), Some(0));
        assert_eq!(stash_index_of(&repo, first), Some(1));
        assert_eq!(stash_index_of(&repo, repo.head().unwrap().target().unwrap()), None);
        let stash = repo.find_commit(first).unwrap();
        assert!(untracked_tree(&repo, &stash).unwrap().get_name("untracked.txt").is_some());
        let plain = repo.find_commit(second).unwrap();
        // libgit2 records an untracked-files commit even when there are none: it is just empty.
        assert!(untracked_tree(&repo, &plain).map_or(true, |t| t.is_empty()), "no untracked files in the second stash");

        let _ = fs::remove_dir_all(&dir);
    }
}
