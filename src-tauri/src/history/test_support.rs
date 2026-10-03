//! Helpers shared by the history tests: small repositories with real commits.

use git2::{Oid, Repository, Signature};

pub(crate) fn make(repo: &Repository, who: &str, msg: &str, parents: &[Oid], refname: Option<&str>) -> Oid {
    let sig = Signature::now(who, "t@example.com").unwrap();
    let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
    let ps: Vec<_> = parents.iter().map(|o| repo.find_commit(*o).unwrap()).collect();
    let refs: Vec<_> = ps.iter().collect();
    repo.commit(refname, &sig, &sig, msg, &tree, &refs).unwrap()
}

pub(crate) fn msg(repo: &Repository, oid: Oid) -> String {
    repo.find_commit(oid).unwrap().message().unwrap().trim().to_string()
}

/// Commits `content` as file `name` on HEAD (real files, so the working directory matters).
pub(crate) fn commit_file(repo: &Repository, dir: &std::path::Path, name: &str, content: &str, msg: &str) -> Oid {
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
pub(crate) fn merge_commit(repo: &Repository, first: Oid, second: Oid) -> Oid {
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

pub(crate) fn new_repo(name: &str) -> (std::path::PathBuf, Repository) {
    let dir = std::env::temp_dir().join(format!("gc-drop-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let repo = Repository::init(&dir).unwrap();
    (dir, repo)
}

pub(crate) fn log(repo: &Repository) -> Vec<String> {
    let mut walk = repo.revwalk().unwrap();
    walk.push_head().unwrap();
    walk.map(|o| repo.find_commit(o.unwrap()).unwrap().summary().unwrap().unwrap().to_string()).collect()
}
