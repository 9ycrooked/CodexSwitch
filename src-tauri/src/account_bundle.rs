use crate::accounts::{normalize_auth_json, now_string, sanitize_id, save_account_record_to_dir};
use crate::error::{stringify_io, AppResult};
use crate::models::{
    AccountBundleExportResult, AccountBundleImportFailure, AccountBundleImportResult,
    AccountSummary, StoredAccount,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const BUNDLE_FORMAT: &str = "codex-switch.account-bundle";
const BUNDLE_VERSION: u32 = 1;
const MANIFEST_PATH: &str = "manifest.json";
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_AUTH_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ACCOUNTS: usize = 200;
const MAX_PROFILE_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct AccountBundleManifest {
    format: String,
    version: u32,
    exported_at: String,
    accounts: Vec<AccountBundleManifestAccount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccountBundleManifestAccount {
    id: String,
    display_name: String,
    email: Option<String>,
    account_id: Option<String>,
    auth_path: String,
    auth_sha256: String,
    profile_path: Option<String>,
}

pub(crate) fn export_account_bundle_to_path(
    accounts: &[StoredAccount],
    output_path: &Path,
) -> AppResult<AccountBundleExportResult> {
    if accounts.is_empty() {
        return Err("请选择至少一个要导出的账号。".to_string());
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(stringify_io)?;
    }

    let file = fs::File::create(output_path).map_err(stringify_io)?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut manifest_accounts = Vec::with_capacity(accounts.len());
    let mut included_profile_count = 0;

    for account in accounts {
        let id = sanitize_id(&account.summary.id);
        let auth_path = format!("accounts/{id}/auth.json");
        validate_bundle_entry_name(&auth_path)?;
        let auth_bytes =
            serde_json::to_vec_pretty(&account.auth_json).map_err(|err| err.to_string())?;
        let auth_sha256 = sha256_hex(&auth_bytes);

        writer
            .start_file(&auth_path, options)
            .map_err(|err| err.to_string())?;
        writer.write_all(&auth_bytes).map_err(stringify_io)?;

        let mut profile_path = None;
        if let Some(raw_profile_dir) = account.summary.browser_profile_dir.as_deref() {
            let profile_dir = PathBuf::from(raw_profile_dir);
            if profile_dir.is_dir() {
                let zip_profile_path = format!("profiles/{id}");
                write_profile_files(&mut writer, &profile_dir, &zip_profile_path, options)?;
                profile_path = Some(zip_profile_path);
                included_profile_count += 1;
            }
        }

        manifest_accounts.push(AccountBundleManifestAccount {
            id,
            display_name: account.summary.display_name.clone(),
            email: account.summary.email.clone(),
            account_id: account.summary.account_id.clone(),
            auth_path,
            auth_sha256,
            profile_path,
        });
    }

    let manifest = AccountBundleManifest {
        format: BUNDLE_FORMAT.to_string(),
        version: BUNDLE_VERSION,
        exported_at: now_string(),
        accounts: manifest_accounts,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|err| err.to_string())?;
    writer
        .start_file(MANIFEST_PATH, options)
        .map_err(|err| err.to_string())?;
    writer.write_all(&manifest_bytes).map_err(stringify_io)?;
    writer.finish().map_err(|err| err.to_string())?;

    Ok(AccountBundleExportResult {
        path: output_path.to_string_lossy().to_string(),
        exported_count: accounts.len(),
        included_profile_count,
    })
}

pub(crate) fn import_account_bundle_from_path(
    bundle_path: &Path,
    accounts_root: &Path,
    profile_root: &Path,
) -> AppResult<AccountBundleImportResult> {
    let file = fs::File::open(bundle_path).map_err(stringify_io)?;
    let mut archive = ZipArchive::new(file).map_err(|err| format!("导入包不是有效 ZIP：{err}"))?;
    validate_archive_entry_names(&mut archive)?;
    let manifest = read_manifest(&mut archive)?;
    validate_manifest(&manifest)?;

    fs::create_dir_all(accounts_root).map_err(stringify_io)?;
    fs::create_dir_all(profile_root).map_err(stringify_io)?;

    let mut imported = Vec::new();
    let mut failed = Vec::new();
    let mut seen_ids = HashSet::new();

    for entry in manifest.accounts {
        let id = sanitize_id(&entry.id);
        if !seen_ids.insert(id.clone()) {
            failed.push(import_failure(&entry, "导入包包含重复账号 id。"));
            continue;
        }

        match import_manifest_account(&mut archive, accounts_root, profile_root, &entry) {
            Ok(summary) => imported.push(summary),
            Err(message) => failed.push(import_failure(&entry, &message)),
        }
    }

    Ok(AccountBundleImportResult { imported, failed })
}

fn import_manifest_account<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    accounts_root: &Path,
    profile_root: &Path,
    entry: &AccountBundleManifestAccount,
) -> AppResult<AccountSummary> {
    let id = sanitize_id(&entry.id);
    let expected_auth_path = format!("accounts/{id}/auth.json");
    if entry.auth_path != expected_auth_path {
        return Err("manifest 中的 auth 路径和账号 id 不匹配。".to_string());
    }
    validate_bundle_entry_name(&entry.auth_path)?;
    if let Some(profile_path) = entry.profile_path.as_deref() {
        let expected_profile_path = format!("profiles/{id}");
        if profile_path != expected_profile_path {
            return Err("manifest 中的 profile 路径和账号 id 不匹配。".to_string());
        }
        validate_bundle_entry_name(&format!("{profile_path}/profile"))?;
    }

    let auth_bytes = read_zip_file(archive, &entry.auth_path, MAX_AUTH_BYTES)?;
    let actual_hash = sha256_hex(&auth_bytes);
    if actual_hash != entry.auth_sha256 {
        return Err(format!(
            "auth.json SHA-256 校验失败，manifest={} 实际={actual_hash}",
            entry.auth_sha256
        ));
    }
    let raw_auth: Value = serde_json::from_slice(&auth_bytes)
        .map_err(|err| format!("auth.json JSON 解析失败：{err}"))?;
    let (auth_json, mut summary) = normalize_auth_json(&raw_auth)?;
    summary.imported_at = now_string();
    summary.has_config = false;
    summary.quota_state = None;
    summary.usage_state = None;
    if summary.id.trim().is_empty() {
        summary.id = id.clone();
    }
    summary.id = sanitize_id(&summary.id);
    if summary.display_name.trim().is_empty() {
        summary.display_name = entry.display_name.clone();
    }

    if entry.profile_path.is_some() {
        let target_profile_dir = profile_root.join(&summary.id);
        replace_profile_from_archive(archive, entry, &target_profile_dir)?;
        summary.browser_profile_dir = Some(target_profile_dir.to_string_lossy().to_string());
    } else {
        summary.browser_profile_dir = None;
    }

    save_account_record_to_dir(accounts_root, &summary, &auth_json, &raw_auth)?;
    Ok(summary)
}

fn import_failure(
    entry: &AccountBundleManifestAccount,
    message: &str,
) -> AccountBundleImportFailure {
    AccountBundleImportFailure {
        id: Some(entry.id.clone()),
        path: Some(entry.auth_path.clone()),
        message: message.to_string(),
    }
}

fn read_manifest<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> AppResult<AccountBundleManifest> {
    let bytes = read_zip_file(archive, MANIFEST_PATH, MAX_MANIFEST_BYTES)?;
    serde_json::from_slice(&bytes).map_err(|err| format!("manifest.json 解析失败：{err}"))
}

fn validate_manifest(manifest: &AccountBundleManifest) -> AppResult<()> {
    if manifest.format != BUNDLE_FORMAT {
        return Err("导入包格式不匹配，请选择 Codex Switch 导出的压缩包。".to_string());
    }
    if manifest.version != BUNDLE_VERSION {
        return Err(format!(
            "导入包版本不支持：{}，当前仅支持版本 {BUNDLE_VERSION}。",
            manifest.version
        ));
    }
    if manifest.accounts.is_empty() {
        return Err("导入包中没有账号。".to_string());
    }
    if manifest.accounts.len() > MAX_ACCOUNTS {
        return Err(format!("导入包账号数量过多，最多支持 {MAX_ACCOUNTS} 个。"));
    }
    Ok(())
}

fn validate_archive_entry_names<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
) -> AppResult<()> {
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|err| err.to_string())?;
        validate_bundle_entry_name(file.name())?;
    }
    Ok(())
}

