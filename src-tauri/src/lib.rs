mod accounts;
mod backups;
mod codex_home;
mod commands;
mod config_merge;
mod error;
mod io;
mod locations;
mod models;
mod network_check;
mod oauth;
mod paths;
mod process;
mod quota;
mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::import_accounts,
            accounts::list_accounts,
            backups::list_backups,
            commands::switch_account,
            oauth::start_oauth_login,
            oauth::close_oauth_login,
            oauth::complete_oauth_login,
            oauth::refresh_account_tokens,
            network_check::check_oauth_network_exit,
            quota::check_account_quota,
            quota::list_quota_states,
            quota::fetch_codex_usage,
            quota::list_usage_states,
            quota::clear_usage_state,
            backups::backup_current_state,
            backups::restore_backup,
            codex_home::read_current_codex_state,
            locations::read_app_paths,
            locations::open_codex_home_dir,
            locations::open_app_store_dir,
            locations::open_browser_profile_dir,
            locations::open_backups_dir,
            locations::open_backup_dir,
            settings::read_settings,
            settings::update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
