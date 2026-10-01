use git2::{Delta, DiffFindOptions, DiffOptions, Oid, Repository};
use serde::Serialize;

/// More than this many files are cut off (the total is still reported).
const MAX_FILES: usize = 2000;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    /// Previous path, for renamed and copied files.
    pub old_path: Option<String>,
    /// "new" | "modified" | "deleted" | "renamed" | "copied" | "typechange"
    pub status: &'static str,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    /// Everything after the first line, trimmed; empty for one-line messages.
    pub body: String,
    pub author: String,
    pub email: String,
    /// Unix seconds.
    pub time: i64,
    /// Short ids of the parents.
    pub parents: Vec<String>,
    /// Changes are shown relative to the first parent, like most Git GUIs do for merges.
    pub is_merge: bool,
    pub files: Vec<CommitFile>,
    pub total_files: usize,
    pub truncated: bool,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

fn commit_detail(repo: &Repository, id: &str) -> Result<CommitDetail, String> {
    let oid = Oid::from_str(id).map_err(err)?;
    let commit = repo.find_commit(oid).map_err(err)?;
    let tree = commit.tree().map_err(err)?;
    // The root commit has no parent: everything in it counts as added.
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = DiffOptions::new();
    opts.include_typechange(true);
    let mut diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))
        .map_err(err)?;
    let mut find = DiffFindOptions::new();
    find.renames(true);
    diff.find_similar(Some(&mut find)).map_err(err)?;

    let mut files: Vec<CommitFile> = diff
        .deltas()
        .filter_map(|d| {
            let status = match d.status() {
                Delta::Added => "new",
                Delta::Deleted => "deleted",
                Delta::Modified => "modified",
                Delta::Renamed => "renamed",
                Delta::Copied => "copied",
                Delta::Typechange => "typechange",
                _ => return None,
            };
            let new = d.new_file().path();
            let old = d.old_file().path();
            let path = new.or(old)?.to_string_lossy().replace('\\', "/");
            let old_path = match d.status() {
                Delta::Renamed | Delta::Copied => old.map(|p| p.to_string_lossy().replace('\\', "/")),
                _ => None,
            };
            Some(CommitFile { path, old_path, status })
        })
        .collect();
    files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()).then_with(|| a.path.cmp(&b.path)));
    let total_files = files.len();
    files.truncate(MAX_FILES);

    let message = String::from_utf8_lossy(commit.message_raw_bytes()).into_owned();
    let message = message.trim_end();
    let (summary, body) = match message.split_once('\n') {
        Some((first, rest)) => (first.trim_end().to_string(), rest.trim().to_string()),
        None => (message.to_string(), String::new()),
    };
    let author = commit.author();

    Ok(CommitDetail {
        id: oid.to_string(),
        short_id: oid.to_string()[..7].to_string(),
        summary,
        body,
        author: author.name().unwrap_or("").to_string(),
        email: author.email().unwrap_or("").to_string(),
        time: commit.time().seconds(),
        parents: commit.parent_ids().map(|p| p.to_string()[..7].to_string()).collect(),
        is_merge: commit.parent_count() > 1,
        truncated: total_files > files.len(),
        files,
        total_files,
    })
}

/// The message, author and changed files of a commit.
#[tauri::command]
pub async fn get_commit_detail(path: String, id: String) -> Result<CommitDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        commit_detail(&Repository::discover(&path).map_err(err)?, &id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{IndexAddOption, Signature};
    use std::fs;

    /// Commits the current working-tree state (additions, edits and deletions).
    fn commit_all(repo: &Repository, msg: &str, parents: &[Oid]) -> Oid {
        let mut index = repo.index().unwrap();
        index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
        index.update_all(["*"], None).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig = Signature::now("Ann", "ann@example.com").unwrap();
        let ps: Vec<_> = parents.iter().map(|o| repo.find_commit(*o).unwrap()).collect();
        let refs: Vec<_> = ps.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs).unwrap()
    }

    fn statuses(d: &CommitDetail) -> Vec<(String, &'static str)> {
        d.files.iter().map(|f| (f.path.clone(), f.status)).collect()
    }

    #[test]
    fn lists_changed_files_with_renames() {
        let dir = std::env::temp_dir().join(format!("gc-detail-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let body: String = (0..30).map(|i| format!("line {i}\n")).collect();

        // Root commit: everything is new, files sorted, nested paths use forward slashes.
        fs::write(dir.join("b.txt"), "old").unwrap();
        fs::write(dir.join("a.txt"), "one").unwrap();
        fs::create_dir(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("big.txt"), &body).unwrap();
        let c1 = commit_all(&repo, "Initial\n\nFirst paragraph.\nSecond line.", &[]);
        let d = commit_detail(&repo, &c1.to_string()).unwrap();
        assert_eq!(
            statuses(&d),
            [("a.txt".into(), "new"), ("b.txt".into(), "new"), ("src/big.txt".into(), "new")]
        );
        assert_eq!((d.summary.as_str(), d.body.as_str()), ("Initial", "First paragraph.\nSecond line."));
        assert_eq!((d.author.as_str(), d.short_id.len(), d.parents.len(), d.is_merge), ("Ann", 7, 0, false));

        // Modify, delete, add and rename (identical content moves) in one commit.
        fs::write(dir.join("a.txt"), "two").unwrap();
        fs::remove_file(dir.join("b.txt")).unwrap();
        fs::rename(dir.join("src").join("big.txt"), dir.join("moved.txt")).unwrap();
        fs::write(dir.join("c.txt"), "new file").unwrap();
        let c2 = commit_all(&repo, "Change things", &[c1]);
        let d = commit_detail(&repo, &c2.to_string()).unwrap();
        assert_eq!(
            statuses(&d),
            [("a.txt".into(), "modified"), ("b.txt".into(), "deleted"), ("c.txt".into(), "new"), ("moved.txt".into(), "renamed")]
        );
        let renamed = d.files.iter().find(|f| f.status == "renamed").unwrap();
        assert_eq!(renamed.old_path.as_deref(), Some("src/big.txt"));
        assert_eq!(d.body, "");
        assert_eq!((d.total_files, d.truncated), (4, false));
        assert_eq!(d.parents, [c1.to_string()[..7].to_string()]);

        // Merge commits are flagged and diffed against the first parent.
        let side = repo.commit(
            None,
            &Signature::now("Ann", "a@e.com").unwrap(),
            &Signature::now("Ann", "a@e.com").unwrap(),
            "side",
            &repo.find_commit(c1).unwrap().tree().unwrap(),
            &[&repo.find_commit(c1).unwrap()],
        ).unwrap();
        let m = commit_all(&repo, "Merge", &[c2, side]);
        let d = commit_detail(&repo, &m.to_string()).unwrap();
        assert!(d.is_merge && d.parents.len() == 2);

        assert!(commit_detail(&repo, "not-an-id").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
