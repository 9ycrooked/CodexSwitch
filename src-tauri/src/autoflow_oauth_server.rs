use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::error::{run_blocking, AppResult};
use crate::models::{
    AccountSummary, AutoFlowExchangeCodeResponse, AutoFlowGenerateAuthUrlResponse,
    AutoFlowOAuthServerStatus, TokenResponse,
};
use crate::oauth;
use crate::settings::{load_settings, sanitize_settings, save_settings, Settings};

pub(crate) const SESSION_TTL: Duration = Duration::from_secs(10 * 60);
static SERVER_STATE: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
static OAUTH_SESSIONS: OnceLock<Mutex<HashMap<String, AutoFlowOAuthSession>>> = OnceLock::new();

#[derive(Debug)]
struct RunningServer {
    port: u16,
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub(crate) struct AutoFlowOAuthSession {
    state: String,
    code_verifier: String,
    redirect_uri: String,
    expires_at: Instant,
    used: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct HttpResponse {
    status: u16,
    body: String,
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Debug, Deserialize)]
struct ExchangeCodeRequest {
    session_id: String,
    code: String,
    state: String,
}

#[tauri::command]
pub async fn start_autoflow_oauth_server() -> AppResult<AutoFlowOAuthServerStatus> {
    run_blocking(start_autoflow_oauth_server_blocking).await
}

#[tauri::command]
pub async fn stop_autoflow_oauth_server() -> AppResult<AutoFlowOAuthServerStatus> {
    run_blocking(stop_autoflow_oauth_server_blocking).await
}

#[tauri::command]
pub fn get_autoflow_oauth_server_status() -> AppResult<AutoFlowOAuthServerStatus> {
    let settings = sanitize_settings(load_settings()?)?;
    Ok(status_from_state(&settings))
}

#[tauri::command]
pub fn reset_autoflow_oauth_admin_key() -> AppResult<Settings> {
    let mut settings = sanitize_settings(load_settings()?)?;
    settings.autoflow_oauth_admin_key = generate_admin_key();
    save_settings(&settings)?;
    Ok(settings)
}

fn start_autoflow_oauth_server_blocking() -> AppResult<AutoFlowOAuthServerStatus> {
    let mut settings = sanitize_settings(load_settings()?)?;
    let already_running = {
        let state = server_state().lock().map_err(|err| err.to_string())?;
        state.is_some()
    };
    if already_running {
        if !settings.autoflow_oauth_server_enabled {
            settings.autoflow_oauth_server_enabled = true;
            save_settings(&settings)?;
        }
        return Ok(status_from_state(&settings));
    }

    if settings.autoflow_oauth_admin_key.trim().is_empty() {
        settings.autoflow_oauth_admin_key = generate_admin_key();
    }
    let port = settings.autoflow_oauth_server_port;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|err| format!("AutoFlow OAuth 服务启动失败：{err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("AutoFlow OAuth 服务初始化失败：{err}"))?;

    let cancel = Arc::new(AtomicBool::new(false));
    let thread_cancel = cancel.clone();
    let join = thread::spawn(move || run_server_loop(listener, thread_cancel));

    {
        let mut state = server_state().lock().map_err(|err| err.to_string())?;
        *state = Some(RunningServer {
            port,
            cancel,
            join: Some(join),
        });
    }

    settings.autoflow_oauth_server_enabled = true;
    save_settings(&settings)?;
    Ok(status_from_state(&settings))
}

fn stop_autoflow_oauth_server_blocking() -> AppResult<AutoFlowOAuthServerStatus> {
    let mut settings = sanitize_settings(load_settings()?)?;
    let running = {
        let mut state = server_state().lock().map_err(|err| err.to_string())?;
        state.take()
    };

    if let Some(mut running) = running {
        running.cancel.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", running.port));
        if let Some(join) = running.join.take() {
            let _ = join.join();
        }
    }

    oauth_sessions()
        .lock()
        .map_err(|err| err.to_string())?
        .clear();
    settings.autoflow_oauth_server_enabled = false;
    save_settings(&settings)?;
    Ok(status_from_state(&settings))
}

fn status_from_state(settings: &Settings) -> AutoFlowOAuthServerStatus {
    let running_port = server_state()
        .lock()
        .ok()
        .and_then(|state| state.as_ref().map(|running| running.port));
    let port = running_port.unwrap_or(settings.autoflow_oauth_server_port);
    AutoFlowOAuthServerStatus {
        running: running_port.is_some(),
        port,
        url: format!("http://127.0.0.1:{port}/admin/accounts"),
        admin_key_configured: !settings.autoflow_oauth_admin_key.trim().is_empty(),
    }
}

fn run_server_loop(listener: TcpListener, cancel: Arc<AtomicBool>) {
    while !cancel.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let response = match read_http_request(&mut stream) {
                    Ok(request) => match load_settings().and_then(sanitize_settings) {
                        Ok(settings) => handle_http_request(
                            request,
                            &settings.autoflow_oauth_admin_key,
                            settings.oauth_callback_port,
                        ),
                        Err(err) => json_error(500, &err),
                    },
                    Err(err) => json_error(400, &err),
                };
                let _ = stream.write_all(&response.to_bytes());
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }
}

fn read_http_request(stream: &mut TcpStream) -> AppResult<HttpRequest> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| err.to_string())?;
    let mut buffer = Vec::new();
    let mut temp = [0_u8; 4096];
    let headers_end;
    loop {
        let read = stream.read(&mut temp).map_err(|err| err.to_string())?;
        if read == 0 {
            return Err("HTTP 请求为空。".to_string());
        }
        buffer.extend_from_slice(&temp[..read]);
        if let Some(index) = find_header_end(&buffer) {
            headers_end = index;
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err("HTTP 请求过大。".to_string());
        }
    }

    let head = String::from_utf8(buffer[..headers_end].to_vec())
        .map_err(|_| "HTTP 请求头不是有效 UTF-8。".to_string())?;
    let content_length = parse_content_length(&head)?;
    let body_start = headers_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut temp).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);
    }
    if buffer.len() < body_start + content_length {
        return Err("HTTP 请求体不完整。".to_string());
    }
    parse_http_request_parts(&head, &buffer[body_start..body_start + content_length])
}

