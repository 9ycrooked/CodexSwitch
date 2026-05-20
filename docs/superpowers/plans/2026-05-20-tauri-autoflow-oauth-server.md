# Tauri AutoFlow OAuth Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a user-started, configurable local AutoFlow OAuth integration service to the Tauri backend with Codex2API-compatible `generate-auth-url` and `exchange-code` endpoints.

**Architecture:** Add a focused Rust module that owns a local `127.0.0.1` HTTP listener, in-memory OAuth sessions, admin-key validation, and endpoint JSON contracts. Reuse existing OAuth token exchange and account persistence helpers from `oauth.rs` and `accounts.rs`, and expose Tauri commands so the Vue settings page can start, stop, copy, and reset the service key.

**Tech Stack:** Rust 2021, Tauri 2, Vue 3, TypeScript, `std::net::TcpListener`, existing `serde`/`serde_json`/`reqwest`/`uuid`/`rand` dependencies.

---

## File Map

- Modify `src-tauri/src/settings.rs`: add persistent AutoFlow server settings, key generation helper, settings save helper, validation.
- Modify `src-tauri/src/models.rs`: add serializable server status and endpoint response/request DTOs shared by commands and UI.
- Modify `src-tauri/src/oauth.rs`: expose PKCE/auth-url/token-exchange helpers and add a reusable `save_token_response_as_account` helper.
- Create `src-tauri/src/autoflow_oauth_server.rs`: local HTTP server lifecycle, request parsing, endpoint handlers, session store, admin-key reset command.
- Modify `src-tauri/src/lib.rs`: register the new module and Tauri commands.
- Modify `src/types.ts`: mirror new settings and server status types.
- Modify `src/api/codexSwitchApi.ts`: add start/stop/status/reset server command wrappers.
- Modify `src/App.vue`: load server status, wire settings save fields, call server commands, show toasts.
- Modify `src/views/SettingsView.vue`: add AutoFlow integration controls.
- Modify `src/styles/views.css`: style the integration panel without nesting cards inside cards.

Do not modify AutoFlow Chrome extension files. Do not modify `CLIProxyAPI/`.

## Task 1: Settings Model And Frontend Types

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/types.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Write Rust settings compatibility tests**

Add this test to the existing `#[cfg(test)] mod tests` in `src-tauri/src/settings.rs`:

```rust
#[test]
fn settings_defaults_autoflow_fields_when_missing() {
    let raw = r#"{
        "codex_home": "C:\\Users\\Y\\.codex",
        "process_names": ["Codex.exe"],
        "close_timeout_ms": 3000,
        "browser_profile_dir": "profiles",
        "oauth_callback_port": 1455,
        "keep_login_profiles": true,
        "oauth_login_mode": "external",
        "check_updates_on_startup": true,
        "force_update_on_startup": false,
        "check_oauth_network_on_login": true,
        "check_egress_region": false
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

    let err = sanitize_settings(settings).expect_err("low service port should fail");

    assert_eq!(err, "AutoFlow 接入服务端口不能小于 1024。");
}
```

- [ ] **Step 2: Run settings tests and verify they fail**

Run:

```bash
cd src-tauri
cargo test settings_defaults_autoflow_fields_when_missing update_settings_rejects_invalid_autoflow_port
```

Expected: FAIL because `Settings` has no `autoflow_oauth_server_enabled`, `autoflow_oauth_server_port`, `autoflow_oauth_admin_key`, and `sanitize_settings` does not exist.

- [ ] **Step 3: Add settings fields, defaults, sanitizer, and save helper**

In `src-tauri/src/settings.rs`, extend `Settings`:

```rust
    #[serde(default)]
    pub autoflow_oauth_server_enabled: bool,
    #[serde(default = "default_autoflow_oauth_server_port")]
    pub autoflow_oauth_server_port: u16,
    #[serde(default)]
    pub autoflow_oauth_admin_key: String,
```

Update `default_settings()`:

```rust
        autoflow_oauth_server_enabled: false,
        autoflow_oauth_server_port: default_autoflow_oauth_server_port(),
        autoflow_oauth_admin_key: String::new(),
```

Replace the body of `update_settings` with a call to a reusable sanitizer and saver:

```rust
#[tauri::command]
pub fn update_settings(settings: Settings) -> AppResult<Settings> {
    let sanitized = sanitize_settings(settings)?;
    save_settings(&sanitized)?;
    Ok(sanitized)
}
```

Add these helpers below `update_settings`:

```rust
pub(crate) fn sanitize_settings(settings: Settings) -> AppResult<Settings> {
    if settings.codex_home.trim().is_empty() {
        return Err("Codex home 不能为空。".into());
    }
    if settings.process_names.is_empty() {
        return Err("至少需要一个 Codex 进程名。".into());
    }
    if settings.close_timeout_ms < 500 {
        return Err("关闭超时不能小于 500ms。".into());
    }
    if settings.oauth_callback_port < 1024 {
        return Err("OAuth callback 端口不能小于 1024。".into());
    }
    if settings.autoflow_oauth_server_port < 1024 {
        return Err("AutoFlow 接入服务端口不能小于 1024。".into());
    }

    Ok(Settings {
        codex_home: settings.codex_home,
        process_names: settings
            .process_names
            .into_iter()
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .collect(),
        close_timeout_ms: settings.close_timeout_ms,
        browser_profile_dir: settings.browser_profile_dir,
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
```

Add the default function:

```rust
fn default_autoflow_oauth_server_port() -> u16 {
    8080
}
```

