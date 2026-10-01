mod branches;
mod changes;
mod commit_detail;
mod graph;
mod history;
mod hunks;
mod remotes;
mod repo;
mod reset;
mod stash;
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
            commit_detail::get_working_diff,
            changes::stage_paths,
            changes::unstage_paths,
            changes::discard_paths,
            changes::create_commit,
            graph::get_graph,
            history::drop_latest_commit,
            history::get_drop_info,
            history::get_rename_info,
            hunks::discard_hunk_cmd,
            hunks::stage_hunk_cmd,
            hunks::unstage_hunk_cmd,
            history::rename_commit_message,
            repo::open_repo,
            repo::get_recent_repos,
            repo::remove_recent_repo,
            reset::get_reset_info,
            reset::reset_to_commit,
            remotes::count_unmerged_remote_commits,
            remotes::delete_remote_branch,
            remotes::get_remotes,
            remotes::rename_remote_branch,
            remotes::add_remote_cmd,
            remotes::set_remote_url_cmd,
            remotes::delete_remote_cmd,
            remotes::set_target_remote,
            stash::create_stash,
            stash::get_stashes,
            stash::pop_stash_cmd,
            stash::stash_paths_cmd,
            stash::drop_stash_cmd,
            sync::get_divergence,
            sync::get_sync_status,
            sync::git_fetch,
            sync::git_auto_fetch,
            sync::git_force_push,
            sync::git_pull,
            sync::git_pull_with,
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