fn validate_bundle_entry_name(name: &str) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err("ZIP 内文件名不能为空。".to_string());
    }
    if name.ends_with('/') || name.ends_with('\\') {
        return Err(format!("ZIP 内不应包含目录项：{name}"));
    }
    if name.contains('\\') || name.contains('\0') || name.contains(':') {
        return Err(format!("ZIP 内路径不安全：{name}"));
    }
    for component in Path::new(name).components() {
        match component {
            Component::Normal(_) => {}
            _ => return Err(format!("ZIP 内路径不安全：{name}")),
        }
    }
    Ok(())
}

fn read_zip_file<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    max_bytes: u64,
) -> AppResult<Vec<u8>> {
    validate_bundle_entry_name(path)?;
    let mut file = archive
        .by_name(path)
        .map_err(|_| format!("导入包缺少文件：{path}"))?;
    if file.size() > max_bytes {
        return Err(format!("导入包内文件过大：{path}"));
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut bytes).map_err(stringify_io)?;
    Ok(bytes)
}

fn write_profile_files<W: Write + std::io::Seek>(
    writer: &mut ZipWriter<W>,
    profile_dir: &Path,
    zip_profile_path: &str,
    options: SimpleFileOptions,
) -> AppResult<()> {
    for entry in WalkDir::new(profile_dir)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
    {
        let entry = entry.map_err(|err| err.to_string())?;
        if entry.depth() == 0 || !entry.file_type().is_file() || entry.path_is_symlink() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(profile_dir)
            .map_err(|err| err.to_string())?;
        let relative_zip = path_to_zip_name(relative)?;
        let zip_path = format!("{zip_profile_path}/{relative_zip}");
        validate_bundle_entry_name(&zip_path)?;
        let metadata = entry.metadata().map_err(|err| err.to_string())?;
        if metadata.len() > MAX_PROFILE_FILE_BYTES {
            return Err(format!(
                "Profile 文件过大，已停止导出：{}",
                entry.path().display()
            ));
        }
        writer
            .start_file(&zip_path, options)
            .map_err(|err| err.to_string())?;
        let mut file = fs::File::open(entry.path()).map_err(stringify_io)?;
        std::io::copy(&mut file, writer).map_err(stringify_io)?;
    }
    Ok(())
}

