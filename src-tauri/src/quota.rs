use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;

use crate::accounts::{list_accounts, load_account, now_string, save_account_record};
use crate::error::{run_blocking, AppResult};
use crate::http_client::backend_client;
use crate::models::{CodexQuotaWindow, QuotaState, StoredAccount, UsageState};

const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const QUOTA_WINDOW_FIVE_HOURS: i64 = 18_000;
const QUOTA_WINDOW_WEEK: i64 = 604_800;
#[tauri::command]
pub async fn check_account_quota(
    account_id: String,
    model: Option<String>,
) -> AppResult<QuotaState> {
    run_blocking(move || check_account_quota_blocking(account_id, model)).await
}

fn check_account_quota_blocking(
    account_id: String,
    model: Option<String>,
) -> AppResult<QuotaState> {
    let mut account = load_account(&account_id)?;
    let token = account
        .auth_json
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "账号缺少 access_token。".to_string())?;
    let account_id_header = account
        .auth_json
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let model = model.unwrap_or_else(|| "gpt-5.5".to_string());
    let state = probe_quota(token, account_id_header, &model);
    account.summary.quota_state = Some(state.clone());
    save_account_record(&account.summary, &account.auth_json, &account.original_json)?;
    Ok(state)
}

#[tauri::command]
pub fn list_quota_states() -> AppResult<HashMap<String, QuotaState>> {
    let mut states = HashMap::new();
    for account in list_accounts()? {
        if let Some(state) = account.quota_state {
            states.insert(account.id, state);
        }
    }
    Ok(states)
}

#[tauri::command]
pub async fn fetch_codex_usage(account_id: String) -> AppResult<UsageState> {
    run_blocking(move || fetch_codex_usage_blocking(account_id)).await
}

fn fetch_codex_usage_blocking(account_id: String) -> AppResult<UsageState> {
    let mut account = load_account(&account_id)?;
    let state = fetch_codex_usage_for_account(&account)?;
    account.summary.usage_state = Some(state.clone());
    account.summary.quota_state = Some(quota_state_from_usage_state(&state));
    save_account_record(&account.summary, &account.auth_json, &account.original_json)?;
    Ok(state)
}

#[tauri::command]
pub fn list_usage_states() -> AppResult<HashMap<String, UsageState>> {
    let mut states = HashMap::new();
    for account in list_accounts()? {
        if let Some(state) = account.usage_state {
            states.insert(account.id, state);
        }
    }
    Ok(states)
}

#[tauri::command]
pub async fn clear_usage_state(account_id: String) -> AppResult<()> {
    run_blocking(move || clear_usage_state_blocking(account_id)).await
}

fn clear_usage_state_blocking(account_id: String) -> AppResult<()> {
    let mut account = load_account(&account_id)?;
    account.summary.usage_state = None;
    save_account_record(&account.summary, &account.auth_json, &account.original_json)
}

fn probe_quota(token: &str, account_id: &str, model: &str) -> QuotaState {
    let checked_at = now_string();
    let body = json!({
        "model": model,
        "instructions": "",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "quota ping"}]
        }],
        "stream": false
    });
    let client = match backend_client(Duration::from_secs(60)) {
        Ok(client) => client,
        Err(err) => {
            return quota_failure(&checked_at, model, err);
        }
    };
    let mut request = client
        .post(format!("{CODEX_BASE_URL}/responses"))
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Originator", "codex_cli_rs")
        .header(
            "User-Agent",
            "codex_cli_rs/0.118.0 (Windows; x86_64) CodexAccountSwitcher/0.1.0",
        )
        .json(&body);
    if !account_id.trim().is_empty() {
        request = request.header("Chatgpt-Account-Id", account_id);
    }
    match request.send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let text = response.text().unwrap_or_default();
            if status == 429 {
                parse_quota_error(&text, &checked_at, model)
            } else if (200..300).contains(&status) {
                QuotaState {
                    status: "ok".to_string(),
                    last_checked_at: Some(checked_at),
                    last_error: None,
                    resets_at: None,
                    resets_in_seconds: None,
                    model: Some(model.to_string()),
                }
            } else if status == 401 || status == 403 {
                QuotaState {
                    status: "token_invalid".to_string(),
                    last_checked_at: Some(checked_at),
                    last_error: Some(format!("HTTP {status}: {text}")),
                    resets_at: None,
                    resets_in_seconds: None,
                    model: Some(model.to_string()),
                }
            } else {
                quota_failure(&checked_at, model, format!("HTTP {status}: {text}"))
            }
        }
        Err(err) => quota_failure(&checked_at, model, err.to_string()),
    }
}

