use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::account_bundle::{export_account_bundle_to_path, import_account_bundle_from_path};
use crate::accounts::{current_identity_from_auth, load_account};
use crate::backups::create_backup;
use crate::codex_home::{
    account_warnings, capture_codex_desktop_launch_path, close_codex_processes_fast,
    reopen_codex_if_needed,
};
use crate::config_merge::merge_config_files;
use crate::error::{run_blocking, stringify_io, AppResult};
use crate::io::{atomic_write_json, atomic_write_text};
use crate::models::{
    AccountBundleExportResult, AccountBundleImportResult, AccountSummary, AuthJsonExportResult,
    SwitchResult,
};
use crate::paths::{account_dir, app_store_dir};
use crate::settings::load_settings;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const MAX_CURRENT_AUTH_EXPORT_BYTES: u64 = 4 * 1024 * 1024;

#[tauri::command]
pub async fn import_accounts(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
    run_blocking(move || import_accounts_blocking(paths)).await
}

#[tauri::command]
pub async fn import_account_bundle(path: String) -> AppResult<AccountBundleImportResult> {
    run_blocking(move || import_account_bundle_blocking(path)).await
}

#[tauri::command]
pub async fn export_account_bundle(
    account_ids: Vec<String>,
    output_path: String,
) -> AppResult<AccountBundleExportResult> {
    run_blocking(move || export_account_bundle_blocking(account_ids, output_path)).await
}

#[tauri::command]
pub async fn export_current_auth_json(output_path: String) -> AppResult<AuthJsonExportResult> {
    run_blocking(move || export_current_auth_json_blocking(output_path)).await
}

#[tauri::command]
pub async fn delete_account(account_id: String, delete_profile: bool) -> AppResult<()> {
    run_blocking(move || delete_account_blocking(account_id, delete_profile)).await
}

fn import_accounts_blocking(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
    if paths.is_empty() {
        return Err("请选择至少一个导出压缩包。".into());
    }

    let mut imported = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        match path
            .extension()
            .and_then(|item| item.to_str())
            .map(str::to_ascii_lowercase)
        {
            Some(ext) if ext == "zip" => {
                let result = import_account_bundle_blocking(path.to_string_lossy().to_string())?;
                imported.extend(result.imported);
                if !result.failed.is_empty() && imported.is_empty() {
                    let details = result
                        .failed
                        .into_iter()
                        .map(|failure| failure.message)
                        .collect::<Vec<_>>()
                        .join("；");
                    return Err(format!("导入包中没有可用账号：{details}"));
                }
            }
            Some(ext) if ext == "toml" || ext == "json" => {
                return Err(legacy_import_unsupported_message());
            }
            _ => return Err(format!("不支持的文件类型：{}", path.display())),
        }
    }

    if imported.is_empty() {
        return Err("没有找到可导入的账号。".into());
    }

    Ok(imported)
}

fn import_account_bundle_blocking(path: String) -> AppResult<AccountBundleImportResult> {
    let path = PathBuf::from(path);
    match path
        .extension()
        .and_then(|item| item.to_str())
        .map(str::to_ascii_lowercase)
    {
        Some(ext) if ext == "zip" => {}
        Some(ext) if ext == "toml" || ext == "json" => {
            return Err(legacy_import_unsupported_message());
        }
        _ => return Err(format!("不支持的文件类型：{}", path.display())),
    }

    let settings = load_settings()?;
    let accounts_root = app_store_dir()?.join("accounts");
    let profile_root = PathBuf::from(settings.browser_profile_dir);
    import_account_bundle_from_path(&path, &accounts_root, &profile_root)
}

fn export_account_bundle_blocking(
    account_ids: Vec<String>,
    output_path: String,
) -> AppResult<AccountBundleExportResult> {
    if account_ids.is_empty() {
        return Err("请选择至少一个要导出的账号。".to_string());
    }
    let output_path = PathBuf::from(output_path);
    if output_path
        .extension()
        .and_then(|item| item.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("zip"))
    {
        return Err("导出文件必须是 .zip 压缩包。".to_string());
    }

    let mut accounts = Vec::with_capacity(account_ids.len());
    for account_id in account_ids {
        accounts.push(load_account(&account_id)?);
    }
    export_account_bundle_to_path(&accounts, &output_path)
}