fn replace_profile_from_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    entry: &AccountBundleManifestAccount,
    target_profile_dir: &Path,
) -> AppResult<()> {
    let profile_path = entry
        .profile_path
        .as_deref()
        .ok_or_else(|| "manifest 未声明 profile 路径。".to_string())?;
    let prefix = format!("{profile_path}/");
    let parent = target_profile_dir
        .parent()
        .ok_or_else(|| "Profile 目标路径无效。".to_string())?;
    fs::create_dir_all(parent).map_err(stringify_io)?;
    if target_profile_dir.exists() {
        fs::remove_dir_all(target_profile_dir).map_err(stringify_io)?;
    }
    fs::create_dir_all(target_profile_dir).map_err(stringify_io)?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|err| err.to_string())?;
        let name = file.name().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }
        validate_bundle_entry_name(&name)?;
        if file.size() > MAX_PROFILE_FILE_BYTES {
            return Err(format!("Profile 文件过大：{name}"));
        }
        let relative = name.trim_start_matches(&prefix);
        validate_bundle_entry_name(relative)?;
        let target = target_profile_dir.join(Path::new(relative));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(stringify_io)?;
        }
        let mut output = fs::File::create(&target).map_err(stringify_io)?;
        std::io::copy(&mut file, &mut output).map_err(stringify_io)?;
    }

    Ok(())
}