fn parse_quota_error(body: &str, checked_at: &str, model: &str) -> QuotaState {
    let value = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let resets_in_seconds = value
        .pointer("/error/resets_in_seconds")
        .and_then(Value::as_i64);
    let resets_at = value
        .pointer("/error/resets_at")
        .and_then(Value::as_i64)
        .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
        .map(|date| date.to_rfc3339())
        .or_else(|| {
            resets_in_seconds
                .and_then(|secs| DateTime::<Utc>::from_timestamp(Utc::now().timestamp() + secs, 0))
                .map(|date| date.to_rfc3339())
        });
    QuotaState {
        status: "cooldown".to_string(),
        last_checked_at: Some(checked_at.to_string()),
        last_error: Some(
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or(body)
                .to_string(),
        ),
        resets_at,
        resets_in_seconds,
        model: Some(model.to_string()),
    }
}

fn quota_failure(checked_at: &str, model: &str, error: String) -> QuotaState {
    QuotaState {
        status: "check_failed".to_string(),
        last_checked_at: Some(checked_at.to_string()),
        last_error: Some(error),
        resets_at: None,
        resets_in_seconds: None,
        model: Some(model.to_string()),
    }
}

fn fetch_codex_usage_for_account(account: &StoredAccount) -> AppResult<UsageState> {
    let checked_at = now_string();
    let token = account
        .auth_json
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "账号缺少 access_token。".to_string())?;
    let account_id_header = account
        .auth_json
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .or(account.summary.account_id.as_deref())
        .unwrap_or_default();
    let client = match backend_client(Duration::from_secs(60)) {
        Ok(client) => client,
        Err(err) => return usage_failure(&checked_at, None, err, None),
    };
    let mut request = client
        .get(CODEX_USAGE_URL)
        .bearer_auth(token)
        .header("Content-Type", "application/json")
        .header(
            "User-Agent",
            "codex_cli_rs/0.76.0 (Windows; x86_64) CodexAccountSwitcher/0.1.0",
        );
    if !account_id_header.trim().is_empty() {
        request = request.header("Chatgpt-Account-Id", account_id_header);
    }

    match request.send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let text = response.text().unwrap_or_default();
            if (200..300).contains(&status) {
                match parse_codex_usage_payload(&text) {
                    Some(payload) => {
                        let windows = build_codex_usage_windows(&payload);
                        if windows.is_empty() {
                            usage_failure(
                                &checked_at,
                                Some(status),
                                "Codex usage 响应中没有可显示的额度窗口。".to_string(),
                                usage_plan_type(&payload),
                            )
                        } else {
                            Ok(UsageState {
                                status: "success".to_string(),
                                last_checked_at: Some(checked_at),
                                last_error: None,
                                http_status: Some(status),
                                resets_at: None,
                                raw_plan_type: usage_plan_type(&payload),
                                windows,
                            })
                        }
                    }
                    None => usage_failure(
                        &checked_at,
                        Some(status),
                        "Codex usage 响应不是有效 JSON。".to_string(),
                        None,
                    ),
                }
            } else if status == 429 {
                Ok(usage_state_from_quota_error(&text, &checked_at, status))
            } else if status == 401 || status == 403 {
                Ok(UsageState {
                    status: "token_invalid".to_string(),
                    last_checked_at: Some(checked_at),
                    last_error: Some(format!("HTTP {status}: {text}")),
                    http_status: Some(status),
                    resets_at: None,
                    raw_plan_type: None,
                    windows: Vec::new(),
                })
            } else {
                usage_failure(
                    &checked_at,
                    Some(status),
                    format!("HTTP {status}: {text}"),
                    None,
                )
            }
        }
        Err(err) => usage_failure(&checked_at, None, err.to_string(), None),
    }
}

fn parse_codex_usage_payload(text: &str) -> Option<Value> {
    let value = serde_json::from_str::<Value>(text.trim()).ok()?;
    if let Some(body) = value.get("body") {
        if body.is_object() {
            return Some(body.clone());
        }
        if let Some(body_text) = body.as_str() {
            return serde_json::from_str::<Value>(body_text.trim()).ok();
        }
    }
    if value.is_object() {
        Some(value)
    } else {
        None
    }
}

