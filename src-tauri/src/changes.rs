use std::path::Path;

use git2::{ObjectType, Oid, Repository, RepositoryState, Status, StatusOptions};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    /// Status in the index (staged), if any: "new" | "modified" | "deleted" | "typechange".
    pub staged: Option<&'static str>,
    /// Status in the working tree (unstaged), if any; also "conflicted".
    pub unstaged: Option<&'static str>,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

fn status_of(repo: &Repository) -> Result<Vec<FileChange>, String> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        // Renames are reported as delete + add so each path can be (un)staged independently.
        .renames_head_to_index(false)
        .renames_index_to_workdir(false);

    let statuses = repo.statuses(Some(&mut opts)).map_err(err)?;
    let mut out = Vec::new();
    for entry in statuses.iter() {
        let Ok(path) = entry.path() else { continue };
        let s = entry.status();

        let staged = if s.contains(Status::INDEX_NEW) {
            Some("new")
        } else if s.contains(Status::INDEX_MODIFIED) {
            Some("modified")
        } else if s.contains(Status::INDEX_DELETED) {
            Some("deleted")
        } else if s.contains(Status::INDEX_TYPECHANGE) {
            Some("typechange")
        } else {
            None
        };
        let unstaged = if s.contains(Status::CONFLICTED) {
            Some("conflicted")
        } else if s.contains(Status::WT_NEW) {
            Some("new")
        } else if s.contains(Status::WT_MODIFIED) {
            Some("modified")
        } else if s.contains(Status::WT_DELETED) {
            Some("deleted")
        } else if s.contains(Status::WT_TYPECHANGE) {
            Some("typechange")
        } else {
            None
        };

        if staged.is_some() || unstaged.is_some() {
            out.push(FileChange { path: path.to_string(), staged, unstaged });
        }
    }
    Ok(out)
}

fn stage(repo: &Repository, paths: &[String]) -> Result<(), String> {
    let workdir = repo.workdir().ok_or("repository has no working directory")?;
    let mut index = repo.index().map_err(err)?;
    for p in paths {
        let rel = Path::new(p);
        if workdir.join(rel).symlink_metadata().is_ok() {
            index.add_path(rel).map_err(err)?;
        } else {
            index.remove_path(rel).map_err(err)?; // deleted in the working tree
        }
    }
    index.write().map_err(err)
}

fn unstage(repo: &Repository, paths: &[String]) -> Result<(), String> {
    match repo.head() {
        Ok(head) => {
            let target = head.peel(ObjectType::Commit).map_err(err)?;
            repo.reset_default(Some(&target), paths.iter().map(String::as_str))
                .map_err(err)
        }
        // No commits yet: unstaging just means dropping the entry from the index.
        Err(_) => {
            let mut index = repo.index().map_err(err)?;
            for p in paths {
                index.remove_path(Path::new(p)).map_err(err)?;
            }
            index.write().map_err(err)
        }
    }
}

fn commit_staged(repo: &mut Repository, message: &str) -> Result<Oid, String> {
    let message = git2::message_prettify(message, None).map_err(err)?;
    if message.trim().is_empty() {
        return Err("Commit message is empty".into());
    }
    let sig = repo
        .signature()
        .map_err(|_| "Git identity not set. Configure user.name and user.email.".to_string())?;

    // Concluding a merge: the merge heads become additional parents.
    // (Collected first because mergehead_foreach needs `&mut repo`.)
    let merging = repo.state() == RepositoryState::Merge;
    let mut merge_heads = Vec::new();
    if merging {
        repo.mergehead_foreach(|oid| {
            merge_heads.push(*oid);
            true
        })
        .map_err(err)?;
    }

    let mut index = repo.index().map_err(err)?;
    if index.has_conflicts() {
        return Err("Resolve conflicts before committing".into());
    }
    let tree = repo.find_tree(index.write_tree().map_err(err)?).map_err(err)?;

    let mut parents = Vec::new();
    if let Ok(head) = repo.head() {
        parents.push(head.peel_to_commit().map_err(err)?);
    }
    for oid in merge_heads {
        parents.push(repo.find_commit(oid).map_err(err)?);
    }

    if !merging && parents.first().is_some_and(|p| p.tree_id() == tree.id()) {
        return Err("Nothing staged to commit".into());
    }
    if !merging && parents.is_empty() && tree.is_empty() {
        return Err("Nothing staged to commit".into());
    }

    let parent_refs: Vec<_> = parents.iter().collect();
    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, &message, &tree, &parent_refs)
        .map_err(err)?;
    if merging {
        repo.cleanup_state().map_err(err)?;
    }
    Ok(oid)
}