fn path_to_zip_name(path: &Path) -> AppResult<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_string_lossy();
                if value.contains('/') || value.contains('\\') || value.contains(':') {
                    return Err(format!("路径不能写入导出包：{}", path.display()));
                }
                parts.push(value.to_string());
            }
            _ => return Err(format!("路径不能写入导出包：{}", path.display())),
        }
    }
    if parts.is_empty() {
        return Err("路径不能写入导出包。".to_string());
    }
    Ok(parts.join("/"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AccountSummary, StoredAccount};
    use serde_json::json;
    use std::fs;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;
    use zip::{ZipArchive, ZipWriter};

    fn test_account(profile_dir: Option<String>) -> StoredAccount {
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "access_token": "access",
                "id_token": "id",
                "refresh_token": "refresh",
                "account_id": "acct-1"
            },
            "last_refresh": "2026-05-22T00:00:00Z"
        });
        StoredAccount {
            summary: AccountSummary {
                id: "acct-1".to_string(),
                display_name: "user@example.com".to_string(),
                email: Some("user@example.com".to_string()),
                account_id: Some("acct-1".to_string()),
                plan: Some("plus".to_string()),
                expired: None,
                disabled: false,
                imported_at: "2026-05-22T00:00:00Z".to_string(),
                has_config: false,
                browser_profile_dir: profile_dir,
                oauth_metadata: None,
                quota_state: None,
                usage_state: None,
            },
            auth_json: auth_json.clone(),
            original_json: auth_json,
        }
    }

    fn make_bundle(entries: Vec<(&str, &[u8])>) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn validate_bundle_entry_name_rejects_escape_paths() {
        assert!(validate_bundle_entry_name("manifest.json").is_ok());
        assert!(validate_bundle_entry_name("accounts/acct-1/auth.json").is_ok());

        for invalid in [
            "",
            "/manifest.json",
            "accounts/../auth.json",
            "accounts\\acct-1\\auth.json",
            "C:/accounts/acct-1/auth.json",
            "accounts/acct-1/",
        ] {
            assert!(
                validate_bundle_entry_name(invalid).is_err(),
                "{invalid} should reject"
            );
        }
    }

    #[test]
    fn import_rejects_auth_hash_mismatch() {
        let auth = br#"{"auth_mode":"chatgpt","tokens":{"access_token":"a","id_token":"i","refresh_token":"r","account_id":"acct-1"}}"#;
        let manifest = br#"{
            "format": "codex-switch.account-bundle",
            "version": 1,
            "exported_at": "2026-05-22T00:00:00Z",
            "accounts": [{
                "id": "acct-1",
                "display_name": "acct-1",
                "email": null,
                "account_id": "acct-1",
                "auth_path": "accounts/acct-1/auth.json",
                "auth_sha256": "bad",
                "profile_path": null
            }]
        }"#;
        let zip = make_bundle(vec![
            ("manifest.json", manifest),
            ("accounts/acct-1/auth.json", auth),
        ]);
        let temp = tempfile::tempdir().unwrap();
        let bundle_path = temp.path().join("bundle.zip");
        fs::write(&bundle_path, zip).unwrap();

        let result = import_account_bundle_from_path(
            &bundle_path,
            &temp.path().join("accounts"),
            &temp.path().join("profiles"),
        )
        .expect("bundle should parse with per-account failure");

        assert_eq!(result.imported.len(), 0);
        assert_eq!(result.failed.len(), 1);
        assert!(result.failed[0].message.contains("SHA-256"));
    }

    #[test]
    fn export_writes_auth_and_profile_files() {
        let temp = tempfile::tempdir().unwrap();
        let profile = temp.path().join("profile");
        fs::create_dir_all(profile.join("Default")).unwrap();
        fs::write(profile.join("Default").join("Cookies"), b"cookie-db").unwrap();
        fs::write(profile.join("Local State"), b"state").unwrap();
        let account = test_account(Some(profile.to_string_lossy().to_string()));
        let bundle_path = temp.path().join("bundle.zip");

        let result = export_account_bundle_to_path(&[account], &bundle_path)
            .expect("bundle export should succeed");

        assert_eq!(result.exported_count, 1);
        assert_eq!(result.included_profile_count, 1);
        let mut archive = ZipArchive::new(fs::File::open(bundle_path).unwrap()).unwrap();
        assert!(archive.by_name("manifest.json").is_ok());
        assert!(archive.by_name("accounts/acct-1/auth.json").is_ok());
        assert!(archive.by_name("profiles/acct-1/Default/Cookies").is_ok());
        assert!(archive.by_name("profiles/acct-1/Local State").is_ok());
    }

    #[test]
    fn import_restores_profile_and_account_metadata() {
        let temp = tempfile::tempdir().unwrap();
        let profile = temp.path().join("source-profile");
        fs::create_dir_all(profile.join("Default")).unwrap();
        fs::write(profile.join("Default").join("Cookies"), b"cookie-db").unwrap();
        let account = test_account(Some(profile.to_string_lossy().to_string()));
        let bundle_path = temp.path().join("bundle.zip");
        export_account_bundle_to_path(&[account], &bundle_path).unwrap();

        let result = import_account_bundle_from_path(
            &bundle_path,
            &temp.path().join("accounts"),
            &temp.path().join("profiles"),
        )
        .expect("bundle import should succeed");

        assert_eq!(result.imported.len(), 1);
        assert_eq!(result.failed.len(), 0);
        let imported = &result.imported[0];
        assert_eq!(imported.id, "acct-1");
        assert_eq!(imported.has_config, false);
        let profile_dir = imported
            .browser_profile_dir
            .as_ref()
            .expect("profile path should be recorded");
        assert!(profile_dir.ends_with("acct-1"));
        assert_eq!(
            fs::read(
                temp.path()
                    .join("profiles")
                    .join("acct-1")
                    .join("Default")
                    .join("Cookies")
            )
            .unwrap(),
            b"cookie-db"
        );
        assert!(temp
            .path()
            .join("accounts")
            .join("acct-1")
            .join("metadata.json")
            .exists());
    }
}
