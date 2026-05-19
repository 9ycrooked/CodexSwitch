use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use uuid::Uuid;

use crate::accounts::{
    auth_json_from_token_response, load_account, now_string, sanitize_id, save_account_record,
    summary_from_auth_json,
};
use crate::error::{run_blocking, stringify_io, AppResult};
use crate::models::{AccountSummary, OAuthLoginStart, TokenResponse};
use crate::settings::{self, load_settings};

const OAUTH_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
static OAUTH_PENDING: OnceLock<Mutex<Option<OAuthPending>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct OAuthPending {
    state: String,
    code_verifier: String,
    redirect_uri: String,
    profile_id: String,
    browser_profile_dir: PathBuf,
    window_label: String,
    cancel: Arc<AtomicBool>,
}

#[tauri::command]
pub fn start_oauth_login(app: AppHandle, profile_id: Option<String>) -> AppResult<OAuthLoginStart> {
    cancel_pending_oauth_login(&app);
    let settings = load_settings()?;
    let (port, listener) = bind_oauth_listener(settings.oauth_callback_port)?;
    let redirect_uri = format!("http://localhost:{port}/auth/callback");
    let state = Uuid::new_v4().simple().to_string();
    let (code_verifier, code_challenge) = generate_pkce_codes();
    let profile_id = sanitize_id(
        &profile_id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("login-{}", Uuid::new_v4().simple())),
    );
    let profile_dir = PathBuf::from(&settings.browser_profile_dir).join(&profile_id);
    fs::create_dir_all(&profile_dir).map_err(stringify_io)?;
    let window_label = format!("oauth-login-{profile_id}");
    let mode = settings::sanitize_oauth_login_mode(&settings.oauth_login_mode);

    listener
        .set_nonblocking(true)
        .map_err(|err| format!("OAuth callback server 初始化失败：{err}"))?;
    let cancel = Arc::new(AtomicBool::new(false));

    let pending = OAuthPending {
        state: state.clone(),
        code_verifier,
        redirect_uri: redirect_uri.clone(),
        profile_id: profile_id.clone(),
        browser_profile_dir: profile_dir.clone(),
        window_label: window_label.clone(),
        cancel: cancel.clone(),
    };
    *oauth_pending().lock().map_err(|err| err.to_string())? = Some(pending);

    let auth_url = build_oauth_url(&redirect_uri, &state, &code_challenge);
    if mode == "embedded" {
        open_oauth_webview(&app, &window_label, &auth_url, &profile_dir).map_err(|err| {
            let _ = oauth_pending().lock().map(|mut pending| pending.take());
            err
        })?;
    } else {
        open_oauth_external(&app, &auth_url, &profile_dir).map_err(|err| {
            let _ = oauth_pending().lock().map(|mut pending| pending.take());
            err
        })?;
    }

    let callback_app = app.clone();
    let callback_window_label = window_label.clone();
    thread::spawn(move || {
        let started_at = std::time::Instant::now();
        while !cancel.load(Ordering::Relaxed) && started_at.elapsed() < Duration::from_secs(15 * 60)
        {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let mut buffer = [0_u8; 8192];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let query = request
                        .lines()
                        .next()
                        .and_then(extract_query_from_request_line)
                        .unwrap_or_default();
                    let result = complete_oauth_login_internal(&query);
                    let close_window = result.is_ok();
                    let (status, body) = match result {
                        Ok(account) => (
                            "HTTP/1.1 200 OK",
                            format!(
                                "<html><body><h1>Codex 登录成功</h1><p>{}</p><p>可以关闭此窗口并返回账号切换器。</p></body></html>",
                                account.email.unwrap_or(account.display_name)
                            ),
                        ),
                        Err(err) => (
                            "HTTP/1.1 400 Bad Request",
                            format!(
                                "<html><body><h1>Codex 登录失败</h1><p>{err}</p></body></html>"
                            ),
                        ),
                    };
                    let response = format!(
                        "{status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.as_bytes().len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                    if close_window {
                        thread::sleep(Duration::from_millis(900));
                        if let Some(window) =
                            callback_app.get_webview_window(&callback_window_label)
                        {
                            let _ = window.close();
                        }
                    }
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(200));
                }
                Err(_) => break,
            }
        }
    });

    Ok(OAuthLoginStart {
        auth_url,
        profile_id,
        browser_profile_dir: profile_dir.to_string_lossy().to_string(),
        callback_port: port,
        state,
        mode,
    })
}

#[tauri::command]
pub fn close_oauth_login(app: AppHandle) -> AppResult<()> {
    cancel_pending_oauth_login(&app);
    Ok(())
}

fn cancel_pending_oauth_login(app: &AppHandle) {
    if let Ok(mut pending) = oauth_pending().lock() {
        if let Some(pending) = pending.take() {
            pending.cancel.store(true, Ordering::Relaxed);
            if let Some(window) = app.get_webview_window(&pending.window_label) {
                let _ = window.close();
            }
        }
    }
}

