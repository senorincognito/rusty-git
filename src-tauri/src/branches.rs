use git2::{BranchType, Repository};
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
}
