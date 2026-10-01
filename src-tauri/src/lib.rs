mod changes;
mod graph;
mod repo;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            changes::get_status,
            changes::stage_paths,
            changes::unstage_paths,
            changes::create_commit,
            graph::get_graph,
            repo::open_repo,
            repo::get_recent_repos,
            repo::remove_recent_repo,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
