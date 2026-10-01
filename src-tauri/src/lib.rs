mod branches;
mod changes;
mod graph;
mod remotes;
mod repo;
mod sync;
mod terminal;
mod watch;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(watch::RepoWatcher::default())
        .manage(terminal::Terminal::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
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
            terminal::term_start,
            terminal::term_write,
            terminal::term_resize,
            terminal::term_stop,
            watch::watch_repo,
            watch::unwatch_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
