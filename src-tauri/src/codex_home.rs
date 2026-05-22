use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

use crate::accounts::current_identity_from_auth;
use crate::error::AppResult;
use crate::models::{AccountSummary, CodexState};
use crate::settings::{load_settings, Settings};

const FAST_CLOSE_WAIT_MS: u64 = 300;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CodexProcessSnapshot {
    #[serde(alias = "Name", alias = "ProcessName")]
    pub name: String,
    #[serde(default, alias = "Path", alias = "ExecutablePath")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct StartAppEntry {
    #[serde(alias = "Name")]
    name: String,
    #[serde(alias = "AppID")]
    app_id: String,
}

#[derive(Debug, Clone)]
struct CodexReopenTarget {
    program: String,
    argument: String,
}

fn is_process_running(name: &str) -> bool {
    let output = crate::process::hidden_command("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {name}"), "/FO", "CSV", "/NH"])
        .output();

    let Ok(output) = output else {
        return false;
    };

    let text = String::from_utf8_lossy(&output.stdout);
    text.to_ascii_lowercase()
        .contains(&name.to_ascii_lowercase())
}

fn wait_until_processes_exit(process_names: &[String], timeout_ms: u64) {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        if process_names.iter().all(|name| !is_process_running(name)) {
            return;
        }
        thread::sleep(Duration::from_millis(150));
    }
}

fn process_name_matches(snapshot_name: &str, configured_name: &str) -> bool {
    let snapshot = snapshot_name
        .trim()
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    let configured = configured_name
        .trim()
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    !configured.is_empty() && snapshot == configured
}

fn parse_process_snapshots(text: &str) -> Vec<CodexProcessSnapshot> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(items) = serde_json::from_str::<Vec<CodexProcessSnapshot>>(trimmed) {
        return items;
    }
    serde_json::from_str::<CodexProcessSnapshot>(trimmed)
        .map(|item| vec![item])
        .unwrap_or_default()
}

fn parse_start_apps(text: &str) -> Vec<StartAppEntry> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if let Ok(items) = serde_json::from_str::<Vec<StartAppEntry>>(trimmed) {
        return items;
    }
    serde_json::from_str::<StartAppEntry>(trimmed)
        .map(|item| vec![item])
        .unwrap_or_default()
}

fn query_codex_start_app_id() -> Option<String> {
    let output = crate::process::hidden_command("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Get-StartApps | Where-Object { $_.Name -eq 'Codex' } | Select-Object -First 1 Name,AppID | ConvertTo-Json -Compress",
        ])
        .stderr(Stdio::null())
        .output();

    let output = output.ok()?;
    if !output.status.success() {
        return None;
    }
    parse_start_apps(&String::from_utf8_lossy(&output.stdout))
        .into_iter()
        .find(|entry| entry.name == "Codex")
        .map(|entry| entry.app_id)
        .filter(|app_id| !app_id.trim().is_empty())
}

fn capture_process_snapshots(process_names: &[String]) -> Vec<CodexProcessSnapshot> {
    let output = crate::process::hidden_command("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            "Get-Process | Select-Object ProcessName,Path | ConvertTo-Json -Compress",
        ])
        .stderr(Stdio::null())
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    parse_process_snapshots(&String::from_utf8_lossy(&output.stdout))
        .into_iter()
        .filter(|snapshot| {
            process_names
                .iter()
                .any(|configured| process_name_matches(&snapshot.name, configured))
        })
        .collect()
}

pub(crate) fn select_codex_desktop_launch_path(
    snapshots: &[CodexProcessSnapshot],
) -> Option<String> {
    snapshots.iter().find_map(|snapshot| {
        if snapshot.name != "Codex" && snapshot.name != "Codex.exe" {
            return None;
        }
        let path = snapshot.path.as_deref()?.trim();
        if path.is_empty() {
            None
        } else {
            Some(path.to_string())
        }
    })
}

pub fn capture_codex_desktop_launch_path(settings: &Settings) -> Option<String> {
    let snapshots = capture_process_snapshots(&settings.process_names);
    select_codex_desktop_launch_path(&snapshots)
}

