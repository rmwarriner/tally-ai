pub mod commands;
pub mod core;
pub mod db;
pub mod ai;
pub mod scheduler;
pub mod error;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
