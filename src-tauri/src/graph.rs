use std::collections::HashMap;

use git2::{Oid, Repository, Sort};
use serde::Serialize;

const COLOR_COUNT: usize = 8;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefLabel {
    pub name: String,
    /// "branch" | "remote" | "tag"
    pub kind: &'static str,
    pub is_head: bool,
}

/// A line entering the node from the top edge of its row.
#[derive(Serialize)]
pub struct Edge {
    pub col: usize,
    pub color: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphRow {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub author: String,
    pub email: String,
    /// Unix seconds.
    pub time: i64,
    pub parents: Vec<String>,
    pub refs: Vec<RefLabel>,
    pub col: usize,
    pub color: usize,
    /// Lanes ending at this node, drawn from the top edge of the row.
    pub top: Vec<Edge>,
    /// Lanes passing straight through the row.
    pub through: Vec<Edge>,
    /// Lines from this node down to the bottom edge of the row.
    pub bottom: Vec<Edge>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Graph {
    pub rows: Vec<GraphRow>,
    pub max_lanes: usize,
    /// True when history continues beyond the returned rows.
    pub has_more: bool,
}

#[derive(Clone, Copy)]
struct Lane {
    oid: Oid,
    color: usize,
}

fn collect_refs(repo: &Repository) -> Result<(HashMap<Oid, Vec<RefLabel>>, Vec<Oid>), git2::Error> {
    let mut labels: HashMap<Oid, Vec<RefLabel>> = HashMap::new();
    let mut tips = Vec::new();
    let head_name = repo.head().ok().and_then(|h| h.name().ok().map(str::to_string));

    for r in repo.references()?.flatten() {
        let Ok(full) = r.name() else { continue };
        let (kind, short) = if let Some(s) = full.strip_prefix("refs/heads/") {
            ("branch", s)
        } else if let Some(s) = full.strip_prefix("refs/remotes/") {
            if s.ends_with("/HEAD") {
                continue;
            }
            ("remote", s)
        } else if let Some(s) = full.strip_prefix("refs/tags/") {
            ("tag", s)
        } else {
            continue; // stash, notes, etc.
        };
        let Ok(commit) = r.peel_to_commit() else { continue };
        let oid = commit.id();
        tips.push(oid);
        labels.entry(oid).or_default().push(RefLabel {
            name: short.to_string(),
            kind,
            is_head: head_name.as_deref() == Some(full),
        });
    }

    // Detached HEAD isn't covered by a branch ref.
    if let Ok(h) = repo.head() {
        if !h.is_branch() {
            if let Some(oid) = h.target() {
                tips.push(oid);
                labels.entry(oid).or_default().push(RefLabel {
                    name: "HEAD".into(),
                    kind: "branch",
                    is_head: true,
                });
            }
        }
    }

    // Stable order inside a commit: current branch first, then branches, remotes, tags.
    for list in labels.values_mut() {
        list.sort_by_key(|l| {
            (
                !l.is_head,
                match l.kind {
                    "branch" => 0,
                    "remote" => 1,
                    _ => 2,
                },
            )
        });
    }
    Ok((labels, tips))
}

fn first_free(lanes: &mut Vec<Option<Lane>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

fn build_graph(path: &str, limit: usize) -> Result<Graph, String> {
    let repo = Repository::discover(path).map_err(|e| e.message().to_string())?;
    let (mut labels, tips) = collect_refs(&repo).map_err(|e| e.message().to_string())?;

    let mut walk = repo.revwalk().map_err(|e| e.message().to_string())?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)
        .map_err(|e| e.message().to_string())?;
    for tip in tips {
        walk.push(tip).map_err(|e| e.message().to_string())?;
    }

    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color = 0usize;
    let mut new_color = || {
        let c = next_color % COLOR_COUNT;
        next_color += 1;
        c
    };

    let mut rows = Vec::new();
    let mut max_lanes = 0usize;
    let mut has_more = false;

    for oid in walk {
        let oid = oid.map_err(|e| e.message().to_string())?;
        if rows.len() >= limit {
            has_more = true;
            break;
        }
        let commit = repo.find_commit(oid).map_err(|e| e.message().to_string())?;

        let matches: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_some_and(|l| l.oid == oid))
            .map(|(i, _)| i)
            .collect();

        let (col, color) = match matches.first() {
            Some(&i) => (i, lanes[i].unwrap().color),
            None => {
                let i = first_free(&mut lanes);
                (i, new_color())
            }
        };

        let top: Vec<Edge> = matches
            .iter()
            .map(|&i| Edge { col: i, color: lanes[i].unwrap().color })
            .collect();
        let through: Vec<Edge> = lanes
            .iter()
            .enumerate()
            .filter(|(i, l)| l.is_some() && !matches.contains(i))
            .map(|(i, l)| Edge { col: i, color: l.unwrap().color })
            .collect();
        for &i in &matches {
            lanes[i] = None;
        }

        let mut bottom = Vec::new();
        for (n, pid) in commit.parent_ids().enumerate() {
            let existing = lanes.iter().position(|l| l.is_some_and(|l| l.oid == pid));
            match (existing, n) {
                // Parent already has a lane (another branch reaches it): join that lane.
                (Some(j), _) => bottom.push(Edge { col: j, color: lanes[j].unwrap().color }),
                // First parent continues in this node's lane.
                (None, 0) => {
                    lanes[col] = Some(Lane { oid: pid, color });
                    bottom.push(Edge { col, color });
                }
                // Additional parents (merge) open a new lane.
                (None, _) => {
                    let slot = first_free(&mut lanes);
                    let c = new_color();
                    lanes[slot] = Some(Lane { oid: pid, color: c });
                    bottom.push(Edge { col: slot, color: c });
                }
            }
        }

        max_lanes = max_lanes.max(lanes.len()).max(col + 1);
        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }

        let author = commit.author();
        rows.push(GraphRow {
            id: oid.to_string(),
            short_id: oid.to_string()[..7].to_string(),
            summary: commit.summary().ok().flatten().unwrap_or("").to_string(),
            author: author.name().unwrap_or("").to_string(),
            email: author.email().unwrap_or("").to_string(),
            time: commit.time().seconds(),
            parents: commit.parent_ids().map(|p| p.to_string()).collect(),
            refs: labels.remove(&oid).unwrap_or_default(),
            col,
            color,
            top,
            through,
            bottom,
        });
    }

