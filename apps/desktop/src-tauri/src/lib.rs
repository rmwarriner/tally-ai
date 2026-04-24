pub mod commands;
pub mod core;
pub mod crypto;
pub mod db;
pub mod id;
pub mod ai;
pub mod chat;
pub mod scheduler;
pub mod secrets;
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
            commands::append_chat_message,
            commands::list_chat_messages,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::submit_message,
            commands::commit_proposal,
            commands::get_account_balances,
            commands::get_current_envelope_periods,
            commands::get_pending_transactions,
            commands::read_gnucash_file,
            commands::gnucash_build_default_plan,
            commands::gnucash_apply_mapping_edit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
