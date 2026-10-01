mod branches;
mod changes;
mod commit_detail;
mod graph;
mod history;
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
            branches::checkout_local_branch,
            branches::count_unmerged_commits,
            branches::create_branch,
            branches::delete_local_branch,
            branches::get_local_branches,
            branches::rename_local_branch,
            changes::get_head_commit,
            changes::get_status,
            commit_detail::get_commit_detail,
            commit_detail::get_file_diff,
            changes::stage_paths,
            changes::unstage_paths,
            changes::create_commit,
            graph::get_graph,
            history::get_rename_info,
            history::rename_commit_message,
            repo::open_repo,
            repo::get_recent_repos,
            repo::remove_recent_repo,
            remotes::count_unmerged_remote_commits,
            remotes::delete_remote_branch,
            remotes::get_origin,
            remotes::rename_remote_branch,
            remotes::add_origin_remote,
            sync::get_sync_status,
            sync::git_fetch,
            sync::git_auto_fetch,
            sync::git_force_push,
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