fn export_current_auth_json_blocking(output_path: String) -> AppResult<AuthJsonExportResult> {
    let output_path = PathBuf::from(output_path);
    if output_path
        .extension()
        .and_then(|item| item.to_str())
        .is_none_or(|ext| !ext.eq_ignore_ascii_case("zip"))
    {
        return Err("导出文件必须是 .zip 压缩包。".to_string());
    }

    let settings = load_settings()?;
    let auth_path = PathBuf::from(settings.codex_home).join("auth.json");
    if !auth_path.exists() {
        return Err(format!("当前 Codex home 中不存在 auth.json：{}", auth_path.display()));
    }
    if !auth_path.is_file() {
        return Err(format!("auth.json 不是文件：{}", auth_path.display()));
    }
    let metadata = fs::metadata(&auth_path).map_err(stringify_io)?;
    if metadata.len() > MAX_CURRENT_AUTH_EXPORT_BYTES {
        return Err("auth.json 文件过大，已拒绝导出。".to_string());
    }

    let auth_bytes = fs::read(&auth_path).map_err(stringify_io)?;
    let auth_json: Value = serde_json::from_slice(&auth_bytes)
        .map_err(|err| format!("auth.json 不是有效 JSON：{err}"))?;
    let folder_name = current_auth_export_folder_name(&auth_json);
    write_current_auth_zip(&output_path, &folder_name, &auth_bytes)?;

    Ok(AuthJsonExportResult {
        path: output_path.to_string_lossy().to_string(),
        folder_name,
    })
}

fn write_current_auth_zip(
    output_path: &Path,
    folder_name: &str,
    auth_bytes: &[u8],
) -> AppResult<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(stringify_io)?;
    }

    let file = fs::File::create(output_path).map_err(stringify_io)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    writer
        .start_file(format!("{folder_name}/auth.json"), options)
        .map_err(|err| err.to_string())?;
    writer.write_all(auth_bytes).map_err(stringify_io)?;
    writer.finish().map_err(|err| err.to_string())?;
    Ok(())
}

fn current_auth_export_folder_name(auth_json: &Value) -> String {
    let identity = current_identity_from_auth(auth_json);
    let raw = identity
        .email
        .or(identity.account_id)
        .unwrap_or_else(|| "unknown-account".to_string());
    sanitize_auth_export_folder_name(&raw)
}

fn sanitize_auth_export_folder_name(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(ch);
        }
    }

    let safe = out
        .trim()
        .trim_matches(|ch| ch == '.' || ch == '-' || ch == ' ')
        .chars()
        .take(120)
        .collect::<String>();
    let safe = safe
        .trim_end_matches(|ch| ch == '.' || ch == ' ')
        .to_string();

    if safe.is_empty() {
        "unknown-account".to_string()
    } else {
        safe
    }
}

fn legacy_import_unsupported_message() -> String {
    "旧格式 .json / .toml 批量导入已不再支持，请导入 Codex Switch 导出的 .zip 压缩包。".to_string()
}

fn delete_account_blocking(account_id: String, delete_profile: bool) -> AppResult<()> {
    let account = load_account(&account_id)?;
    let account_path = account_dir(&account_id)?;
    let accounts_root = app_store_dir()?.join("accounts");
    ensure_child_path(&accounts_root, &account_path, "账号目录")?;

    if delete_profile {
        if let Some(profile_dir) = account.summary.browser_profile_dir.as_deref() {
            let profile_path = PathBuf::from(profile_dir);
            if profile_path.exists() {
                let profile_root = load_settings()?.browser_profile_dir;
                let profile_root = PathBuf::from(profile_root);
                ensure_child_path(&profile_root, &profile_path, "Profile 目录")?;
                fs::remove_dir_all(&profile_path).map_err(stringify_io)?;
            }
        }
    }

    if account_path.exists() {
        fs::remove_dir_all(account_path).map_err(stringify_io)?;
    }
    Ok(())
}

