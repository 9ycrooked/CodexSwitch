use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{stringify_io, AppResult};
use crate::io::{atomic_write_json, read_json};
use crate::paths::{app_store_dir, settings_path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_codex_home")]
    pub codex_home: String,
    #[serde(default = "default_process_names")]
    pub process_names: Vec<String>,
    #[serde(default = "default_close_timeout_ms")]
    pub close_timeout_ms: u64,
    #[serde(default = "default_browser_profile_dir")]
    pub browser_profile_dir: String,
    #[serde(default = "default_oauth_callback_port")]
    pub oauth_callback_port: u16,
    #[serde(default = "default_keep_login_profiles")]
    pub keep_login_profiles: bool,
    #[serde(default = "default_oauth_login_mode")]
    pub oauth_login_mode: String,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
    #[serde(default)]
    pub force_update_on_startup: bool,
    #[serde(default = "default_true")]
    pub check_oauth_network_on_login: bool,
    #[serde(default)]
    pub check_egress_region: bool,
    #[serde(default)]
    pub autoflow_oauth_server_enabled: bool,
    #[serde(default = "default_autoflow_oauth_server_port")]
    pub autoflow_oauth_server_port: u16,
    #[serde(default)]
    pub autoflow_oauth_admin_key: String,
}

pub fn load_settings() -> AppResult<Settings> {
    let path = settings_path()?;
    if path.exists() {
        read_json(&path)
    } else {
        let settings = default_settings();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(stringify_io)?;
        }
        atomic_write_json(&path, &settings)?;
        Ok(settings)
    }
}

pub fn default_settings() -> Settings {
    Settings {
        codex_home: default_codex_home(),
        process_names: default_process_names(),
        close_timeout_ms: default_close_timeout_ms(),
        browser_profile_dir: default_browser_profile_dir(),
        oauth_callback_port: default_oauth_callback_port(),
        keep_login_profiles: default_keep_login_profiles(),
        oauth_login_mode: default_oauth_login_mode(),
        check_updates_on_startup: default_true(),
        force_update_on_startup: false,
        check_oauth_network_on_login: true,
        check_egress_region: false,
        autoflow_oauth_server_enabled: false,
        autoflow_oauth_server_port: default_autoflow_oauth_server_port(),
        autoflow_oauth_admin_key: String::new(),
    }
}

#[tauri::command]
pub fn read_settings() -> AppResult<Settings> {
    load_settings()
}

#[tauri::command]
pub fn update_settings(settings: Settings) -> AppResult<Settings> {
    let sanitized = sanitize_settings(settings)?;
    save_settings(&sanitized)?;
    Ok(sanitized)
}

pub(crate) fn sanitize_settings(settings: Settings) -> AppResult<Settings> {
    if settings.codex_home.trim().is_empty() {
        return Err("Codex home 不能为空。".into());
    }
    if settings.close_timeout_ms < 500 {
        return Err("关闭超时不能小于 500ms。".into());
    }
    if settings.oauth_callback_port < 1024 {
        return Err("OAuth 回调端口不能小于 1024。".into());
    }
    if settings.autoflow_oauth_server_port < 1024 {
        return Err("AutoFlow 接入服务端口不能小于 1024。".into());
    }

    let process_names: Vec<String> = settings
        .process_names
        .into_iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect();
    if process_names.is_empty() {
        return Err("至少需要一个 Codex 进程名。".into());
    }

    Ok(Settings {
        codex_home: settings.codex_home.trim().to_string(),
        process_names,
        close_timeout_ms: settings.close_timeout_ms,
        browser_profile_dir: settings.browser_profile_dir.trim().to_string(),
        oauth_callback_port: settings.oauth_callback_port,
        keep_login_profiles: settings.keep_login_profiles,
        oauth_login_mode: sanitize_oauth_login_mode(&settings.oauth_login_mode),
        check_updates_on_startup: settings.check_updates_on_startup,
        force_update_on_startup: settings.force_update_on_startup,
        check_oauth_network_on_login: settings.check_oauth_network_on_login,
        check_egress_region: settings.check_egress_region,
        autoflow_oauth_server_enabled: settings.autoflow_oauth_server_enabled,
        autoflow_oauth_server_port: settings.autoflow_oauth_server_port,
        autoflow_oauth_admin_key: settings.autoflow_oauth_admin_key.trim().to_string(),
    })
}

pub(crate) fn save_settings(settings: &Settings) -> AppResult<()> {
    atomic_write_json(&settings_path()?, settings)
}

pub fn sanitize_oauth_login_mode(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("embedded") {
        "embedded".to_string()
    } else {
        "external".to_string()
    }
}

pub fn default_true() -> bool {
    true
}

fn default_codex_home() -> String {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Y"))
        .join(".codex")
        .to_string_lossy()
        .to_string()
}

fn default_process_names() -> Vec<String> {
    vec!["Codex.exe".to_string(), "codex.exe".to_string()]
}

fn default_close_timeout_ms() -> u64 {
    6000
}

fn default_browser_profile_dir() -> String {
    app_store_dir()
        .unwrap_or_else(|_| PathBuf::from(r"C:\codex-account-switcher"))
        .join("browser-profiles")
        .to_string_lossy()
        .to_string()
}

fn default_oauth_callback_port() -> u16 {
    1455
}

fn default_autoflow_oauth_server_port() -> u16 {
    8080
}

fn default_keep_login_profiles() -> bool {
    true
}

fn default_oauth_login_mode() -> String {
    "external".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_update_preferences_when_missing() {
        let raw = r#"{
            "codex_home": "C:\\Users\\Y\\.codex",
            "process_names": ["Codex.exe"],
            "close_timeout_ms": 3000,
            "browser_profile_dir": "profiles",
            "oauth_callback_port": 1455,
            "keep_login_profiles": true,
            "oauth_login_mode": "external"
        }"#;

        let settings: Settings = serde_json::from_str(raw).expect("settings should deserialize");

        assert!(settings.check_updates_on_startup);
        assert!(!settings.force_update_on_startup);
        assert!(settings.check_oauth_network_on_login);
        assert!(!settings.check_egress_region);
    }

    #[test]
    fn settings_defaults_autoflow_fields_when_missing() {
        let raw = r#"{
            "codex_home": "C:\\Users\\Y\\.codex",
            "process_names": ["Codex.exe"],
            "close_timeout_ms": 3000,
            "browser_profile_dir": "profiles",
            "oauth_callback_port": 1455,
            "keep_login_profiles": true,
            "oauth_login_mode": "external"
        }"#;

        let settings: Settings = serde_json::from_str(raw).expect("settings should deserialize");

        assert!(!settings.autoflow_oauth_server_enabled);
        assert_eq!(settings.autoflow_oauth_server_port, 8080);
        assert_eq!(settings.autoflow_oauth_admin_key, "");
    }

    #[test]
    fn update_settings_rejects_invalid_autoflow_port() {
        let mut settings = default_settings();
        settings.autoflow_oauth_server_port = 80;

        let err = sanitize_settings(settings).expect_err("invalid port should be rejected");

        assert_eq!(err, "AutoFlow 接入服务端口不能小于 1024。");
    }
}