fn parse_http_request_parts(head: &str, body: &[u8]) -> AppResult<HttpRequest> {
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "HTTP 请求行缺失。".to_string())?;
    let mut pieces = request_line.split_whitespace();
    let method = pieces
        .next()
        .ok_or_else(|| "HTTP 方法缺失。".to_string())?
        .to_string();
    let path = pieces
        .next()
        .ok_or_else(|| "HTTP 路径缺失。".to_string())?
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let body =
        String::from_utf8(body.to_vec()).map_err(|_| "HTTP 请求体不是有效 UTF-8。".to_string())?;
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn parse_content_length(head: &str) -> AppResult<usize> {
    for line in head.lines().skip(1) {
        if let Some((key, value)) = line.split_once(':') {
            if key.trim().eq_ignore_ascii_case("content-length") {
                return value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "Content-Length 无效。".to_string());
            }
        }
    }
    Ok(0)
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn handle_http_request(request: HttpRequest, admin_key: &str, callback_port: u16) -> HttpResponse {
    if request.method != "POST" {
        return json_error(405, "只支持 POST 请求。");
    }
    if request.path != "/api/admin/oauth/generate-auth-url"
        && request.path != "/api/admin/oauth/exchange-code"
    {
        return json_error(404, "接口不存在。");
    }
    if let Err(response) = validate_admin_key(
        admin_key,
        request.headers.get("x-admin-key").map(String::as_str),
    ) {
        return response;
    }

    match request.path.as_str() {
        "/api/admin/oauth/generate-auth-url" => {
            if !request.body.trim().is_empty()
                && serde_json::from_str::<serde_json::Value>(&request.body).is_err()
            {
                return json_error(400, "JSON 请求体无效。");
            }
            let redirect_uri = format!("http://localhost:{callback_port}/auth/callback");
            let mut sessions = match oauth_sessions().lock() {
                Ok(sessions) => sessions,
                Err(err) => return json_error(500, &err.to_string()),
            };
            match generate_auth_url_for_sessions(&mut sessions, &redirect_uri, Instant::now()) {
                Ok(response) => json_response(200, &response),
                Err(err) => json_error(400, &err),
            }
        }
        "/api/admin/oauth/exchange-code" => {
            let payload = match serde_json::from_str::<ExchangeCodeRequest>(&request.body) {
                Ok(payload) => payload,
                Err(_) => return json_error(400, "JSON 请求体无效。"),
            };
            let mut sessions = match oauth_sessions().lock() {
                Ok(sessions) => sessions,
                Err(err) => return json_error(500, &err.to_string()),
            };
            match exchange_code_for_sessions(
                &mut sessions,
                &payload.session_id,
                &payload.code,
                &payload.state,
                Instant::now(),
                oauth::exchange_code_for_tokens,
                oauth::save_token_response_as_account,
            ) {
                Ok(response) => json_response(200, &response),
                Err(err) => json_error(400, &err),
            }
        }
        _ => json_error(404, "接口不存在。"),
    }
}

