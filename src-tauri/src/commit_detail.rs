use git2::{Delta, DiffFindOptions, DiffFormat, DiffOptions, Oid, Repository};
use serde::Serialize;

/// More than this many files are cut off (the total is still reported).
const MAX_FILES: usize = 2000;
/// A file diff longer than this many lines is cut off.
const MAX_DIFF_LINES: usize = 20_000;
/// Files above this size are treated like binary files and not shown.
const MAX_DIFF_BYTES: i64 = 5 * 1024 * 1024;
/// Enough context to cover any file: the diff then contains the whole file.
const FULL_FILE_CONTEXT: u32 = 1_000_000;

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

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    /// "ctx" | "add" | "del" | "hunk" (a hunk header) | "note" (e.g. no newline at end of file)
    pub kind: &'static str,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileDiff {
    pub lines: Vec<DiffLine>,
    /// Binary or too large to show.
    pub binary: bool,
    /// The diff was cut off after the maximum number of lines.
    pub truncated: bool,
    pub additions: usize,
    pub deletions: usize,
}

/// Options shared by every file diff: literal paths (names with [, * or ? are not patterns),
/// type changes reported, and either the whole file or three lines of context.
fn diff_options(path: &str, old_path: Option<&str>, full_file: bool) -> DiffOptions {
    let mut opts = DiffOptions::new();
    opts.include_typechange(true)
        .context_lines(if full_file { FULL_FILE_CONTEXT } else { 3 })
        .max_size(MAX_DIFF_BYTES)
        .disable_pathspec_match(true)
        .pathspec(path);
    if let Some(old) = old_path {
        opts.pathspec(old);
    }
    opts
}

/// Turns a git diff into the line list the file view draws.
fn render_diff(diff: &git2::Diff, full_file: bool) -> Result<FileDiff, String> {
    let mut out = FileDiff { lines: Vec::new(), binary: false, truncated: false, additions: 0, deletions: 0 };
    diff.print(DiffFormat::Patch, |delta, _hunk, line| {
        if delta.flags().is_binary() || line.origin() == 'B' {
            out.binary = true;
            return true;
        }
        let kind = match line.origin() {
            '+' => {
                out.additions += 1;
                "add"
            }
            '-' => {
                out.deletions += 1;
                "del"
            }
            ' ' => "ctx",
            'H' if !full_file => "hunk",
            '>' | '<' | '=' => "note",
            _ => return true, // file headers, and the single hunk header of a full-file diff
        };
        if out.lines.len() >= MAX_DIFF_LINES {
            out.truncated = true;
            return true; // keep counting additions and deletions
        }
        let text = match kind {
            "note" => "No newline at end of file".to_string(),
            _ => String::from_utf8_lossy(line.content()).trim_end_matches(['\n', '\r']).to_string(),
        };
        out.lines.push(DiffLine { kind, old_no: line.old_lineno(), new_no: line.new_lineno(), text });
        true
    })
    .map_err(err)?;

    if out.binary {
        out.lines.clear();
    }
    Ok(out)
}

/// A file's uncommitted changes. `staged` compares HEAD with the index (what the next commit
/// would contain); otherwise the index with the file on disk (what staging would add), where an
/// untracked file shows up as all additions.
fn working_diff(repo: &Repository, path: &str, staged: bool, full_file: bool) -> Result<FileDiff, String> {
    let mut opts = diff_options(path, None, full_file);
    let index = repo.index().map_err(err)?;
    let diff = if staged {
        // On a branch without commits there is no HEAD tree: everything staged counts as added.
        let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), Some(&mut opts))
    } else {
        opts.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
        repo.diff_index_to_workdir(Some(&index), Some(&mut opts))
    }
    .map_err(err)?;
    render_diff(&diff, full_file)
}

/// What one file of a commit changed, against the first parent. With `full_file` the whole file
/// is returned with the added and removed lines marked in place; otherwise only the changed
/// hunks with three lines of context.
fn file_diff(
    repo: &Repository,
    id: &str,
    path: &str,
    old_path: Option<&str>,
    full_file: bool,
) -> Result<FileDiff, String> {
    let oid = Oid::from_str(id).map_err(err)?;
    let commit = repo.find_commit(oid).map_err(err)?;
    let tree = commit.tree().map_err(err)?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

    let mut opts = diff_options(path, old_path, full_file);
    let mut diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))
        .map_err(err)?;
    if old_path.is_some() {
        let mut find = DiffFindOptions::new();
        find.renames(true);
        diff.find_similar(Some(&mut find)).map_err(err)?;
    }

    render_diff(&diff, full_file)
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

