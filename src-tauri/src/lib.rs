mod accounts;
mod backups;
mod codex_home;
mod config_merge;
mod error;
mod io;
mod models;
mod oauth;
mod paths;
mod quota;
mod settings;

use std::fs;
use std::path::PathBuf;

use accounts::{import_account_json, load_account, matching_config_path};
use backups::create_backup;
use codex_home::{account_warnings, close_codex_processes};
use config_merge::merge_config_files;
use error::{stringify_io, AppResult};
use io::{atomic_write_json, atomic_write_text};
use models::{AccountSummary, SwitchResult};
use paths::{account_dir, app_store_dir};
use settings::load_settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            import_accounts,
            accounts::list_accounts,
            backups::list_backups,
            switch_account,
            oauth::start_oauth_login,
            oauth::close_oauth_login,
            oauth::complete_oauth_login,
            oauth::refresh_account_tokens,
            quota::check_account_quota,
            quota::list_quota_states,
            quota::fetch_codex_usage,
            quota::list_usage_states,
            quota::clear_usage_state,
            backups::backup_current_state,
            backups::restore_backup,
            codex_home::read_current_codex_state,
            settings::read_settings,
            settings::update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn import_accounts(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
    if paths.is_empty() {
        return Err("请选择至少一个账号文件。".into());
    }

    let store = app_store_dir()?;
    let accounts_dir = store.join("accounts");
    fs::create_dir_all(&accounts_dir).map_err(stringify_io)?;

    let mut imported = Vec::new();
    let mut json_paths = Vec::new();
    let mut toml_paths = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        match path
            .extension()
            .and_then(|item| item.to_str())
            .map(str::to_ascii_lowercase)
        {
            Some(ext) if ext == "toml" => toml_paths.push(path),
            Some(ext) if ext == "json" => json_paths.push(path),
            _ => return Err(format!("不支持的文件类型：{}", path.display())),
        }
    }

    for path in json_paths {
        let matching_config = matching_config_path(&path, &toml_paths);
        let account = import_account_json(&path, matching_config.as_deref(), &accounts_dir)?;
        imported.push(account);
    }

    if imported.is_empty() {
        return Err("没有找到可导入的 JSON 账号文件。".into());
    }

    Ok(imported)
}

#[tauri::command]
fn switch_account(account_id: String) -> AppResult<SwitchResult> {
    let settings = load_settings()?;
    let account = load_account(&account_id)?;
    let codex_home = PathBuf::from(&settings.codex_home);
    fs::create_dir_all(&codex_home).map_err(stringify_io)?;

    let mut warnings = account_warnings(&account.summary);
    close_codex_processes(&settings, &mut warnings);

    let backup = create_backup(&settings)?;
    let target_config_path = account_dir(&account_id)?.join("config.toml");
    let current_config_path = codex_home.join("config.toml");
    let merged_config = merge_config_files(&current_config_path, &target_config_path)?;

    let auth_path = codex_home.join("auth.json");
    let rollback_auth = fs::read_to_string(&auth_path).ok();
    let rollback_config = fs::read_to_string(&current_config_path).ok();

    if let Err(err) = atomic_write_json(&auth_path, &account.auth_json)
        .and_then(|_| atomic_write_text(&current_config_path, &merged_config))
    {
        if let Some(contents) = rollback_auth {
            let _ = atomic_write_text(&auth_path, &contents);
        }
        if let Some(contents) = rollback_config {
            let _ = atomic_write_text(&current_config_path, &contents);
        }
        return Err(format!("切换失败，已尝试回滚：{err}"));
    }

    Ok(SwitchResult {
        account: account.summary,
        backup_id: backup.id,
        warnings,
    })
}