- [ ] **Step 4: Update TypeScript settings type and reactive defaults**

In `src/types.ts`, extend `Settings`:

```ts
  autoflow_oauth_server_enabled: boolean;
  autoflow_oauth_server_port: number;
  autoflow_oauth_admin_key: string;
```

In `src/App.vue`, extend the `settings = reactive<Settings>({...})` object:

```ts
  autoflow_oauth_server_enabled: false,
  autoflow_oauth_server_port: 8080,
  autoflow_oauth_admin_key: ""
```

In `saveSettings()`, include the same fields in the object passed to `api.updateSettings`:

```ts
      autoflow_oauth_server_enabled: Boolean(settings.autoflow_oauth_server_enabled),
      autoflow_oauth_server_port: Number(settings.autoflow_oauth_server_port),
      autoflow_oauth_admin_key: settings.autoflow_oauth_admin_key
```

- [ ] **Step 5: Run settings tests and frontend typecheck**

Run:

```bash
cd src-tauri
cargo test settings_defaults_autoflow_fields_when_missing update_settings_rejects_invalid_autoflow_port
```

Expected: PASS.

Run from repo root:

```bash
yarn build
```

Expected: PASS through `vue-tsc --noEmit && vite build`.

- [ ] **Step 6: Commit settings model**

```bash
git add src-tauri/src/settings.rs src/types.ts src/App.vue
git commit -m "feat: add AutoFlow OAuth service settings"
```

## Task 2: Reusable OAuth Account-Save Helpers

**Files:**
- Modify: `src-tauri/src/oauth.rs`

- [ ] **Step 1: Add tests for reusable OAuth account saving**

Add this test to `src-tauri/src/oauth.rs` under existing tests:

```rust
#[test]
fn saves_token_response_as_account_summary() {
    let claims = json!({
        "email": "autoflow@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_account_id": "acct-autoflow",
            "chatgpt_plan_type": "plus"
        }
    });
    let token = TokenResponse {
        access_token: "access-token".to_string(),
        refresh_token: "refresh-token".to_string(),
        id_token: format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        ),
        expires_in: 3600,
    };

    let summary = account_summary_from_token_response(&token, "fallback-profile", None)
        .expect("summary should be generated");

    assert_eq!(summary.id, "acct-autoflow");
    assert_eq!(summary.email.as_deref(), Some("autoflow@example.com"));
    assert_eq!(summary.account_id.as_deref(), Some("acct-autoflow"));
    assert_eq!(summary.plan.as_deref(), Some("plus"));
}
```

- [ ] **Step 2: Run the new OAuth helper test and verify it fails**

Run:

```bash
cd src-tauri
cargo test saves_token_response_as_account_summary
```

Expected: FAIL because `account_summary_from_token_response` is not defined.

- [ ] **Step 3: Expose existing OAuth helpers and add account summary helper**

In `src-tauri/src/oauth.rs`, change these functions from private to crate-visible:

```rust
pub(crate) fn generate_pkce_codes() -> (String, String) {
```

```rust
pub(crate) fn build_oauth_url(redirect_uri: &str, state: &str, code_challenge: &str) -> String {
```

```rust
pub(crate) fn exchange_code_for_tokens(
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> AppResult<TokenResponse> {
```

```rust
pub(crate) fn flat_oauth_json_from_auth_json(auth_json: &Value, summary: &AccountSummary) -> Value {
```

Add this helper near `complete_oauth_login_internal`:

```rust
pub(crate) fn account_summary_from_token_response(
    token: &TokenResponse,
    fallback_id: &str,
    browser_profile_dir: Option<String>,
) -> AppResult<AccountSummary> {
    let auth_json = auth_json_from_token_response(token);
    let mut summary = summary_from_auth_json(&auth_json, None);
    summary.id = sanitize_id(
        &summary
            .account_id
            .clone()
            .or(summary.email.clone())
            .unwrap_or_else(|| fallback_id.to_string()),
    );
    summary.browser_profile_dir = browser_profile_dir;
    summary.imported_at = now_string();
    summary.has_config = false;
    Ok(summary)
}

pub(crate) fn save_token_response_as_account(
    token: &TokenResponse,
    fallback_id: &str,
    browser_profile_dir: Option<String>,
) -> AppResult<AccountSummary> {
    let summary = account_summary_from_token_response(token, fallback_id, browser_profile_dir)?;
    let auth_json = auth_json_from_token_response(token);
    let original = flat_oauth_json_from_auth_json(&auth_json, &summary);
    save_account_record(&summary, &auth_json, &original)?;
    Ok(summary)
}
```

Refactor `complete_oauth_login_internal` after token exchange to call the new helper:

```rust
    let token = exchange_code_for_tokens(&code, &pending.redirect_uri, &pending.code_verifier)?;
    save_token_response_as_account(
        &token,
        &pending.profile_id,
        Some(pending.browser_profile_dir.to_string_lossy().to_string()),
    )
```

- [ ] **Step 4: Run OAuth tests**

Run:

```bash
cd src-tauri
cargo test saves_token_response_as_account_summary generates_pkce_verifier_and_challenge validates_oauth_callback_query_values
```

Expected: PASS.

- [ ] **Step 5: Commit OAuth helper extraction**

```bash
git add src-tauri/src/oauth.rs
git commit -m "refactor: expose reusable Codex OAuth helpers"
```

## Task 3: AutoFlow Server Core And Session Tests

**Files:**
- Create: `src-tauri/src/autoflow_oauth_server.rs`
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: Add model DTOs**