pub(crate) fn validate_admin_key(
    expected: &str,
    provided: Option<&str>,
) -> Result<(), HttpResponse> {
    if expected.trim().is_empty() {
        return Err(json_error(401, "管理密钥未配置。"));
    }
    let Some(provided) = provided.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(json_error(401, "缺少管理密钥。"));
    };
    if provided != expected {
        return Err(json_error(401, "管理密钥无效。"));
    }
    Ok(())
}

pub(crate) fn generate_auth_url_for_sessions(
    sessions: &mut HashMap<String, AutoFlowOAuthSession>,
    redirect_uri: &str,
    now: Instant,
) -> AppResult<AutoFlowGenerateAuthUrlResponse> {
    prune_expired_sessions(sessions, now);
    let session_id = format!("sess_{}", Uuid::new_v4().simple());
    let state = Uuid::new_v4().simple().to_string();
    let (code_verifier, code_challenge) = oauth::generate_pkce_codes();
    let auth_url = oauth::build_oauth_url(redirect_uri, &state, &code_challenge);
    sessions.insert(
        session_id.clone(),
        AutoFlowOAuthSession {
            state,
            code_verifier,
            redirect_uri: redirect_uri.to_string(),
            expires_at: now + SESSION_TTL,
            used: false,
        },
    );
    Ok(AutoFlowGenerateAuthUrlResponse {
        auth_url,
        session_id,
    })
}

pub(crate) fn exchange_code_for_sessions<E, S>(
    sessions: &mut HashMap<String, AutoFlowOAuthSession>,
    session_id: &str,
    code: &str,
    state: &str,
    now: Instant,
    exchange_fn: E,
    save_fn: S,
) -> AppResult<AutoFlowExchangeCodeResponse>
where
    E: FnOnce(&str, &str, &str) -> AppResult<TokenResponse>,
    S: FnOnce(&TokenResponse, &str, Option<String>) -> AppResult<AccountSummary>,
{
    if code.trim().is_empty() {
        return Err("OAuth code 不能为空。".to_string());
    }
    let session = sessions
        .get_mut(session_id)
        .ok_or_else(|| "OAuth 会话不存在。".to_string())?;
    if session.used {
        return Err("OAuth 会话已使用。".to_string());
    }
    if now > session.expires_at {
        return Err("OAuth 会话已过期。".to_string());
    }
    if state != session.state {
        return Err("OAuth state 校验失败。".to_string());
    }

    let token = exchange_fn(code.trim(), &session.redirect_uri, &session.code_verifier)?;
    let fallback_id = format!("autoflow-{}", Uuid::new_v4().simple());
    let account = save_fn(&token, &fallback_id, None)?;
    session.used = true;
    Ok(AutoFlowExchangeCodeResponse {
        message: "账号已添加。".to_string(),
        id: account.id,
        email: account.email,
    })
}

fn prune_expired_sessions(sessions: &mut HashMap<String, AutoFlowOAuthSession>, now: Instant) {
    sessions.retain(|_, session| !session.used && now <= session.expires_at);
}

fn json_response<T: serde::Serialize>(status: u16, payload: &T) -> HttpResponse {
    match serde_json::to_string(payload) {
        Ok(body) => HttpResponse { status, body },
        Err(err) => json_error(500, &err.to_string()),
    }
}

fn json_error(status: u16, message: &str) -> HttpResponse {
    HttpResponse {
        status,
        body: json!({ "message": message }).to_string(),
    }
}

impl HttpResponse {
    fn reason(&self) -> &'static str {
        match self.status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            _ => "OK",
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let body = self.body.as_bytes();
        format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.status,
            self.reason(),
            body.len(),
            self.body
        )
        .into_bytes()
    }
}

fn generate_admin_key() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn server_state() -> &'static Mutex<Option<RunningServer>> {
    SERVER_STATE.get_or_init(|| Mutex::new(None))
}

