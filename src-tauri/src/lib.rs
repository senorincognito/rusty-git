mod branches;
mod changes;
mod graph;
mod remotes;
mod repo;
mod sync;
mod watch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(watch::RepoWatcher::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            branches::checkout_local_branch,
            branches::create_branch,
            branches::get_local_branches,
            changes::get_status,
            changes::stage_paths,
            changes::unstage_paths,
            changes::create_commit,
            graph::get_graph,
            repo::open_repo,
            repo::get_recent_repos,
            repo::remove_recent_repo,
            remotes::get_origin,
            remotes::add_origin_remote,
            sync::get_sync_status,
            sync::git_fetch,
            sync::git_pull,
            sync::git_push,
            watch::watch_repo,
            watch::unwatch_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