fn bind_oauth_listener(preferred_port: u16) -> AppResult<(u16, TcpListener)> {
    let start = preferred_port.max(1024);
    for offset in 0..50_u16 {
        let port = start.saturating_add(offset);
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            return Ok((port, listener));
        }
    }
    Err(format!(
        "OAuth callback 端口 {start} 到 {} 都无法监听，请稍后重试或改一个端口。",
        start.saturating_add(49)
    ))
}

#[tauri::command]
pub fn complete_oauth_login(callback_query: String) -> AppResult<AccountSummary> {
    complete_oauth_login_internal(&callback_query)
}

#[tauri::command]
pub async fn refresh_account_tokens(account_id: String) -> AppResult<AccountSummary> {
    run_blocking(move || refresh_account_tokens_blocking(account_id)).await
}

fn refresh_account_tokens_blocking(account_id: String) -> AppResult<AccountSummary> {
    let mut account = load_account(&account_id)?;
    let refresh_token = account
        .auth_json
        .pointer("/tokens/refresh_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "账号缺少 refresh_token。".to_string())?
        .to_string();
    let refreshed = refresh_tokens(&refresh_token)?;
    let auth_json = auth_json_from_token_response(&refreshed);
    let summary = summary_from_auth_json(&auth_json, Some(account.summary.clone()));
    account.auth_json = auth_json.clone();
    account.summary = summary.clone();
    save_account_record(&summary, &auth_json, &account.original_json)?;
    Ok(summary)
}

fn complete_oauth_login_internal(callback_query: &str) -> AppResult<AccountSummary> {
    let params = parse_query(callback_query);
    if let Some(error) = params.get("error") {
        return Err(params
            .get("error_description")
            .cloned()
            .unwrap_or_else(|| format!("OAuth 返回错误：{error}")));
    }
    let code = params
        .get("code")
        .ok_or_else(|| "OAuth callback 缺少 code。".to_string())?
        .to_string();
    let state = params
        .get("state")
        .ok_or_else(|| "OAuth callback 缺少 state。".to_string())?
        .to_string();
    let pending = oauth_pending()
        .lock()
        .map_err(|err| err.to_string())?
        .take()
        .ok_or_else(|| "没有等待中的 OAuth 登录。".to_string())?;
    if state != pending.state {
        return Err("OAuth state 校验失败。".into());
    }
    let _window_label = pending.window_label.clone();
    let token = exchange_code_for_tokens(&code, &pending.redirect_uri, &pending.code_verifier)?;
    let auth_json = auth_json_from_token_response(&token);
    let mut summary = summary_from_auth_json(&auth_json, None);
    summary.id = sanitize_id(
        &summary
            .account_id
            .clone()
            .or(summary.email.clone())
            .unwrap_or_else(|| pending.profile_id.clone()),
    );
    summary.browser_profile_dir = Some(pending.browser_profile_dir.to_string_lossy().to_string());
    summary.imported_at = now_string();
    summary.has_config = false;
    let original = flat_oauth_json_from_auth_json(&auth_json, &summary);
    save_account_record(&summary, &auth_json, &original)?;
    Ok(summary)
}

fn oauth_pending() -> &'static Mutex<Option<OAuthPending>> {
    OAUTH_PENDING.get_or_init(|| Mutex::new(None))
}

fn generate_pkce_codes() -> (String, String) {
    let mut bytes = [0_u8; 96];
    rand::thread_rng().fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn build_oauth_url(redirect_uri: &str, state: &str, code_challenge: &str) -> String {
    let params = [
        ("client_id", OAUTH_CLIENT_ID),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("scope", "openid email profile offline_access"),
        ("state", state),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("prompt", "login"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
    ];
    let encoded = params
        .iter()
        .map(|(key, value)| format!("{key}={}", urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{OAUTH_AUTH_URL}?{encoded}")
}

fn open_oauth_webview(
    app: &AppHandle,
    window_label: &str,
    auth_url: &str,
    profile_dir: &Path,
) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(window_label) {
        let _ = window.close();
    }
    let url: tauri::Url = auth_url
        .parse()
        .map_err(|err| format!("OAuth URL 解析失败：{err}"))?;
    WebviewWindowBuilder::new(app, window_label, WebviewUrl::External(url))
        .title("Codex OAuth 登录")
        .inner_size(520.0, 760.0)
        .min_inner_size(420.0, 560.0)
        .data_directory(profile_dir.to_path_buf())
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0",
        )
        .devtools(true)
        .build()
        .map_err(|err| format!("创建 OAuth WebView 窗口失败：{err}"))?;
    Ok(())
}

fn open_oauth_external(app: &AppHandle, auth_url: &str, profile_dir: &Path) -> AppResult<()> {
    let chrome_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    ];
    let (width, height) = oauth_external_window_size(app);
    if let Some(browser) = chrome_paths
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
    {
        let browser = browser.to_string_lossy();
        crate::process::hidden_command(browser.as_ref())
            .arg(format!("--user-data-dir={}", profile_dir.to_string_lossy()))
            .arg("--new-window")
            .arg(format!("--window-size={width},{height}"))
            .arg(auth_url)
            .spawn()
            .map_err(stringify_io)?;
        return Ok(());
    }
    crate::process::hidden_command("rundll32")
        .args(["url.dll,FileProtocolHandler", auth_url])
        .spawn()
        .map_err(stringify_io)?;
    Ok(())
}

fn oauth_external_window_size(app: &AppHandle) -> (u32, u32) {
    app.get_webview_window("main")
        .and_then(|window| window.outer_size().ok())
        .map(|size| (size.width.max(520), size.height.max(560)))
        .unwrap_or((1120, 760))
}

fn exchange_code_for_tokens(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<TokenResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .post(OAUTH_TOKEN_URL)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", OAUTH_CLIENT_ID),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
        ])
        .send()
        .map_err(|err| format!("token exchange 请求失败：{err}"))?;
    parse_token_http_response(response, "token exchange")
}

