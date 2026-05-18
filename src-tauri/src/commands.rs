use std::fs;
use std::path::PathBuf;

use crate::accounts::{import_account_json, load_account, matching_config_path};
use crate::backups::create_backup;
use crate::codex_home::{account_warnings, close_codex_processes};
use crate::config_merge::merge_config_files;
use crate::error::{stringify_io, AppResult};
use crate::io::{atomic_write_json, atomic_write_text};
use crate::models::{AccountSummary, SwitchResult};
use crate::paths::{account_dir, app_store_dir};
use crate::settings::load_settings;

#[tauri::command]
pub fn import_accounts(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
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
pub fn switch_account(account_id: String) -> AppResult<SwitchResult> {
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