fn oauth_sessions() -> &'static Mutex<HashMap<String, AutoFlowOAuthSession>> {
    OAUTH_SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TokenResponse;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn validate_admin_key_rejects_missing_and_invalid_keys() {
        let missing = validate_admin_key("expected-key", None).expect_err("missing key rejects");
        assert_eq!(missing.status, 401);
        assert!(missing.body.contains("缺少管理密钥"));

        let invalid =
            validate_admin_key("expected-key", Some("wrong-key")).expect_err("invalid key rejects");
        assert_eq!(invalid.status, 401);
        assert!(invalid.body.contains("管理密钥无效"));

        let unconfigured =
            validate_admin_key("", Some("expected-key")).expect_err("unconfigured key rejects");
        assert_eq!(unconfigured.status, 401);
        assert!(unconfigured.body.contains("管理密钥未配置"));
    }

    #[test]
    fn generate_auth_url_stores_session_with_redirect_uri() {
        let mut sessions = HashMap::new();

        let response = generate_auth_url_for_sessions(
            &mut sessions,
            "http://localhost:1455/auth/callback",
            Instant::now(),
        )
        .expect("auth url should be generated");

        assert!(response.session_id.starts_with("sess_"));
        assert!(response
            .auth_url
            .contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        let session = sessions
            .get(&response.session_id)
            .expect("session should be stored");
        assert_eq!(session.redirect_uri, "http://localhost:1455/auth/callback");
        assert!(response
            .auth_url
            .contains(&format!("state={}", session.state)));
    }

    #[test]
    fn exchange_code_rejects_state_mismatch() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "sess_test".to_string(),
            AutoFlowOAuthSession {
                state: "state-1".to_string(),
                code_verifier: "verifier".to_string(),
                redirect_uri: "http://localhost:1455/auth/callback".to_string(),
                expires_at: Instant::now() + SESSION_TTL,
                used: false,
            },
        );

        let err = exchange_code_for_sessions(
            &mut sessions,
            "sess_test",
            "code",
            "state-2",
            Instant::now(),
            |_code, _redirect_uri, _verifier| panic!("exchange should not be called"),
            |_token, _fallback_id, _browser_profile_dir| panic!("save should not be called"),
        )
        .expect_err("state mismatch rejects");

        assert_eq!(err, "OAuth state 校验失败。");
    }

    #[test]
    fn exchange_code_rejects_empty_expired_and_used_sessions() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "sess_used".to_string(),
            AutoFlowOAuthSession {
                state: "state".to_string(),
                code_verifier: "verifier".to_string(),
                redirect_uri: "http://localhost:1455/auth/callback".to_string(),
                expires_at: Instant::now() + SESSION_TTL,
                used: true,
            },
        );
        sessions.insert(
            "sess_expired".to_string(),
            AutoFlowOAuthSession {
                state: "state".to_string(),
                code_verifier: "verifier".to_string(),
                redirect_uri: "http://localhost:1455/auth/callback".to_string(),
                expires_at: Instant::now() - Duration::from_secs(1),
                used: false,
            },
        );

        let used = exchange_code_for_sessions(
            &mut sessions,
            "sess_used",
            "code",
            "state",
            Instant::now(),
            |_code, _redirect_uri, _verifier| panic!("exchange should not be called"),
            |_token, _fallback_id, _browser_profile_dir| panic!("save should not be called"),
        )
        .expect_err("used session rejects");
        assert_eq!(used, "OAuth 会话已使用。");

        let expired = exchange_code_for_sessions(
            &mut sessions,
            "sess_expired",
            "code",
            "state",
            Instant::now(),
            |_code, _redirect_uri, _verifier| panic!("exchange should not be called"),
            |_token, _fallback_id, _browser_profile_dir| panic!("save should not be called"),
        )
        .expect_err("expired session rejects");
        assert_eq!(expired, "OAuth 会话已过期。");

        let empty_code = exchange_code_for_sessions(
            &mut sessions,
            "sess_expired",
            "",
            "state",
            Instant::now(),
            |_code, _redirect_uri, _verifier| panic!("exchange should not be called"),
            |_token, _fallback_id, _browser_profile_dir| panic!("save should not be called"),
        )
        .expect_err("empty code rejects");
        assert_eq!(empty_code, "OAuth code 不能为空。");
    }

    #[test]
    fn exchange_code_saves_account_and_marks_session_used() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "sess_ok".to_string(),
            AutoFlowOAuthSession {
                state: "state".to_string(),
                code_verifier: "verifier".to_string(),
                redirect_uri: "http://localhost:1455/auth/callback".to_string(),
                expires_at: Instant::now() + SESSION_TTL,
                used: false,
            },
        );

        let response = exchange_code_for_sessions(
            &mut sessions,
            "sess_ok",
            " code-1 ",
            "state",
            Instant::now(),
            |code, redirect_uri, verifier| {
                assert_eq!(code, "code-1");
                assert_eq!(redirect_uri, "http://localhost:1455/auth/callback");
                assert_eq!(verifier, "verifier");
                Ok(TokenResponse {
                    access_token: "access".to_string(),
                    refresh_token: "refresh".to_string(),
                    id_token: "id".to_string(),
                    expires_in: 3600,
                })
            },
            |_token, fallback_id, browser_profile_dir| {
                assert!(fallback_id.starts_with("autoflow-"));
                assert!(browser_profile_dir.is_none());
                Ok(crate::models::AccountSummary {
                    id: "acct-1".to_string(),
                    display_name: "autoflow@example.com".to_string(),
                    email: Some("autoflow@example.com".to_string()),
                    account_id: Some("acct-1".to_string()),
                    plan: None,
                    expired: None,
                    disabled: false,
                    imported_at: String::new(),
                    has_config: false,
                    browser_profile_dir: None,
                    oauth_metadata: None,
                    quota_state: None,
                    usage_state: None,
                })
            },
        )
        .expect("exchange should succeed");

        assert_eq!(response.message, "账号已添加。");
        assert_eq!(response.id, "acct-1");
        assert_eq!(response.email.as_deref(), Some("autoflow@example.com"));
        assert!(sessions.get("sess_ok").expect("session exists").used);
    }

    #[test]
    fn parses_simple_json_post_request_parts() {
        let request = parse_http_request_parts(
            "POST /api/admin/oauth/generate-auth-url HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Admin-Key: secret\r\nContent-Length: 2",
            b"{}",
        )
        .expect("request should parse");

        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/api/admin/oauth/generate-auth-url");
        assert_eq!(
            request.headers.get("x-admin-key").map(String::as_str),
            Some("secret")
        );
        assert_eq!(request.body, "{}");
    }

    #[test]
    fn handle_http_request_returns_json_for_bad_method_and_invalid_json() {
        let bad_method = handle_http_request(
            HttpRequest {
                method: "GET".to_string(),
                path: "/api/admin/oauth/generate-auth-url".to_string(),
                headers: HashMap::from([("x-admin-key".to_string(), "secret".to_string())]),
                body: "{}".to_string(),
            },
            "secret",
            1455,
        );
        assert_eq!(bad_method.status, 405);
        assert!(bad_method.body.contains("\"message\""));
        let raw = String::from_utf8(bad_method.to_bytes()).expect("response is utf8");
        assert!(raw.contains("Content-Type: application/json; charset=utf-8"));

        let invalid_json = handle_http_request(
            HttpRequest {
                method: "POST".to_string(),
                path: "/api/admin/oauth/generate-auth-url".to_string(),
                headers: HashMap::from([("x-admin-key".to_string(), "secret".to_string())]),
                body: "{".to_string(),
            },
            "secret",
            1455,
        );
        assert_eq!(invalid_json.status, 400);
        assert!(invalid_json.body.contains("JSON 请求体无效"));
    }

    #[test]
    fn handle_generate_auth_url_endpoint_stores_global_session() {
        oauth_sessions().lock().expect("sessions lock").clear();
        let response = handle_http_request(
            HttpRequest {
                method: "POST".to_string(),
                path: "/api/admin/oauth/generate-auth-url".to_string(),
                headers: HashMap::from([("x-admin-key".to_string(), "secret".to_string())]),
                body: "{}".to_string(),
            },
            "secret",
            1455,
        );

        assert_eq!(response.status, 200);
        let payload: crate::models::AutoFlowGenerateAuthUrlResponse =
            serde_json::from_str(&response.body).expect("response json should parse");
        assert!(payload
            .auth_url
            .contains("http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
        assert!(oauth_sessions()
            .lock()
            .expect("sessions lock")
            .contains_key(&payload.session_id));
        oauth_sessions().lock().expect("sessions lock").clear();
    }
}