/// One file of a commit as a line-by-line diff, for the file view.
#[tauri::command]
pub async fn get_file_diff(
    path: String,
    id: String,
    file: String,
    old_path: Option<String>,
    full_file: bool,
) -> Result<FileDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        file_diff(&Repository::discover(&path).map_err(err)?, &id, &file, old_path.as_deref(), full_file)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// A file's uncommitted changes, staged or not, as a line diff for the file view.
#[tauri::command]
pub async fn get_working_diff(path: String, file: String, staged: bool, full_file: bool) -> Result<FileDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        working_diff(&Repository::discover(&path).map_err(err)?, &file, staged, full_file)
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

    #[test]
    fn file_diff_marks_changes_in_place() {
        let dir = std::env::temp_dir().join(format!("gc-filediff-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let lines = |n: usize| -> Vec<String> { (1..=n).map(|i| format!("line {i}")).collect() };
        let write = |name: &str, v: &[String]| fs::write(dir.join(name), v.join("\n") + "\n").unwrap();

        // Root commit: a new file is all additions.
        write("a.txt", &lines(40));
        write("old.txt", &lines(30));
        let c1 = commit_all(&repo, "one", &[]);
        let d = file_diff(&repo, &c1.to_string(), "a.txt", None, true).unwrap();
        assert_eq!((d.lines.len(), d.additions, d.deletions), (40, 40, 0));
        assert!(d.lines.iter().all(|l| l.kind == "add" && l.old_no.is_none()));
        assert_eq!(d.lines[0].new_no, Some(1));

        // Edit line 2, delete nothing, append at the end, and rename+edit old.txt.
        let mut v = lines(40);
        v[1] = "line 2 changed".into();
        v[38] = "line 39 changed".into();
        write("a.txt", &v);
        let mut moved = lines(30);
        moved[10] = "line 11 changed".into();
        fs::remove_file(dir.join("old.txt")).unwrap();
        write("new name.txt", &moved);
        fs::write(dir.join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
        let c2 = commit_all(&repo, "two", &[c1]);
        let id = c2.to_string();

        // Full file: everything is there, changes marked in place with both line numbers.
        let full = file_diff(&repo, &id, "a.txt", None, true).unwrap();
        assert_eq!((full.additions, full.deletions, full.lines.len()), (2, 2, 42));
        assert!(full.lines.iter().all(|l| l.kind != "hunk"));
        let del = full.lines.iter().find(|l| l.kind == "del" && l.text == "line 2").unwrap();
        assert_eq!((del.old_no, del.new_no), (Some(2), None));
        let add = full.lines.iter().find(|l| l.kind == "add" && l.text == "line 2 changed").unwrap();
        assert_eq!((add.old_no, add.new_no), (None, Some(2)));
        let last = full.lines.last().unwrap();
        assert_eq!((last.kind, last.old_no, last.new_no), ("ctx", Some(40), Some(40)));

        // Changes only: two separate hunks, far fewer lines.
        let part = file_diff(&repo, &id, "a.txt", None, false).unwrap();
        assert_eq!(part.lines.iter().filter(|l| l.kind == "hunk").count(), 2);
        assert!(part.lines.len() < full.lines.len());
        assert_eq!((part.additions, part.deletions), (2, 2));

        // A rename with an edit diffs against the old path; names with special characters work.
        let r = file_diff(&repo, &id, "new name.txt", Some("old.txt"), true).unwrap();
        assert_eq!((r.additions, r.deletions), (1, 1));

        // Binary files are flagged, not shown.
        let b = file_diff(&repo, &id, "bin.dat", None, true).unwrap();
        assert!(b.binary && b.lines.is_empty());

        assert!(file_diff(&repo, "nope", "a.txt", None, true).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn working_diff_shows_staged_and_unstaged_changes() {
        let dir = std::env::temp_dir().join(format!("gc-workdiff-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();
        let text = |lines: &[&str]| lines.join("\n") + "\n";
        let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));

        // A branch without commits: a staged new file is all additions.
        fs::write(&a, text(&["one", "two", "three"])).unwrap();
        fs::write(&b, "keep").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let d = working_diff(&repo, "a.txt", true, true).unwrap();
        assert_eq!((d.additions, d.deletions, d.lines.len()), (3, 0, 3));
        // ...and an untracked file is all additions in the unstaged view.
        let d = working_diff(&repo, "b.txt", false, true).unwrap();
        assert_eq!((d.additions, d.deletions), (1, 0));
        assert_eq!(d.lines[0].new_no, Some(1));
        assert_eq!(working_diff(&repo, "b.txt", true, true).unwrap().lines.len(), 0);

        // Commit a.txt, then edit it: the change is unstaged only.
        commit_all(&repo, "base", &[]);
        fs::write(&a, text(&["one", "TWO", "three"])).unwrap();
        let un = working_diff(&repo, "a.txt", false, true).unwrap();
        assert_eq!((un.additions, un.deletions, un.lines.len()), (1, 1, 4));
        assert!(un.lines.iter().any(|l| l.kind == "del" && l.text == "two" && l.old_no == Some(2)));
        assert!(un.lines.iter().any(|l| l.kind == "add" && l.text == "TWO" && l.new_no == Some(2)));
        assert_eq!(working_diff(&repo, "a.txt", true, true).unwrap().lines.len(), 0);

        // Stage it: now it shows up staged, and nothing is left unstaged.
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("a.txt")).unwrap();
        index.write().unwrap();
        let st = working_diff(&repo, "a.txt", true, true).unwrap();
        assert_eq!((st.additions, st.deletions), (1, 1));
        assert_eq!(working_diff(&repo, "a.txt", false, true).unwrap().lines.len(), 0);

        // Edit again after staging: both views have their own, different, diff.
        fs::write(&a, text(&["one", "TWO", "three", "four"])).unwrap();
        let un = working_diff(&repo, "a.txt", false, true).unwrap();
        assert_eq!((un.additions, un.deletions), (1, 0));
        let st = working_diff(&repo, "a.txt", true, true).unwrap();
        assert_eq!((st.additions, st.deletions), (1, 1));

        // A deleted file is all removals; a missing path just has no changes.
        fs::remove_file(&b).unwrap();
        let _ = working_diff(&repo, "b.txt", false, true).unwrap();
        assert_eq!(working_diff(&repo, "nope.txt", false, true).unwrap().lines.len(), 0);

        // Binary files are flagged, not shown.
        fs::write(dir.join("bin.dat"), [0u8, 1, 2, 0, 255]).unwrap();
        let bin = working_diff(&repo, "bin.dat", false, true).unwrap();
        assert!(bin.binary && bin.lines.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