fn build_codex_usage_windows(payload: &Value) -> Vec<CodexQuotaWindow> {
    let mut windows = Vec::new();
    if let Some(rate_limit) = field(payload, &["rate_limit", "rateLimit"]) {
        let (five_hour, weekly) = pick_codex_windows(rate_limit, true);
        let limit_reached = bool_field(rate_limit, &["limit_reached", "limitReached"]);
        let allowed = bool_field(rate_limit, &["allowed"]);
        add_codex_window(
            &mut windows,
            "five-hour",
            "5 小时额度",
            five_hour,
            limit_reached,
            allowed,
        );
        add_codex_window(
            &mut windows,
            "weekly",
            "周额度",
            weekly,
            limit_reached,
            allowed,
        );
    }

    if let Some(rate_limit) = field(payload, &["code_review_rate_limit", "codeReviewRateLimit"]) {
        let (five_hour, weekly) = pick_codex_windows(rate_limit, true);
        let limit_reached = bool_field(rate_limit, &["limit_reached", "limitReached"]);
        let allowed = bool_field(rate_limit, &["allowed"]);
        add_codex_window(
            &mut windows,
            "code-review-five-hour",
            "Code Review 5 小时额度",
            five_hour,
            limit_reached,
            allowed,
        );
        add_codex_window(
            &mut windows,
            "code-review-weekly",
            "Code Review 周额度",
            weekly,
            limit_reached,
            allowed,
        );
    }

    if let Some(items) = field(payload, &["additional_rate_limits", "additionalRateLimits"])
        .and_then(Value::as_array)
    {
        for (index, item) in items.iter().enumerate() {
            let Some(rate_info) = field(item, &["rate_limit", "rateLimit"]) else {
                continue;
            };
            let name = string_field(item, &["limit_name", "limitName"])
                .or_else(|| string_field(item, &["metered_feature", "meteredFeature"]))
                .unwrap_or_else(|| format!("additional-{}", index + 1));
            let id_prefix = normalize_window_id(&name, index + 1);
            let limit_reached = bool_field(rate_info, &["limit_reached", "limitReached"]);
            let allowed = bool_field(rate_info, &["allowed"]);
            add_codex_window(
                &mut windows,
                &format!("{id_prefix}-five-hour-{index}"),
                &format!("{name} 5 小时额度"),
                field(rate_info, &["primary_window", "primaryWindow"]),
                limit_reached,
                allowed,
            );
            add_codex_window(
                &mut windows,
                &format!("{id_prefix}-weekly-{index}"),
                &format!("{name} 周额度"),
                field(rate_info, &["secondary_window", "secondaryWindow"]),
                limit_reached,
                allowed,
            );
        }
    }
    windows
}

fn pick_codex_windows(
    rate_info: &Value,
    allow_order_fallback: bool,
) -> (Option<&Value>, Option<&Value>) {
    let primary = field(rate_info, &["primary_window", "primaryWindow"]);
    let secondary = field(rate_info, &["secondary_window", "secondaryWindow"]);
    let mut five_hour = None;
    let mut weekly = None;
    for window in [primary, secondary].into_iter().flatten() {
        match number_field(window, &["limit_window_seconds", "limitWindowSeconds"])
            .map(|value| value as i64)
        {
            Some(QUOTA_WINDOW_FIVE_HOURS) if five_hour.is_none() => five_hour = Some(window),
            Some(QUOTA_WINDOW_WEEK) if weekly.is_none() => weekly = Some(window),
            _ => {}
        }
    }
    if allow_order_fallback {
        if five_hour.is_none() {
            if let Some(primary) = primary {
                if weekly.map_or(true, |weekly| !std::ptr::eq(primary, weekly)) {
                    five_hour = Some(primary);
                }
            }
        }
        if weekly.is_none() {
            if let Some(secondary) = secondary {
                if five_hour.map_or(true, |five_hour| !std::ptr::eq(secondary, five_hour)) {
                    weekly = Some(secondary);
                }
            }
        }
    }
    (five_hour, weekly)
}

fn add_codex_window(
    windows: &mut Vec<CodexQuotaWindow>,
    id: &str,
    label: &str,
    window: Option<&Value>,
    limit_reached: Option<bool>,
    allowed: Option<bool>,
) {
    let Some(window) = window else {
        return;
    };
    let is_limit_reached = limit_reached.unwrap_or(false) || allowed == Some(false);
    let reset_at = codex_window_reset_at(window);
    let used_percent = number_field(window, &["used_percent", "usedPercent"])
        .or_else(|| is_limit_reached.then_some(100.0));
    windows.push(CodexQuotaWindow {
        id: id.to_string(),
        label: label.to_string(),
        used_percent,
        reset_label: reset_at.clone().unwrap_or_else(|| "-".to_string()),
        reset_at,
        limit_reached: is_limit_reached,
    });
}

fn codex_window_reset_at(window: &Value) -> Option<String> {
    number_field(window, &["reset_at", "resetAt"])
        .map(|value| value as i64)
        .filter(|value| *value > 0)
        .or_else(|| {
            number_field(window, &["reset_after_seconds", "resetAfterSeconds"])
                .map(|value| Utc::now().timestamp() + value as i64)
                .filter(|value| *value > 0)
        })
        .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
        .map(|value| value.to_rfc3339())
}

fn usage_state_from_quota_error(body: &str, checked_at: &str, status: u16) -> UsageState {
    let quota = parse_quota_error(body, checked_at, "usage");
    UsageState {
        status: "cooldown".to_string(),
        last_checked_at: Some(checked_at.to_string()),
        last_error: quota.last_error,
        http_status: Some(status),
        resets_at: quota.resets_at,
        raw_plan_type: None,
        windows: Vec::new(),
    }
}

