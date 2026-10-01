use git2::{BranchType, Repository};
use serde::Serialize;

const ORIGIN: &str = "origin";

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    /// Remote-tracking branches without the "origin/" prefix, sorted case-insensitively.
    pub branches: Vec<String>,
}

fn err(e: git2::Error) -> String {
    e.message().to_string()
}

/// The `origin` remote, or None if the repo doesn't have one.
fn origin_info(repo: &Repository) -> Result<Option<RemoteInfo>, String> {
    let remote = match repo.find_remote(ORIGIN) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let url = remote.url().unwrap_or("").to_string();

    let prefix = format!("{ORIGIN}/");
    let mut branches = Vec::new();
    for item in repo.branches(Some(BranchType::Remote)).map_err(err)? {
        let (branch, _) = item.map_err(err)?;
        let Some(name) = branch.name().ok().flatten() else { continue };
        if let Some(short) = name.strip_prefix(&prefix) {
            if short != "HEAD" {
                branches.push(short.to_string());
            }
        }
    }
    branches.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));

    Ok(Some(RemoteInfo { name: ORIGIN.into(), url, branches }))
}

fn add_origin(repo: &Repository, url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Enter the repository URL".into());
    }
    if url.chars().any(char::is_whitespace) {
        return Err("The URL must not contain spaces".into());
    }
    match repo.remote(ORIGIN, url) {
        Ok(_) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            Err("A remote named origin already exists".into())
        }
        Err(e) => Err(err(e)),
    }
}

async fn blocking<T: Send + 'static>(
    path: String,
    f: impl FnOnce(&Repository) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || f(&Repository::discover(&path).map_err(err)?))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_origin(path: String) -> Result<Option<RemoteInfo>, String> {
    blocking(path, origin_info).await
}

#[tauri::command]
pub async fn add_origin_remote(path: String, url: String) -> Result<(), String> {
    blocking(path, move |r| add_origin(r, &url)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    #[test]
    fn add_and_describe_origin() {
        let dir = std::env::temp_dir().join(format!("gc-remotes-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();

        assert!(origin_info(&repo).unwrap().is_none());

        assert!(add_origin(&repo, "   ").is_err());
        assert!(add_origin(&repo, "https://example.com/a b.git").is_err());
        add_origin(&repo, "  https://example.com/a.git ").unwrap();
        assert_eq!(
            add_origin(&repo, "https://example.com/other.git").unwrap_err(),
            "A remote named origin already exists"
        );

        let info = origin_info(&repo).unwrap().unwrap();
        assert_eq!(info.url, "https://example.com/a.git");
        assert!(info.branches.is_empty());

        // Remote-tracking refs: sorted, HEAD hidden, other remotes ignored.
        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let c = repo.commit(Some("refs/heads/main"), &sig, &sig, "c", &tree, &[]).unwrap();
        for r in [
            "refs/remotes/origin/main",
            "refs/remotes/origin/Beta",
            "refs/remotes/origin/alpha",
            "refs/remotes/origin/HEAD",
            "refs/remotes/upstream/zzz",
        ] {
            repo.reference(r, c, true, "test").unwrap();
        }
        let info = origin_info(&repo).unwrap().unwrap();
        assert_eq!(info.branches, ["alpha", "Beta", "main"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
