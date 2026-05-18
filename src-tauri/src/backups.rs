use chrono::Utc;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::accounts::now_string;
use crate::error::{stringify_io, AppResult};
use crate::io::{atomic_write_json, atomic_write_text, read_json};
use crate::models::{BackupMeta, BackupSummary};
use crate::paths::app_store_dir;
use crate::settings::{load_settings, Settings};

#[tauri::command]
pub fn list_backups() -> AppResult<Vec<BackupSummary>> {
    let backups_dir = app_store_dir()?.join("backups");
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in fs::read_dir(backups_dir).map_err(stringify_io)? {
        let entry = entry.map_err(stringify_io)?;
        if !entry.file_type().map_err(stringify_io)?.is_dir() {
            continue;
        }
        let meta_path = entry.path().join("metadata.json");
        if meta_path.exists() {
            let meta: BackupMeta = read_json(&meta_path)?;
            backups.push(BackupSummary {
                id: meta.id,
                created_at: meta.created_at,
                auth_path: meta.auth_path,
                config_path: meta.config_path,
            });
        }
    }
    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

#[tauri::command]
pub fn backup_current_state() -> AppResult<BackupSummary> {
    let settings = load_settings()?;
    create_backup(&settings)
}

#[tauri::command]
pub fn restore_backup(backup_id: String) -> AppResult<()> {
    let settings = load_settings()?;
    let codex_home = PathBuf::from(settings.codex_home);
    fs::create_dir_all(&codex_home).map_err(stringify_io)?;

    let backup_dir = app_store_dir()?.join("backups").join(&backup_id);
    if !backup_dir.exists() {
        return Err(format!("备份不存在：{backup_id}"));
    }

    let auth_backup = backup_dir.join("auth.json");
    let config_backup = backup_dir.join("config.toml");
    if auth_backup.exists() {
        atomic_write_text(
            &codex_home.join("auth.json"),
            &fs::read_to_string(auth_backup).map_err(stringify_io)?,
        )?;
    }
    if config_backup.exists() {
        atomic_write_text(
            &codex_home.join("config.toml"),
            &fs::read_to_string(config_backup).map_err(stringify_io)?,
        )?;
    }
    Ok(())
}

pub fn create_backup(settings: &Settings) -> AppResult<BackupSummary> {
    let codex_home = PathBuf::from(&settings.codex_home);
    let backups_dir = app_store_dir()?.join("backups");
    fs::create_dir_all(&backups_dir).map_err(stringify_io)?;

    let id = format!(
        "{}-{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        Uuid::new_v4().simple()
    );
    let backup_dir = backups_dir.join(&id);
    fs::create_dir_all(&backup_dir).map_err(stringify_io)?;

    let auth_src = codex_home.join("auth.json");
    let config_src = codex_home.join("config.toml");
    let auth_path = if auth_src.exists() {
        fs::copy(&auth_src, backup_dir.join("auth.json")).map_err(stringify_io)?;
        Some(backup_dir.join("auth.json").to_string_lossy().to_string())
    } else {
        None
    };
    let config_path = if config_src.exists() {
        fs::copy(&config_src, backup_dir.join("config.toml")).map_err(stringify_io)?;
        Some(backup_dir.join("config.toml").to_string_lossy().to_string())
    } else {
        None
    };

    let backup = BackupSummary {
        id,
        created_at: now_string(),
        auth_path,
        config_path,
    };
    let meta = BackupMeta {
        id: backup.id.clone(),
        created_at: backup.created_at.clone(),
        auth_path: backup.auth_path.clone(),
        config_path: backup.config_path.clone(),
    };
    atomic_write_json(&backup_dir.join("metadata.json"), &meta)?;
    Ok(backup)
}