fn app_activation_target(app_id: &str) -> Option<CodexReopenTarget> {
    let app_id = app_id.trim();
    if app_id.is_empty() {
        return None;
    }
    Some(CodexReopenTarget {
        program: "explorer.exe".to_string(),
        argument: format!(r"shell:AppsFolder\{app_id}"),
    })
}

fn codex_app_id_from_windowsapps_path(path: &str) -> Option<String> {
    let marker = "OpenAI.Codex_";
    let component = path
        .split(['\\', '/'])
        .find(|part| part.starts_with(marker) && part.contains("__"))?;
    let publisher_id = component.split("__").nth(1)?.trim();
    if publisher_id.is_empty() {
        return None;
    }
    Some(format!("OpenAI.Codex_{publisher_id}!App"))
}

fn codex_reopen_target_from_path(path: &str) -> Option<CodexReopenTarget> {
    query_codex_start_app_id()
        .and_then(|app_id| app_activation_target(&app_id))
        .or_else(|| {
            codex_app_id_from_windowsapps_path(path)
                .and_then(|app_id| app_activation_target(&app_id))
        })
}

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

pub fn close_codex_processes_fast(settings: &Settings, warnings: &mut Vec<String>) {
    for name in &settings.process_names {
        let gentle = crate::process::hidden_command("taskkill")
            .args(["/IM", name, "/T"])
            .output();
        if let Err(err) = gentle {
            warnings.push(format!("无法请求关闭 {name}：{err}"));
            continue;
        }
    }

    wait_until_processes_exit(&settings.process_names, FAST_CLOSE_WAIT_MS);

    for name in &settings.process_names {
        let forced = crate::process::hidden_command("taskkill")
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

pub fn reopen_codex_if_needed(launch_path: Option<&str>, warnings: &mut Vec<String>) -> bool {
    let Some(launch_path) = launch_path.map(str::trim).filter(|path| !path.is_empty()) else {
        return false;
    };

    let target = codex_reopen_target_from_path(launch_path).unwrap_or_else(|| CodexReopenTarget {
        program: launch_path.to_string(),
        argument: String::new(),
    });
    let mut command = crate::process::hidden_command(&target.program);
    if !target.argument.is_empty() {
        command.arg(&target.argument);
    }

    match command.spawn() {
        Ok(_) => true,
        Err(err) => {
            warnings.push(format!("账号已切换，但重新打开 Codex 失败：{err}"));
            false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_desktop_codex_launch_path_ignores_lowercase_helpers() {
        let snapshots = vec![
            CodexProcessSnapshot {
                name: "codex".to_string(),
                path: Some(r"C:\Users\Y\AppData\Local\OpenAI\Codex\bin\codex.exe".to_string()),
            },
            CodexProcessSnapshot {
                name: "Codex".to_string(),
                path: Some(r"C:\Program Files\WindowsApps\OpenAI.Codex\app\Codex.exe".to_string()),
            },
        ];

        assert_eq!(
            select_codex_desktop_launch_path(&snapshots).as_deref(),
            Some(r"C:\Program Files\WindowsApps\OpenAI.Codex\app\Codex.exe")
        );
    }

    #[test]
    fn select_desktop_codex_launch_path_returns_none_without_desktop_process() {
        let snapshots = vec![CodexProcessSnapshot {
            name: "codex".to_string(),
            path: Some(r"C:\Users\Y\AppData\Local\OpenAI\Codex\bin\codex.exe".to_string()),
        }];

        assert!(select_codex_desktop_launch_path(&snapshots).is_none());
    }

    #[test]
    fn derives_windows_app_activation_target_from_codex_windowsapps_path() {
        let app_id = codex_app_id_from_windowsapps_path(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.519.3891.0_x64__2p2nqsd0c76g0\app\Codex.exe",
        )
        .expect("WindowsApps Codex path should produce an AppUserModelID");
        let target = app_activation_target(&app_id).expect("AppUserModelID should produce target");

        assert_eq!(target.program, "explorer.exe");
        assert_eq!(
            target.argument,
            r"shell:AppsFolder\OpenAI.Codex_2p2nqsd0c76g0!App"
        );
    }
}
