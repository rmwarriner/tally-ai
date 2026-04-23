pub mod commands;
pub mod core;
pub mod crypto;
pub mod db;
pub mod id;
pub mod ai;
pub mod scheduler;
pub mod error;

use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::check_setup_status,
            commands::create_household,
            commands::create_account,
            commands::set_opening_balance,
            commands::create_envelope,
            commands::import_hledger,
            commands::get_ai_defaults,
            commands::undo_last_transaction,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