    Ok(Graph { rows, max_lanes, has_more })
}

/// Commit graph over all branches, remotes and tags, newest first, with lane layout.
/// Async so the (blocking) walk runs off the main thread.
#[tauri::command]
pub async fn get_graph(path: String, limit: usize) -> Result<Graph, String> {
    tauri::async_runtime::spawn_blocking(move || build_graph(&path, limit))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;

    fn commit(repo: &Repository, msg: &str, parents: &[Oid], update: Option<&str>) -> Oid {
        let sig = Signature::now("t", "t@example.com").unwrap();
        let tree = repo.find_tree(repo.treebuilder(None).unwrap().write().unwrap()).unwrap();
        let ps: Vec<_> = parents.iter().map(|o| repo.find_commit(*o).unwrap()).collect();
        let refs: Vec<_> = ps.iter().collect();
        repo.commit(update, &sig, &sig, msg, &tree, &refs).unwrap()
    }

    #[test]
    fn branch_and_merge_layout() {
        let dir = std::env::temp_dir().join(format!("gc-graph-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = Repository::init(&dir).unwrap();

        //  M  (merge of A2 and B1)
        //  |\
        //  A2 B1
        //  |/
        //  A1
        let a1 = commit(&repo, "a1", &[], Some("refs/heads/main"));
        let a2 = commit(&repo, "a2", &[a1], Some("refs/heads/main"));
        let b1 = commit(&repo, "b1", &[a1], Some("refs/heads/topic"));
        let m = commit(&repo, "m", &[a2, b1], Some("refs/heads/main"));
        repo.set_head("refs/heads/main").unwrap();

        let g = build_graph(dir.to_str().unwrap(), 100).unwrap();
        let ids: Vec<_> = g.rows.iter().map(|r| r.id.clone()).collect();
        assert_eq!(g.rows.len(), 4);
        assert_eq!(ids[0], m.to_string());
        assert_eq!(ids[3], a1.to_string());
        assert!(!g.has_more);

        // Merge node opens a second lane for its second parent.
        assert_eq!(g.rows[0].bottom.len(), 2);
        assert_eq!(g.max_lanes, 2);
        // The branch commit joins the existing lane of its parent on its own row.
        let b1_row = g.rows.iter().find(|r| r.id == b1.to_string()).unwrap();
        assert_eq!(b1_row.col, 1);
        assert_eq!(b1_row.bottom.len(), 1);
        assert_eq!(b1_row.bottom[0].col, 0);
        // Root commit receives the single remaining lane.
        assert_eq!(g.rows[3].top.len(), 1);
        assert!(g.rows[3].bottom.is_empty());
        // Branch labels land on tips, HEAD flagged.
        assert!(g.rows[0].refs.iter().any(|r| r.name == "main" && r.is_head));
        assert!(g.rows.iter().any(|r| r.refs.iter().any(|l| l.name == "topic")));

        // Limit truncates and reports more history.
        let g = build_graph(dir.to_str().unwrap(), 2).unwrap();
        assert_eq!(g.rows.len(), 2);
        assert!(g.has_more);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