In `src-tauri/src/models.rs`, append these structs:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowOAuthServerStatus {
    pub running: bool,
    pub port: u16,
    pub url: String,
    pub admin_key_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowGenerateAuthUrlResponse {
    pub auth_url: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowExchangeCodeResponse {
    pub message: String,
    pub id: String,
    pub email: Option<String>,
}
```

- [ ] **Step 2: Create server module with failing core tests**

Create `src-tauri/src/autoflow_oauth_server.rs` with the module header, test-only stubs, and tests first:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{AutoFlowExchangeCodeResponse, AutoFlowGenerateAuthUrlResponse};
use crate::oauth::{build_oauth_url, generate_pkce_codes};

const SESSION_TTL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone)]
struct AutoFlowOAuthSession {
    state: String,
    code_verifier: String,
    redirect_uri: String,
    expires_at: SystemTime,
    used: bool,
}

#[derive(Debug, Deserialize)]
struct ExchangeCodeRequest {
    session_id: String,
    code: String,
    state: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

#[derive(Debug, Clone)]
struct ApiResponse {
    status: u16,
    body: Value,
}

fn json_error(status: u16, message: impl Into<String>) -> ApiResponse {
    ApiResponse {
        status,
        body: json!(ErrorResponse {
            message: message.into()
        }),
    }
}

fn validate_admin_key(expected: &str, provided: Option<&str>) -> Result<(), ApiResponse> {
    let expected = expected.trim();
    if expected.is_empty() {
        return Err(json_error(401, "AutoFlow 管理密钥尚未配置。"));
    }
    match provided.map(str::trim) {
        Some(value) if value == expected => Ok(()),
        Some(_) => Err(json_error(401, "X-Admin-Key 无效。")),
        None => Err(json_error(401, "缺少 X-Admin-Key。")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_admin_key_rejects_missing_and_invalid_values() {
        assert_eq!(
            validate_admin_key("secret", None).unwrap_err().body["message"],
            "缺少 X-Admin-Key。"
        );
        assert_eq!(
            validate_admin_key("secret", Some("wrong")).unwrap_err().body["message"],
            "X-Admin-Key 无效。"
        );
        assert!(validate_admin_key("secret", Some("secret")).is_ok());
    }

    #[test]
    fn generate_auth_url_stores_session_with_state() {
        let mut sessions = HashMap::new();
        let response = generate_auth_url_for_sessions(
            &mut sessions,
            "http://localhost:1455/auth/callback",
            SystemTime::UNIX_EPOCH,
        )
        .expect("auth url should be generated");

        assert!(response.auth_url.contains("state="));
        assert!(response.auth_url.contains("code_challenge="));
        let session = sessions.get(&response.session_id).expect("session should exist");
        assert!(response.auth_url.contains(&session.state));
        assert_eq!(session.redirect_uri, "http://localhost:1455/auth/callback");
        assert!(!session.used);
    }

    #[test]
    fn exchange_rejects_state_mismatch() {
        let mut sessions = HashMap::new();
        sessions.insert(
            "sess_1".to_string(),
            AutoFlowOAuthSession {
                state: "state-1".to_string(),
                code_verifier: "verifier".to_string(),
                redirect_uri: "http://localhost:1455/auth/callback".to_string(),
                expires_at: SystemTime::UNIX_EPOCH + SESSION_TTL,
                used: false,
            },
        );

        let request = ExchangeCodeRequest {
            session_id: "sess_1".to_string(),
            code: "code".to_string(),
            state: "state-2".to_string(),
        };

        let response = exchange_code_for_sessions(
            &mut sessions,
            request,
            SystemTime::UNIX_EPOCH,
            |_code, _redirect_uri, _code_verifier| panic!("exchange must not run"),
            |_token| panic!("save must not run"),
        );

        assert_eq!(response.unwrap_err().body["message"], "OAuth state 不匹配。");
    }
}
```

- [ ] **Step 3: Run server core tests and verify they fail**

Run:

```bash
cd src-tauri
cargo test validate_admin_key_rejects_missing_and_invalid_values generate_auth_url_stores_session_with_state exchange_rejects_state_mismatch
```

Expected: FAIL because `generate_auth_url_for_sessions` and `exchange_code_for_sessions` are not defined.

- [ ] **Step 4: Implement pure session functions**

Add these functions above the test module in `src-tauri/src/autoflow_oauth_server.rs`:

```rust
fn generate_session_id() -> String {
    format!("sess_{}", Uuid::new_v4().simple())
}

fn generate_auth_url_for_sessions(
    sessions: &mut HashMap<String, AutoFlowOAuthSession>,
    redirect_uri: &str,
    now: SystemTime,
) -> AppResult<AutoFlowGenerateAuthUrlResponse> {
    let session_id = generate_session_id();
    let state = Uuid::new_v4().simple().to_string();
    let (code_verifier, code_challenge) = generate_pkce_codes();
    let auth_url = build_oauth_url(redirect_uri, &state, &code_challenge);
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

fn exchange_code_for_sessions<F, S>(
    sessions: &mut HashMap<String, AutoFlowOAuthSession>,
    request: ExchangeCodeRequest,
    now: SystemTime,
    exchange: F,
    save: S,
) -> Result<AutoFlowExchangeCodeResponse, ApiResponse>
where
    F: FnOnce(&str, &str, &str) -> AppResult<crate::models::TokenResponse>,
    S: FnOnce(&crate::models::TokenResponse) -> AppResult<crate::models::AccountSummary>,
{
    let session = sessions
        .get_mut(request.session_id.trim())
        .ok_or_else(|| json_error(400, "OAuth session 不存在。"))?;
    if session.used {
        return Err(json_error(400, "OAuth session 已使用。"));
    }
    if now > session.expires_at {
        return Err(json_error(400, "OAuth session 已过期。"));
    }
    if request.state.trim() != session.state {
        return Err(json_error(400, "OAuth state 不匹配。"));
    }
    if request.code.trim().is_empty() {
        return Err(json_error(400, "callback code 不能为空。"));
    }

    let token = exchange(
        request.code.trim(),
        &session.redirect_uri,
        &session.code_verifier,
    )
    .map_err(|err| json_error(400, err))?;
    let account = save(&token).map_err(|err| json_error(500, err))?;
    session.used = true;

    let label = account
        .email
        .clone()
        .unwrap_or_else(|| account.display_name.clone());
    Ok(AutoFlowExchangeCodeResponse {
        message: format!("OAuth 账号 {label} 添加成功"),
        id: account.id,
        email: account.email,
    })
}
```

If `TokenResponse` is still `pub(crate)` in `models.rs`, keep `autoflow_oauth_server.rs` in the same crate and use `crate::models::TokenResponse` as shown.

- [ ] **Step 5: Add missing exchange tests**

Add these tests to the same module:

```rust
#[test]
fn exchange_rejects_expired_and_used_sessions() {
    let mut sessions = HashMap::new();
    sessions.insert(
        "sess_expired".to_string(),
        AutoFlowOAuthSession {
            state: "state-1".to_string(),
            code_verifier: "verifier".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            expires_at: SystemTime::UNIX_EPOCH,
            used: false,
        },
    );
    sessions.insert(
        "sess_used".to_string(),
        AutoFlowOAuthSession {
            state: "state-2".to_string(),
            code_verifier: "verifier".to_string(),
            redirect_uri: "http://localhost:1455/auth/callback".to_string(),
            expires_at: SystemTime::UNIX_EPOCH + SESSION_TTL,
            used: true,
        },
    );

    let expired = exchange_code_for_sessions(
        &mut sessions,
        ExchangeCodeRequest {
            session_id: "sess_expired".to_string(),
            code: "code".to_string(),
            state: "state-1".to_string(),
        },
        SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        |_code, _redirect_uri, _code_verifier| panic!("exchange must not run"),
        |_token| panic!("save must not run"),
    )
    .unwrap_err();
    assert_eq!(expired.body["message"], "OAuth session 已过期。");

    let used = exchange_code_for_sessions(
        &mut sessions,
        ExchangeCodeRequest {
            session_id: "sess_used".to_string(),
            code: "code".to_string(),
            state: "state-2".to_string(),
        },
        SystemTime::UNIX_EPOCH,
        |_code, _redirect_uri, _code_verifier| panic!("exchange must not run"),
        |_token| panic!("save must not run"),
    )
    .unwrap_err();
    assert_eq!(used.body["message"], "OAuth session 已使用。");
}
```

- [ ] **Step 6: Run server core tests**

Run:

```bash
cd src-tauri
cargo test autoflow_oauth_server
```

Expected: PASS for all tests in `autoflow_oauth_server.rs`.

- [ ] **Step 7: Commit server core**

```bash
git add src-tauri/src/autoflow_oauth_server.rs src-tauri/src/models.rs
git commit -m "feat: add AutoFlow OAuth session core"
```

## Task 4: Local HTTP Listener And Tauri Commands

**Files:**
- Modify: `src-tauri/src/autoflow_oauth_server.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add command-level tests for key generation and status**

In `src-tauri/src/autoflow_oauth_server.rs`, add tests:

```rust
#[test]
fn generated_admin_key_is_url_safe_and_long() {
    let key = generate_admin_key();
    assert!(key.len() >= 43);
    assert!(key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'));
}

#[test]
fn service_url_uses_localhost_admin_accounts_path() {
    assert_eq!(
        service_url(8080),
        "http://127.0.0.1:8080/admin/accounts"
    );
}
```

- [ ] **Step 2: Run command tests and verify they fail**

Run:

```bash
cd src-tauri
cargo test generated_admin_key_is_url_safe_and_long service_url_uses_localhost_admin_accounts_path
```

Expected: FAIL because `generate_admin_key` and `service_url` are not defined.

- [ ] **Step 3: Implement server state, admin key generation, and status helpers**

In `src-tauri/src/autoflow_oauth_server.rs`, extend imports:

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::thread::{self, JoinHandle};

use crate::models::{AccountSummary, AutoFlowOAuthServerStatus, TokenResponse};
use crate::oauth::{exchange_code_for_tokens, save_token_response_as_account};
use crate::settings::{load_settings, sanitize_settings, save_settings, Settings};
```

Add state types:

```rust
struct RunningServer {
    port: u16,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

static SERVER: OnceLock<Mutex<Option<RunningServer>>> = OnceLock::new();
static SESSIONS: OnceLock<Mutex<HashMap<String, AutoFlowOAuthSession>>> = OnceLock::new();

fn server_state() -> &'static Mutex<Option<RunningServer>> {
    SERVER.get_or_init(|| Mutex::new(None))
}

fn sessions_state() -> &'static Mutex<HashMap<String, AutoFlowOAuthSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn service_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/admin/accounts")
}

fn generate_admin_key() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn ensure_admin_key(settings: &mut Settings) -> AppResult<()> {
    if settings.autoflow_oauth_admin_key.trim().is_empty() {
        settings.autoflow_oauth_admin_key = generate_admin_key();
        save_settings(settings)?;
    }
    Ok(())
}

fn current_status_from_settings(settings: &Settings) -> AutoFlowOAuthServerStatus {
    let running = server_state()
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false);
    AutoFlowOAuthServerStatus {
        running,
        port: settings.autoflow_oauth_server_port,
        url: service_url(settings.autoflow_oauth_server_port),
        admin_key_configured: !settings.autoflow_oauth_admin_key.trim().is_empty(),
    }
}
```

- [ ] **Step 4: Implement HTTP parser and response writer**

Add these helpers:

```rust
#[derive(Debug)]
struct ParsedRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: String,
}

fn parse_http_request(raw: &str) -> Option<ParsedRequest> {
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw, ""));
    let mut lines = head.lines();
    let request_line = lines.next()?;
    let mut pieces = request_line.split_whitespace();
    let method = pieces.next()?.to_string();
    let path = pieces.next()?.to_string();
    let headers = lines
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    Some(ParsedRequest {
        method,
        path,
        headers,
        body: body.to_string(),
    })
}

fn response_json(status: u16, body: &Value) -> Vec<u8> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let body_text = if status == 204 {
        String::new()
    } else {
        body.to_string()
    };
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, X-Admin-Key\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_text}",
        body_text.as_bytes().len()
    )
    .into_bytes()
}
```

- [ ] **Step 5: Implement API request dispatch**

Add:

```rust
fn handle_api_request(request: ParsedRequest, settings: &Settings) -> ApiResponse {
    if request.method.eq_ignore_ascii_case("OPTIONS") {
        return ApiResponse {
            status: 204,
            body: Value::Null,
        };
    }
    if !request.method.eq_ignore_ascii_case("POST") {
        return json_error(405, "只支持 POST 请求。");
    }

    let provided_key = request.headers.get("x-admin-key").map(String::as_str);
    if let Err(response) = validate_admin_key(&settings.autoflow_oauth_admin_key, provided_key) {
        return response;
    }

    match request.path.as_str() {
        "/api/admin/oauth/generate-auth-url" => {
            let redirect_uri = format!(
                "http://localhost:{}/auth/callback",
                settings.oauth_callback_port
            );
            let mut sessions = match sessions_state().lock() {
                Ok(sessions) => sessions,
                Err(err) => return json_error(500, err.to_string()),
            };
            match generate_auth_url_for_sessions(&mut sessions, &redirect_uri, SystemTime::now()) {
                Ok(result) => ApiResponse {
                    status: 200,
                    body: serde_json::to_value(result).unwrap_or_else(|_| json!({ "message": "响应序列化失败。" })),
                },
                Err(err) => json_error(500, err),
            }
        }
        "/api/admin/oauth/exchange-code" => {
            let parsed = match serde_json::from_str::<ExchangeCodeRequest>(&request.body) {
                Ok(parsed) => parsed,
                Err(_) => return json_error(400, "请求 JSON 无效。"),
            };
            let mut sessions = match sessions_state().lock() {
                Ok(sessions) => sessions,
                Err(err) => return json_error(500, err.to_string()),
            };
            match exchange_code_for_sessions(
                &mut sessions,
                parsed,
                SystemTime::now(),
                exchange_code_for_tokens,
                |token: &TokenResponse| {
                    save_token_response_as_account(token, &format!("autoflow-{}", Uuid::new_v4().simple()), None)
                },
            ) {
                Ok(result) => ApiResponse {
                    status: 200,
                    body: serde_json::to_value(result).unwrap_or_else(|_| json!({ "message": "响应序列化失败。" })),
                },
                Err(response) => response,
            }
        }
        _ => json_error(404, "接口不存在。"),
    }
}
```

- [ ] **Step 6: Implement Tauri commands and listener lifecycle**

Add:

```rust
#[tauri::command]
pub async fn start_autoflow_oauth_server() -> AppResult<AutoFlowOAuthServerStatus> {
    crate::error::run_blocking(start_autoflow_oauth_server_blocking).await
}

fn start_autoflow_oauth_server_blocking() -> AppResult<AutoFlowOAuthServerStatus> {
    let mut settings = sanitize_settings(load_settings()?)?;
    ensure_admin_key(&mut settings)?;

    let mut state = server_state().lock().map_err(|err| err.to_string())?;
    if state.is_some() {
        settings.autoflow_oauth_server_enabled = true;
        save_settings(&settings)?;
        return Ok(current_status_from_settings(&settings));
    }

    let port = settings.autoflow_oauth_server_port;
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|err| format!("AutoFlow 接入服务端口 {port} 无法监听：{err}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("AutoFlow 接入服务初始化失败：{err}"))?;

    let cancel = Arc::new(AtomicBool::new(false));
    let thread_cancel = cancel.clone();
    let handle = thread::spawn(move || {
        while !thread_cancel.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let mut buffer = [0_u8; 32768];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
                    let settings = match load_settings().and_then(sanitize_settings) {
                        Ok(settings) => settings,
                        Err(err) => {
                            let response = response_json(500, &json!({ "message": err }));
                            let _ = stream.write_all(&response);
                            continue;
                        }
                    };
                    let api_response = parse_http_request(&raw)
                        .map(|request| handle_api_request(request, &settings))
                        .unwrap_or_else(|| json_error(400, "HTTP 请求格式无效。"));
                    let response = response_json(api_response.status, &api_response.body);
                    let _ = stream.write_all(&response);
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => break,
            }
        }
    });

    settings.autoflow_oauth_server_enabled = true;
    save_settings(&settings)?;
    *state = Some(RunningServer {
        port,
        cancel,
        handle: Some(handle),
    });
    Ok(current_status_from_settings(&settings))
}

#[tauri::command]
pub async fn stop_autoflow_oauth_server() -> AppResult<AutoFlowOAuthServerStatus> {
    crate::error::run_blocking(stop_autoflow_oauth_server_blocking).await
}

fn stop_autoflow_oauth_server_blocking() -> AppResult<AutoFlowOAuthServerStatus> {
    let mut running = server_state().lock().map_err(|err| err.to_string())?.take();
    if let Some(mut server) = running.take() {
        server.cancel.store(true, Ordering::Relaxed);
        let _ = std::net::TcpStream::connect(("127.0.0.1", server.port));
        if let Some(handle) = server.handle.take() {
            let _ = handle.join();
        }
    }
    sessions_state()
        .lock()
        .map_err(|err| err.to_string())?
        .clear();
    let mut settings = sanitize_settings(load_settings()?)?;
    settings.autoflow_oauth_server_enabled = false;
    save_settings(&settings)?;
    Ok(current_status_from_settings(&settings))
}

#[tauri::command]
pub fn get_autoflow_oauth_server_status() -> AppResult<AutoFlowOAuthServerStatus> {
    let settings = sanitize_settings(load_settings()?)?;
    Ok(current_status_from_settings(&settings))
}

#[tauri::command]
pub fn reset_autoflow_oauth_admin_key() -> AppResult<Settings> {
    let mut settings = sanitize_settings(load_settings()?)?;
    settings.autoflow_oauth_admin_key = generate_admin_key();
    save_settings(&settings)?;
    Ok(settings)
}
```

- [ ] **Step 7: Register the module and commands**

In `src-tauri/src/lib.rs`, add:

```rust
mod autoflow_oauth_server;
```

Add these commands to `tauri::generate_handler![...]`:

```rust
            autoflow_oauth_server::start_autoflow_oauth_server,
            autoflow_oauth_server::stop_autoflow_oauth_server,
            autoflow_oauth_server::get_autoflow_oauth_server_status,
            autoflow_oauth_server::reset_autoflow_oauth_admin_key,
```

- [ ] **Step 8: Run Rust tests**

Run:

```bash
cd src-tauri
cargo test autoflow_oauth_server
```

Expected: PASS.

Run:

```bash
cd src-tauri
cargo test
```

Expected: PASS.

- [ ] **Step 9: Commit local server commands**

```bash
git add src-tauri/src/autoflow_oauth_server.rs src-tauri/src/settings.rs src-tauri/src/lib.rs src-tauri/src/models.rs
git commit -m "feat: add AutoFlow OAuth local server"
```

## Task 5: Frontend API Wiring

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api/codexSwitchApi.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Add TypeScript status type**

In `src/types.ts`, add:

```ts
export type AutoFlowOAuthServerStatus = {
  running: boolean;
  port: number;
  url: string;
  admin_key_configured: boolean;
};
```

- [ ] **Step 2: Add API wrappers**

In `src/api/codexSwitchApi.ts`, import the type:

```ts
  AutoFlowOAuthServerStatus,
```

Add wrappers:

```ts
export function getAutoFlowOAuthServerStatus() {
  return invoke<AutoFlowOAuthServerStatus>("get_autoflow_oauth_server_status");
}

export function startAutoFlowOAuthServer() {
  return invoke<AutoFlowOAuthServerStatus>("start_autoflow_oauth_server");
}

export function stopAutoFlowOAuthServer() {
  return invoke<AutoFlowOAuthServerStatus>("stop_autoflow_oauth_server");
}

export function resetAutoFlowOAuthAdminKey() {
  return invoke<Settings>("reset_autoflow_oauth_admin_key");
}
```

- [ ] **Step 3: Wire status and actions in App.vue**

Update the type import:

```ts
import type { AccountSummary, AppPaths, AutoFlowOAuthServerStatus, BackupSummary, CodexState, NetworkExitCheckResult, Settings } from "./types";
```

Add state:

```ts
const autoFlowServerStatus = ref<AutoFlowOAuthServerStatus | null>(null);
const autoFlowServerBusy = ref(false);
```

In `refreshAll()`, read the status in the Promise list:

```ts
  const [nextAccounts, nextBackups, nextCurrent, nextSettings, nextAutoFlowStatus] = await Promise.all([
    api.listAccounts(),
    api.listBackups(),
    api.readCurrentCodexState(),
    api.readSettings(),
    api.getAutoFlowOAuthServerStatus()
  ]);
```

Then assign:

```ts
  autoFlowServerStatus.value = nextAutoFlowStatus;
```

Add action functions:

```ts
async function startAutoFlowServer() {
  autoFlowServerBusy.value = true;
  try {
    const saved = await api.updateSettings({
      ...settings,
      autoflow_oauth_server_port: Number(settings.autoflow_oauth_server_port)
    });
    Object.assign(settings, saved);
    autoFlowServerStatus.value = await api.startAutoFlowOAuthServer();
    const refreshed = await api.readSettings();
    Object.assign(settings, refreshed);
    setMessage("success", "AutoFlow 接入服务已开启");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}

async function stopAutoFlowServer() {
  autoFlowServerBusy.value = true;
  try {
    autoFlowServerStatus.value = await api.stopAutoFlowOAuthServer();
    const refreshed = await api.readSettings();
    Object.assign(settings, refreshed);
    setMessage("success", "AutoFlow 接入服务已关闭");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}

async function resetAutoFlowAdminKey() {
  autoFlowServerBusy.value = true;
  try {
    const saved = await api.resetAutoFlowOAuthAdminKey();
    Object.assign(settings, saved);
    autoFlowServerStatus.value = await api.getAutoFlowOAuthServerStatus();
    setMessage("success", "AutoFlow 管理密钥已重置");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    autoFlowServerBusy.value = false;
  }
}
```

Add a copy helper using `navigator.clipboard`:

```ts
async function copyAutoFlowText(value: string, label: string) {
  try {
    await navigator.clipboard.writeText(value);
    setMessage("success", `${label}已复制`);
  } catch (err) {
    setMessage("error", `复制失败：${String(err)}`);
  }
}
```

Pass props/events to `SettingsView`:

```vue
        :autoflow-server-status="autoFlowServerStatus"
        :autoflow-server-busy="autoFlowServerBusy"
        @start-autoflow-server="startAutoFlowServer"
        @stop-autoflow-server="stopAutoFlowServer"
        @reset-autoflow-admin-key="resetAutoFlowAdminKey"
        @copy-autoflow-service-url="copyAutoFlowText(autoFlowServerStatus?.url || `http://127.0.0.1:${settings.autoflow_oauth_server_port}/admin/accounts`, 'AutoFlow 地址')"
        @copy-autoflow-admin-key="copyAutoFlowText(settings.autoflow_oauth_admin_key, '管理密钥')"
```

- [ ] **Step 4: Run frontend typecheck**

Run:

```bash
yarn build
```

Expected: FAIL until `SettingsView.vue` accepts the new props/events. This is the correct red step before Task 6.

- [ ] **Step 5: Commit API wiring after Task 6 passes**

Do not commit in this task until `SettingsView.vue` is updated and `yarn build` passes.

## Task 6: Settings UI Controls

**Files:**
- Modify: `src/views/SettingsView.vue`
- Modify: `src/styles/views.css`
- Modify: `src/App.vue`
- Modify: `src/api/codexSwitchApi.ts`
- Modify: `src/types.ts`

- [ ] **Step 1: Update SettingsView props and emits**

In `src/views/SettingsView.vue`, update the type import:

```ts
import type { AppPaths, AutoFlowOAuthServerStatus, NetworkExitCheckResult, Settings } from "../types";
```

Add props:

```ts
  autoflowServerStatus: AutoFlowOAuthServerStatus | null;
  autoflowServerBusy: boolean;
```

Add emits:

```ts
  startAutoflowServer: [];
  stopAutoflowServer: [];
  resetAutoflowAdminKey: [];
  copyAutoflowServiceUrl: [];
  copyAutoflowAdminKey: [];
```

Add helpers:

```ts
function maskedKey(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return "尚未生成";
  if (trimmed.length <= 12) return "••••••••";
  return `${trimmed.slice(0, 6)}••••••${trimmed.slice(-4)}`;
}
```

- [ ] **Step 2: Add the AutoFlow integration panel to the template**

Place this section after the OAuth login mode form group and before the network check panel:

```vue
    <section class="autoflow-integration-panel">
      <div class="panel-heading-row">
        <div>
          <span class="eyebrow">AutoFlow</span>
          <h3>自有软件 OAuth 接入服务</h3>
          <p>按需开启本地接口，让 AutoFlow 用 Codex2API 协议添加 OAuth 账号。</p>
        </div>
        <span class="service-state" :class="{ running: autoflowServerStatus?.running }">
          {{ autoflowServerStatus?.running ? "运行中" : "未开启" }}
        </span>
      </div>
      <label class="form-group">
        <span>接入服务端口</span>
        <input
          v-model.number="settings.autoflow_oauth_server_port"
          type="number"
          min="1024"
          max="65535"
          :disabled="autoflowServerStatus?.running || autoflowServerBusy"
        />
      </label>
      <div class="service-field-row">
        <span>AutoFlow 地址</span>
        <code>{{ autoflowServerStatus?.url || `http://127.0.0.1:${settings.autoflow_oauth_server_port}/admin/accounts` }}</code>
        <button class="secondary" type="button" :disabled="autoflowServerBusy" @click="$emit('copyAutoflowServiceUrl')">
          复制
        </button>
      </div>
      <div class="service-field-row">
        <span>管理密钥</span>
        <code>{{ maskedKey(settings.autoflow_oauth_admin_key) }}</code>
        <button
          class="secondary"
          type="button"
          :disabled="autoflowServerBusy || !settings.autoflow_oauth_admin_key"
          @click="$emit('copyAutoflowAdminKey')"
        >
          复制
        </button>
        <button class="secondary" type="button" :disabled="autoflowServerBusy" @click="$emit('resetAutoflowAdminKey')">
          重置
        </button>
      </div>
      <div class="service-actions">
        <button
          type="button"
          :disabled="busy || autoflowServerBusy || autoflowServerStatus?.running"
          @click="$emit('startAutoflowServer')"
        >
          {{ autoflowServerBusy ? "处理中" : "开启接入服务" }}
        </button>
        <button
          class="secondary"
          type="button"
          :disabled="busy || autoflowServerBusy || !autoflowServerStatus?.running"
          @click="$emit('stopAutoflowServer')"
        >
          关闭接入服务
        </button>
      </div>
    </section>
```

- [ ] **Step 3: Add styles**

In `src/styles/views.css`, add near the existing `.update-settings-panel` rules:

```css
.autoflow-integration-panel {
  display: grid;
  grid-column: 1 / -1;
  gap: 12px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: 13px;
  background: var(--bg-tertiary);
  animation: fade-in-up 260ms ease both;
}

.panel-heading-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.panel-heading-row h3 {
  margin-top: 4px;
  font-size: 15px;
}

.panel-heading-row p {
  margin-top: 6px;
  color: var(--text-secondary);
  line-height: 1.55;
}

.service-state {
  flex: 0 0 auto;
  border: 1px solid var(--warning-border);
  border-radius: 6px;
  padding: 5px 8px;
  color: var(--warning-text);
  background: var(--warning-bg);
  font-size: 12px;
  font-weight: 700;
}

.service-state.running {
  border-color: var(--success-badge-border);
  color: var(--success-badge-text);
  background: var(--success-badge-bg);
}

.service-field-row,
.service-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  min-width: 0;
}

.service-field-row > span {
  min-width: 84px;
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 700;
}

.service-field-row code {
  flex: 1 1 260px;
  min-width: 0;
  overflow-wrap: anywhere;
  border: 1px solid color-mix(in srgb, var(--border-color) 70%, transparent);
  border-radius: 7px;
  padding: 7px 9px;
  color: var(--text-primary);
  background: var(--bg-secondary);
  font-family: "JetBrains Mono", monospace;
  font-size: 12px;
}
```

- [ ] **Step 4: Run frontend build**

Run:

```bash
yarn build
```

Expected: PASS.

- [ ] **Step 5: Commit frontend UI**

```bash
git add src/types.ts src/api/codexSwitchApi.ts src/App.vue src/views/SettingsView.vue src/styles/views.css
git commit -m "feat: add AutoFlow OAuth service controls"
```

## Task 7: Manual Endpoint Verification

**Files:**
- No source edits expected.

- [ ] **Step 1: Run Rust and frontend verification**

Run:

```bash
cd src-tauri
cargo test
```

Expected: PASS.

Run from repo root:

```bash
yarn build
```

Expected: PASS.

- [ ] **Step 2: Start the Tauri dev app**

Run:

```bash
yarn tauri:dev
```

Expected: app starts and settings page loads. Keep the dev server running for the next steps.

- [ ] **Step 3: In the app, open Settings and start the service**

Actions:

1. Open the settings tab.
2. Confirm AutoFlow port is `8080` or set a free port.
3. Click `开启接入服务`.
4. Confirm status changes to `运行中`.
5. Click `复制` for the admin key and keep the value available for the next command.

Expected: no error toast, service URL displays `http://127.0.0.1:<port>/admin/accounts`.

- [ ] **Step 4: Verify missing key is rejected**

Run in another terminal, replacing `<port>`:

```bash
curl -i -X POST http://127.0.0.1:<port>/api/admin/oauth/generate-auth-url -H "Content-Type: application/json" -d "{}"
```

Expected:

```txt
HTTP/1.1 401 Unauthorized
{"message":"缺少 X-Admin-Key。"}
```

- [ ] **Step 5: Verify auth URL generation**

Run, replacing `<port>` and `<admin-key>`:

```bash
curl -s -X POST http://127.0.0.1:<port>/api/admin/oauth/generate-auth-url -H "Content-Type: application/json" -H "X-Admin-Key: <admin-key>" -d "{}"
```

Expected response contains both fields:

```json
{
  "auth_url": "https://auth.openai.com/oauth/authorize?...",
  "session_id": "sess_..."
}
```

Also verify the `auth_url` contains:

```txt
redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback
state=
code_challenge=
```

If the user configured a different OAuth callback port, the encoded `redirect_uri` must contain that port instead of `1455`.

- [ ] **Step 6: Verify bad exchange request errors clearly**

Run:

```bash
curl -s -X POST http://127.0.0.1:<port>/api/admin/oauth/exchange-code -H "Content-Type: application/json" -H "X-Admin-Key: <admin-key>" -d "{\"session_id\":\"missing\",\"code\":\"code\",\"state\":\"state\"}"
```

Expected:

```json
{"message":"OAuth session 不存在。"}
```

- [ ] **Step 7: Commit no-op verification marker only if source changed**

If verification required no source changes, do not commit.

If source changes were needed, run:

```bash
git add <changed-files>
git commit -m "fix: polish AutoFlow OAuth service verification"
```

## Task 8: Final Full Verification

**Files:**
- No source edits expected.

- [ ] **Step 1: Run full Rust tests**

```bash
cd src-tauri
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run frontend build**

```bash
yarn build
```

Expected: PASS.

- [ ] **Step 3: Run production Tauri compile check**

```bash
cd src-tauri
cargo build
```

Expected: PASS.

- [ ] **Step 4: Inspect git status**

```bash
git status --short --branch
```

Expected: branch is `codex/tauri-autoflow-oauth-server`; only intentional feature files are modified. Existing unrelated `logo.psd` may remain untracked and must not be staged.

## Self-Review Checklist

- Spec coverage: the plan covers user-started lifecycle, configurable port, generated persisted admin key, two Codex2API endpoints, session validation, state checks, token exchange, account save, UI controls, tests, and non-main branch work.
- Placeholder scan: no task contains `TBD`, `TODO`, "implement later", or an instruction to write unspecified tests.
- Type consistency: Rust uses `AutoFlowOAuthServerStatus`, `AutoFlowGenerateAuthUrlResponse`, and `AutoFlowExchangeCodeResponse`; TypeScript uses `AutoFlowOAuthServerStatus`; settings fields are consistently named `autoflow_oauth_server_enabled`, `autoflow_oauth_server_port`, and `autoflow_oauth_admin_key`.
