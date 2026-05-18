use std::path::PathBuf;

use crate::error::AppResult;

pub fn app_store_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir()
        .or_else(dirs::config_dir)
        .ok_or_else(|| "无法定位应用数据目录。".to_string())?;
    Ok(base.join("codex-account-switcher"))
}

pub fn settings_path() -> AppResult<PathBuf> {
    Ok(app_store_dir()?.join("settings.json"))
}

pub fn account_dir(id: &str) -> AppResult<PathBuf> {
    Ok(app_store_dir()?
        .join("accounts")
        .join(crate::accounts::sanitize_id(id)))
}
