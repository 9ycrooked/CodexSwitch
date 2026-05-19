use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{stringify_io, AppResult};
use crate::models::AppPaths;
use crate::paths::app_store_dir;
use crate::process::hidden_command;
use crate::settings::load_settings;

fn open_dir(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("不是目录：{}", path.display()));
    }

    hidden_command("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(stringify_io)?;
    Ok(())
}

fn ensure_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(stringify_io)
}

fn reject_path_escape(value: &str) -> AppResult<()> {
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        return Err("备份 ID 不能是绝对路径。".into());
    }
    if candidate.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("备份 ID 不能包含路径逃逸片段。".into());
    }
    Ok(())
}

fn ensure_backup_target_inside_root(root: &Path, target: &Path) -> AppResult<()> {
    if !target.starts_with(root) {
        return Err("备份目录不在备份根目录内。".into());
    }
    Ok(())
}

fn validate_backup_target(root: &Path, target: &Path) -> AppResult<PathBuf> {
    if !target.exists() {
        return Err(format!("目录不存在：{}", target.display()));
    }
    if !target.is_dir() {
        return Err(format!("不是目录：{}", target.display()));
    }

    let canonical_root = fs::canonicalize(root).map_err(stringify_io)?;
    let canonical_target = fs::canonicalize(target).map_err(stringify_io)?;
    ensure_backup_target_inside_root(&canonical_root, &canonical_target)?;
    Ok(canonical_target)
}

fn backups_dir() -> AppResult<PathBuf> {
    Ok(app_store_dir()?.join("backups"))
}

#[tauri::command]
pub fn read_app_paths() -> AppResult<AppPaths> {
    let settings = load_settings()?;
    let app_store_dir = app_store_dir()?;
    let backups_dir = app_store_dir.join("backups");
    Ok(AppPaths {
        codex_home: settings.codex_home,
        app_store_dir: app_store_dir.to_string_lossy().to_string(),
        backups_dir: backups_dir.to_string_lossy().to_string(),
        browser_profile_dir: settings.browser_profile_dir,
    })
}

#[tauri::command]
pub fn open_codex_home_dir() -> AppResult<()> {
    let settings = load_settings()?;
    open_dir(&PathBuf::from(settings.codex_home))
}

#[tauri::command]
pub fn open_app_store_dir() -> AppResult<()> {
    let dir = app_store_dir()?;
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_browser_profile_dir() -> AppResult<()> {
    let settings = load_settings()?;
    let dir = PathBuf::from(settings.browser_profile_dir);
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_backups_dir() -> AppResult<()> {
    let dir = backups_dir()?;
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_backup_dir(backup_id: String) -> AppResult<()> {
    reject_path_escape(&backup_id)?;
    let root = backups_dir()?;
    let dir = root.join(backup_id);
    let dir = validate_backup_target(&root, &dir)?;
    open_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_backup_id_escape() {
        assert!(reject_path_escape("..\\secret").is_err());
        assert!(reject_path_escape("../secret").is_err());
        assert!(reject_path_escape("C:\\secret").is_err());
    }

    #[test]
    fn accepts_normal_backup_id() {
        assert!(reject_path_escape("20260519-120000-abcdef").is_ok());
    }

    #[test]
    fn accepts_canonical_backup_target_inside_root() {
        assert!(ensure_backup_target_inside_root(
            Path::new(r"C:\data\backups"),
            Path::new(r"C:\data\backups\20260519-120000-abcdef")
        )
        .is_ok());
    }

    #[test]
    fn rejects_canonical_backup_target_outside_root() {
        assert!(ensure_backup_target_inside_root(
            Path::new(r"C:\data\backups"),
            Path::new(r"C:\data\outside")
        )
        .is_err());
    }
}