fn refresh_tokens(refresh_token: &str) -> AppResult<TokenResponse> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .post(OAUTH_TOKEN_URL)
        .form(&[
            ("client_id", OAUTH_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", "openid profile email"),
        ])
        .send()
        .map_err(|err| format!("token refresh 请求失败：{err}"))?;
    let mut token = parse_token_http_response(response, "token refresh")?;
    if token.refresh_token.trim().is_empty() {
        token.refresh_token = refresh_token.to_string();
    }
    Ok(token)
}

fn format_token_error(label: &str, status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let code = parsed.pointer("/error/code").and_then(Value::as_str);
    if code == Some("unsupported_country_region_territory") {
        return format!(
            "{label} 失败：后端请求被 OpenAI 判定为不支持地区。浏览器登录窗口和软件后端可能没有使用同一个网络出口，请先在设置里运行登录前网络检查。原始响应：HTTP {status}: {body}"
        );
    }
    format!("{label} 失败：HTTP {status}: {body}")
}

fn parse_token_http_response(
    response: reqwest::blocking::Response,
    label: &str,
) -> AppResult<TokenResponse> {
    let status = response.status();
    let body = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format_token_error(label, status.as_u16(), &body));
    }
    serde_json::from_str(&body).map_err(|err| format!("{label} 响应解析失败：{err}"))
}

fn flat_oauth_json_from_auth_json(auth_json: &Value, summary: &AccountSummary) -> Value {
    json!({
        "access_token": auth_json.pointer("/tokens/access_token").and_then(Value::as_str).unwrap_or_default(),
        "id_token": auth_json.pointer("/tokens/id_token").and_then(Value::as_str).unwrap_or_default(),
        "refresh_token": auth_json.pointer("/tokens/refresh_token").and_then(Value::as_str).unwrap_or_default(),
        "account_id": summary.account_id,
        "email": summary.email,
        "expired": summary.expired,
        "disabled": summary.disabled,
        "type": "codex",
        "last_refresh": auth_json.get("last_refresh").and_then(Value::as_str).unwrap_or_default()
    })
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let query = query.trim().trim_start_matches('?');
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .filter_map(|part| {
            let mut pieces = part.splitn(2, '=');
            let key = pieces.next()?;
            let value = pieces.next().unwrap_or_default();
            Some((
                urlencoding::decode(key).ok()?.into_owned(),
                urlencoding::decode(value).ok()?.into_owned(),
            ))
        })
        .collect()
}

fn extract_query_from_request_line(line: &str) -> Option<String> {
    let path = line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    Some(query.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_pkce_verifier_and_challenge() {
        let (verifier, challenge) = generate_pkce_codes();
        assert!(verifier.len() >= 43);
        assert!(!verifier.contains('='));
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
    }

    #[test]
    fn validates_oauth_callback_query_values() {
        let parsed = parse_query("code=abc%20123&state=state-1");
        assert_eq!(parsed.get("code").map(String::as_str), Some("abc 123"));
        assert_eq!(parsed.get("state").map(String::as_str), Some("state-1"));
        let query = extract_query_from_request_line("GET /auth/callback?code=abc&state=s HTTP/1.1");
        assert_eq!(query.as_deref(), Some("code=abc&state=s"));
    }

    #[test]
    fn formats_unsupported_region_token_error() {
        let body = r#"{"error":{"code":"unsupported_country_region_territory","message":"Country, region, or territory not supported","type":"request_forbidden"}}"#;
        let message = format_token_error("token exchange", 403, body);
        assert!(message.contains("后端请求被 OpenAI 判定为不支持地区"));
        assert!(message.contains("浏览器登录窗口和软件后端可能没有使用同一个网络出口"));
    }
}
