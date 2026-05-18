use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::accounts::current_identity_from_auth;
use crate::error::AppResult;
use crate::models::{AccountSummary, CodexState};
use crate::settings::{load_settings, Settings};

#[tauri::command]
pub fn read_current_codex_state() -> AppResult<CodexState> {
    let settings = load_settings()?;
    let codex_home = PathBuf::from(&settings.codex_home);
    let auth_path = codex_home.join("auth.json");
    let config_path = codex_home.join("config.toml");
    let auth = fs::read_to_string(&auth_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let current_identity = auth.as_ref().map(current_identity_from_auth);

    Ok(CodexState {
        codex_home: settings.codex_home,
        auth_exists: auth_path.exists(),
        config_exists: config_path.exists(),
        current_account_id: current_identity
            .as_ref()
            .and_then(|identity| identity.account_id.clone()),
        current_email: current_identity
            .as_ref()
            .and_then(|identity| identity.email.clone()),
        current_auth_mode: auth
            .as_ref()
            .and_then(|value| value.get("auth_mode"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

pub fn close_codex_processes(settings: &Settings, warnings: &mut Vec<String>) {
    for name in &settings.process_names {
        let gentle = Command::new("taskkill").args(["/IM", name, "/T"]).output();
        if let Err(err) = gentle {
            warnings.push(format!("无法请求关闭 {name}：{err}"));
            continue;
        }
    }

    thread::sleep(Duration::from_millis(settings.close_timeout_ms));

    for name in &settings.process_names {
        let forced = Command::new("taskkill")
            .args(["/IM", name, "/T", "/F"])
            .output();
        if let Ok(output) = forced {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.contains("not found") && !stderr.contains("没有找到") {
                    let message = stderr.trim();
                    if !message.is_empty() {
                        warnings.push(format!("{name} 强制关闭返回：{message}"));
                    }
                }
            }
        }
    }
}

pub fn account_warnings(summary: &AccountSummary) -> Vec<String> {
    let mut warnings = Vec::new();
    if summary.disabled {
        warnings.push("目标账号标记为 disabled。".to_string());
    }
    if let Some(expired) = &summary.expired {
        if let Ok(date) = DateTime::parse_from_rfc3339(expired) {
            if date.with_timezone(&Utc) < Utc::now() {
                warnings.push(format!("目标账号 token 已过期：{expired}。"));
            }
        }
    }
    warnings
}