async fn blocking<T, F>(path: String, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&mut Repository) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let mut repo = Repository::discover(&path).map_err(err)?;
        f(&mut repo)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_status(path: String) -> Result<Vec<FileChange>, String> {
    blocking(path, |r| status_of(r)).await
}

#[tauri::command]
pub async fn stage_paths(path: String, paths: Vec<String>) -> Result<(), String> {
    blocking(path, move |r| stage(r, &paths)).await
}

#[tauri::command]
pub async fn unstage_paths(path: String, paths: Vec<String>) -> Result<(), String> {
    blocking(path, move |r| unstage(r, &paths)).await
}

/// Commits the index. Returns the new commit id.
#[tauri::command]
pub async fn create_commit(path: String, message: String) -> Result<String, String> {
    blocking(path, move |r| commit_staged(r, &message).map(|o| o.to_string())).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn find<'a>(list: &'a [FileChange], p: &str) -> &'a FileChange {
        list.iter().find(|c| c.path == p).unwrap()
    }

    #[test]
    fn stage_commit_unstage_flow() {
        let dir = std::env::temp_dir().join(format!("gc-changes-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut repo = Repository::init(&dir).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();

        fs::write(dir.join("a.txt"), "one").unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        fs::write(dir.join("sub").join("b.txt"), "two").unwrap();

        let st = status_of(&repo).unwrap();
        assert_eq!(find(&st, "a.txt").unstaged, Some("new"));
        assert_eq!(find(&st, "sub/b.txt").unstaged, Some("new"));

        // Nothing staged yet, including on an unborn branch.
        assert!(commit_staged(&mut repo, "x").is_err());

        stage(&repo, &["a.txt".into(), "sub/b.txt".into()]).unwrap();
        assert_eq!(find(&status_of(&repo).unwrap(), "a.txt").staged, Some("new"));

        // Unstage on an unborn branch.
        unstage(&repo, &["sub/b.txt".into()]).unwrap();
        assert_eq!(find(&status_of(&repo).unwrap(), "sub/b.txt").staged, None);

        assert!(commit_staged(&mut repo, "   ").is_err());
        commit_staged(&mut repo, "first").unwrap();
        let st = status_of(&repo).unwrap();
        assert!(st.iter().all(|c| c.path != "a.txt"));
        assert_eq!(find(&st, "sub/b.txt").unstaged, Some("new"));
        assert_eq!(repo.head().unwrap().peel_to_commit().unwrap().summary(), Ok(Some("first")));

        // Modify + delete, stage both, then unstage one with HEAD present.
        fs::write(dir.join("a.txt"), "changed").unwrap();
        fs::write(dir.join("c.txt"), "three").unwrap();
        stage(&repo, &["a.txt".into(), "c.txt".into()]).unwrap();
        fs::remove_file(dir.join("c.txt")).unwrap();
        stage(&repo, &["c.txt".into()]).unwrap(); // deletion of a never-committed file clears it
        unstage(&repo, &["a.txt".into()]).unwrap();
        let st = status_of(&repo).unwrap();
        assert_eq!(find(&st, "a.txt").staged, None);
        assert_eq!(find(&st, "a.txt").unstaged, Some("modified"));

        // Nothing staged -> refuse.
        assert_eq!(commit_staged(&mut repo, "again").unwrap_err(), "Nothing staged to commit");

        let _ = fs::remove_dir_all(&dir);
    }
}