fn usage_failure(
    checked_at: &str,
    http_status: Option<u16>,
    error: String,
    raw_plan_type: Option<String>,
) -> AppResult<UsageState> {
    Ok(UsageState {
        status: "check_failed".to_string(),
        last_checked_at: Some(checked_at.to_string()),
        last_error: Some(error),
        http_status,
        resets_at: None,
        raw_plan_type,
        windows: Vec::new(),
    })
}

fn quota_state_from_usage_state(state: &UsageState) -> QuotaState {
    QuotaState {
        status: match state.status.as_str() {
            "success" => "ok",
            "cooldown" => "cooldown",
            "token_invalid" => "token_invalid",
            _ => "check_failed",
        }
        .to_string(),
        last_checked_at: state.last_checked_at.clone(),
        last_error: state.last_error.clone(),
        resets_at: state.resets_at.clone(),
        resets_in_seconds: None,
        model: Some("usage".to_string()),
    }
}

fn usage_plan_type(payload: &Value) -> Option<String> {
    string_field(payload, &["plan_type", "planType"])
}

fn field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    field(value, keys)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    field(value, keys).and_then(Value::as_bool)
}

fn number_field(value: &Value, keys: &[&str]) -> Option<f64> {
    field(value, keys).and_then(|value| {
        value.as_f64().or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().parse::<f64>().ok())
        })
    })
}

fn normalize_window_id(name: &str, fallback_index: usize) -> String {
    let mut result = String::new();
    let mut last_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_dash = false;
        } else if !last_dash && !result.is_empty() {
            result.push('-');
            last_dash = true;
        }
    }
    while result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        format!("additional-{fallback_index}")
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quota_reset_metadata() {
        let state = parse_quota_error(
            r#"{"error":{"type":"usage_limit_reached","message":"limit reached","resets_in_seconds":120}}"#,
            "2026-05-17T00:00:00Z",
            "gpt-5.5",
        );
        assert_eq!(state.status, "cooldown");
        assert_eq!(state.resets_in_seconds, Some(120));
        assert_eq!(state.last_error.as_deref(), Some("limit reached"));
        assert_eq!(state.model.as_deref(), Some("gpt-5.5"));
    }

    #[test]
    fn parses_codex_usage_windows() {
        let payload = json!({
            "plan_type": "plus",
            "rate_limit": {
                "allowed": true,
                "primary_window": {
                    "used_percent": 21.5,
                    "limit_window_seconds": 18000,
                    "reset_at": 1770000000
                },
                "secondary_window": {
                    "used_percent": "74",
                    "limit_window_seconds": 604800,
                    "reset_after_seconds": 3600
                }
            },
            "code_review_rate_limit": {
                "limit_reached": true,
                "primary_window": {
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 120
                }
            },
            "additional_rate_limits": [{
                "limit_name": "custom feature",
                "rate_limit": {
                    "primary_window": {
                        "usedPercent": 9,
                        "limitWindowSeconds": 18000
                    }
                }
            }]
        });
        let windows = build_codex_usage_windows(&payload);
        assert_eq!(windows.len(), 4);
        assert_eq!(windows[0].id, "five-hour");
        assert_eq!(windows[0].used_percent, Some(21.5));
        assert_eq!(windows[1].id, "weekly");
        assert_eq!(windows[1].used_percent, Some(74.0));
        assert_eq!(windows[2].id, "code-review-five-hour");
        assert_eq!(windows[2].used_percent, Some(100.0));
        assert!(windows[2].limit_reached);
        assert_eq!(windows[3].id, "custom-feature-five-hour-0");
    }

    #[test]
    fn parses_codex_usage_nested_body() {
        let wrapped = r#"{"body":"{\"planType\":\"team\",\"rateLimit\":{\"primaryWindow\":{\"usedPercent\":\"33\",\"limitWindowSeconds\":\"18000\"}}}"}"#;
        let payload = parse_codex_usage_payload(wrapped).unwrap();
        assert_eq!(usage_plan_type(&payload).as_deref(), Some("team"));
        let windows = build_codex_usage_windows(&payload);
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "5 小时额度");
        assert_eq!(windows[0].used_percent, Some(33.0));
    }

    #[test]
    fn maps_usage_429_to_cooldown_state() {
        let state = usage_state_from_quota_error(
            r#"{"error":{"type":"usage_limit_reached","message":"limit reached","resets_in_seconds":120}}"#,
            "2026-05-17T00:00:00Z",
            429,
        );
        assert_eq!(state.status, "cooldown");
        assert_eq!(state.http_status, Some(429));
        assert_eq!(state.last_error.as_deref(), Some("limit reached"));
        assert!(state.resets_at.is_some());
    }
}