fn ensure_child_path(root: &PathBuf, target: &PathBuf, label: &str) -> AppResult<()> {
    let root = root.canonicalize().map_err(stringify_io)?;
    let target = if target.exists() {
        target.canonicalize().map_err(stringify_io)?
    } else {
        target
            .parent()
            .ok_or_else(|| format!("{label} 路径无效。"))?
            .canonicalize()
            .map_err(stringify_io)?
            .join(
                target
                    .file_name()
                    .ok_or_else(|| format!("{label} 路径无效。"))?,
            )
    };
    if !target.starts_with(&root) {
        return Err(format!("{label} 不在允许删除的目录内。"));
    }
    Ok(())
}

#[tauri::command]
pub async fn switch_account(account_id: String) -> AppResult<SwitchResult> {
    run_blocking(move || switch_account_blocking(account_id)).await
}

fn switch_account_blocking(account_id: String) -> AppResult<SwitchResult> {
    let settings = load_settings()?;
    let account = load_account(&account_id)?;
    let codex_home = PathBuf::from(&settings.codex_home);
    fs::create_dir_all(&codex_home).map_err(stringify_io)?;

    let mut warnings = account_warnings(&account.summary);
    let codex_reopen_path = capture_codex_desktop_launch_path(&settings);
    close_codex_processes_fast(&settings, &mut warnings);

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

    let codex_reopened = reopen_codex_if_needed(codex_reopen_path.as_deref(), &mut warnings);

    Ok(SwitchResult {
        account: account.summary,
        backup_id: backup.id,
        warnings,
        codex_reopened,
        codex_reopen_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_delete_target_inside_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("accounts");
        let target = root.join("account-1");
        fs::create_dir_all(&target).unwrap();

        assert!(ensure_child_path(&root, &target, "账号目录").is_ok());
    }

    #[test]
    fn rejects_delete_target_outside_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("accounts");
        let target = temp.path().join("profiles").join("account-1");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&target).unwrap();

        assert!(ensure_child_path(&root, &target, "账号目录").is_err());
    }

    #[test]
    fn import_accounts_rejects_legacy_json_and_toml() {
        let temp = tempfile::tempdir().unwrap();
        let json_path = temp.path().join("account.json");
        let toml_path = temp.path().join("account.toml");
        fs::write(&json_path, "{}").unwrap();
        fs::write(&toml_path, "").unwrap();

        let err = import_accounts_blocking(vec![
            json_path.to_string_lossy().to_string(),
            toml_path.to_string_lossy().to_string(),
        ])
        .expect_err("legacy files should be rejected");

        assert!(err.contains("旧格式"));
    }

    #[test]
    fn auth_export_folder_uses_email_and_sanitizes_windows_chars() {
        let auth_json = serde_json::json!({
            "email": r#"bad:name/user?@example.com."#,
            "tokens": {
                "account_id": "acct-1"
            }
        });

        assert_eq!(
            current_auth_export_folder_name(&auth_json),
            "bad-name-user-@example.com"
        );
    }

    #[test]
    fn auth_export_folder_uses_email_from_exported_auth_id_token() {
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;

        let claims = serde_json::json!({
            "email": "real-user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-from-claims"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let auth_json = serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access",
                "id_token": token,
                "refresh_token": "refresh",
                "account_id": "acct-from-token"
            }
        });

        assert_eq!(
            current_auth_export_folder_name(&auth_json),
            "real-user@example.com"
        );
    }

    #[test]
    fn auth_export_folder_falls_back_to_unknown() {
        let auth_json = serde_json::json!({});

        assert_eq!(current_auth_export_folder_name(&auth_json), "unknown-account");
    }
}
