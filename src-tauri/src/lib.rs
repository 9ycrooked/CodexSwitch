use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use toml_edit::{DocumentMut, Item, Table};
use uuid::Uuid;

type AppResult<T> = Result<T, String>;

const OAUTH_AUTH_URL: &str = "https://auth.openai.com/oauth/authorize";
const OAUTH_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OAUTH_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api/codex";
const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const QUOTA_WINDOW_FIVE_HOURS: i64 = 18_000;
const QUOTA_WINDOW_WEEK: i64 = 604_800;

static OAUTH_PENDING: OnceLock<Mutex<Option<OAuthPending>>> = OnceLock::new();

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub expired: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub imported_at: String,
    #[serde(default)]
    pub has_config: bool,
    #[serde(default)]
    pub browser_profile_dir: Option<String>,
    #[serde(default)]
    pub oauth_metadata: Option<OAuthMetadata>,
    #[serde(default)]
    pub quota_state: Option<QuotaState>,
    #[serde(default)]
    pub usage_state: Option<UsageState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSummary {
    pub id: String,
    pub created_at: String,
    pub auth_path: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexState {
    pub codex_home: String,
    pub auth_exists: bool,
    pub config_exists: bool,
    pub current_account_id: Option<String>,
    pub current_email: Option<String>,
    pub current_auth_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchResult {
    pub account: AccountSummary,
    pub backup_id: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthMetadata {
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_type: Option<String>,
    pub subscription_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthLoginStart {
    pub auth_url: String,
    pub profile_id: String,
    pub browser_profile_dir: String,
    pub callback_port: u16,
    pub state: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaState {
    pub status: String,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub resets_at: Option<String>,
    pub resets_in_seconds: Option<i64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageState {
    pub status: String,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub http_status: Option<u16>,
    pub resets_at: Option<String>,
    pub raw_plan_type: Option<String>,
    #[serde(default)]
    pub windows: Vec<CodexQuotaWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexQuotaWindow {
    pub id: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub reset_at: Option<String>,
    pub reset_label: String,
    pub limit_reached: bool,
}

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

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    id_token: String,
    #[serde(default)]
    expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAccount {
    summary: AccountSummary,
    auth_json: Value,
    original_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BackupMeta {
    id: String,
    created_at: String,
    auth_path: Option<String>,
    config_path: Option<String>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            import_accounts,
            list_accounts,
            list_backups,
            switch_account,
            start_oauth_login,
            close_oauth_login,
            complete_oauth_login,
            refresh_account_tokens,
            check_account_quota,
            list_quota_states,
            fetch_codex_usage,
            list_usage_states,
            clear_usage_state,
            backup_current_state,
            restore_backup,
            read_current_codex_state,
            read_settings,
            update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn import_accounts(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
    if paths.is_empty() {
        return Err("请选择至少一个账号文件。".into());
    }

    let store = app_store_dir()?;
    let accounts_dir = store.join("accounts");
    fs::create_dir_all(&accounts_dir).map_err(stringify_io)?;

    let mut imported = Vec::new();
    let mut json_paths = Vec::new();
    let mut toml_paths = Vec::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        match path
            .extension()
            .and_then(|item| item.to_str())
            .map(str::to_ascii_lowercase)
        {
            Some(ext) if ext == "toml" => toml_paths.push(path),
            Some(ext) if ext == "json" => json_paths.push(path),
            _ => return Err(format!("不支持的文件类型：{}", path.display())),
        }
    }

    for path in json_paths {
        let matching_config = matching_config_path(&path, &toml_paths);
        let account = import_account_json(&path, matching_config.as_deref(), &accounts_dir)?;
        imported.push(account);
    }

    if imported.is_empty() {
        return Err("没有找到可导入的 JSON 账号文件。".into());
    }

    Ok(imported)
}

#[tauri::command]
fn list_accounts() -> AppResult<Vec<AccountSummary>> {
    let accounts_dir = app_store_dir()?.join("accounts");
    if !accounts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut accounts = Vec::new();
    for entry in fs::read_dir(accounts_dir).map_err(stringify_io)? {
        let entry = entry.map_err(stringify_io)?;
        if !entry.file_type().map_err(stringify_io)?.is_dir() {
            continue;
        }
        let meta_path = entry.path().join("metadata.json");
        if meta_path.exists() {
            let summary: AccountSummary = read_json(&meta_path)?;
            accounts.push(summary);
        }
    }
    accounts.sort_by(|a, b| b.imported_at.cmp(&a.imported_at));
    Ok(accounts)
}

#[tauri::command]
fn list_backups() -> AppResult<Vec<BackupSummary>> {
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
fn switch_account(account_id: String) -> AppResult<SwitchResult> {
    let settings = load_settings()?;
    let account = load_account(&account_id)?;
    let codex_home = PathBuf::from(&settings.codex_home);
    fs::create_dir_all(&codex_home).map_err(stringify_io)?;

    let mut warnings = account_warnings(&account.summary);
    close_codex_processes(&settings, &mut warnings);

    let backup = create_backup(&settings)?;
    let target_config_path = account_dir(&account_id)?.join("config.toml");
    let current_config_path = codex_home.join("config.toml");
    let merged_config = merge_config_files(&current_config_path, &target_config_path)?;

    let auth_path = codex_home.join("auth.json");
    let rollback_auth = fs::read_to_string(&auth_path).ok();
    let rollback_config = fs::read_to_string(&current_config_path).ok();

    if let Err(err) = atomic_write_json(&auth_path, &account.auth_json)
        .and_then(|_| atomic_write_text(&current_config_path, &merged_config))
    {
        if let Some(contents) = rollback_auth {
            let _ = atomic_write_text(&auth_path, &contents);
        }
        if let Some(contents) = rollback_config {
            let _ = atomic_write_text(&current_config_path, &contents);
        }
        return Err(format!("切换失败，已尝试回滚：{err}"));
    }

    Ok(SwitchResult {
        account: account.summary,
        backup_id: backup.id,
        warnings,
    })
}

#[tauri::command]
fn start_oauth_login(app: AppHandle, profile_id: Option<String>) -> AppResult<OAuthLoginStart> {
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
    let mode = sanitize_oauth_login_mode(&settings.oauth_login_mode);

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
fn close_oauth_login(app: AppHandle) -> AppResult<()> {
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
fn complete_oauth_login(callback_query: String) -> AppResult<AccountSummary> {
    complete_oauth_login_internal(&callback_query)
}

#[tauri::command]
fn refresh_account_tokens(account_id: String) -> AppResult<AccountSummary> {
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

#[tauri::command]
fn check_account_quota(account_id: String, model: Option<String>) -> AppResult<QuotaState> {
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
fn list_quota_states() -> AppResult<HashMap<String, QuotaState>> {
    let mut states = HashMap::new();
    for account in list_accounts()? {
        if let Some(state) = account.quota_state {
            states.insert(account.id, state);
        }
    }
    Ok(states)
}

#[tauri::command]
fn fetch_codex_usage(account_id: String) -> AppResult<UsageState> {
    let mut account = load_account(&account_id)?;
    let state = fetch_codex_usage_for_account(&account)?;
    account.summary.usage_state = Some(state.clone());
    account.summary.quota_state = Some(quota_state_from_usage_state(&state));
    save_account_record(&account.summary, &account.auth_json, &account.original_json)?;
    Ok(state)
}

#[tauri::command]
fn list_usage_states() -> AppResult<HashMap<String, UsageState>> {
    let mut states = HashMap::new();
    for account in list_accounts()? {
        if let Some(state) = account.usage_state {
            states.insert(account.id, state);
        }
    }
    Ok(states)
}

#[tauri::command]
fn clear_usage_state(account_id: String) -> AppResult<()> {
    let mut account = load_account(&account_id)?;
    account.summary.usage_state = None;
    save_account_record(&account.summary, &account.auth_json, &account.original_json)
}

#[tauri::command]
fn backup_current_state() -> AppResult<BackupSummary> {
    let settings = load_settings()?;
    create_backup(&settings)
}

#[tauri::command]
fn restore_backup(backup_id: String) -> AppResult<()> {
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

#[tauri::command]
fn read_current_codex_state() -> AppResult<CodexState> {
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

#[tauri::command]
fn read_settings() -> AppResult<Settings> {
    load_settings()
}

#[tauri::command]
fn update_settings(settings: Settings) -> AppResult<Settings> {
    if settings.codex_home.trim().is_empty() {
        return Err("Codex home 不能为空。".into());
    }
    if settings.process_names.is_empty() {
        return Err("至少需要一个 Codex 进程名。".into());
    }
    if settings.close_timeout_ms < 500 {
        return Err("关闭超时不能小于 500ms。".into());
    }

    let sanitized = Settings {
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
    };
    atomic_write_json(&settings_path()?, &sanitized)?;
    Ok(sanitized)
}

fn import_account_json(
    path: &Path,
    config_path: Option<&Path>,
    _accounts_dir: &Path,
) -> AppResult<AccountSummary> {
    let raw_text = fs::read_to_string(path).map_err(stringify_io)?;
    let raw_json: Value = serde_json::from_str(&raw_text)
        .map_err(|err| format!("JSON 解析失败 {}：{err}", path.display()))?;
    let (auth_json, mut summary) = normalize_auth_json(&raw_json)?;
    summary.imported_at = now_string();
    summary.has_config = config_path.is_some();

    let id_source = summary
        .account_id
        .clone()
        .or_else(|| summary.email.clone())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    summary.id = sanitize_id(&id_source);
    if summary.display_name.trim().is_empty() {
        summary.display_name = summary
            .email
            .clone()
            .or_else(|| summary.account_id.clone())
            .unwrap_or_else(|| "Codex account".to_string());
    }
    summary.oauth_metadata = oauth_metadata_from_auth_json(&auth_json);

    save_account_record(&summary, &auth_json, &raw_json)?;

    if let Some(config_path) = config_path {
        let text = fs::read_to_string(config_path).map_err(stringify_io)?;
        parse_toml(&text)?;
        atomic_write_text(&account_dir(&summary.id)?.join("config.toml"), &text)?;
    }

    Ok(summary)
}

fn normalize_auth_json(raw: &Value) -> AppResult<(Value, AccountSummary)> {
    if raw.get("auth_mode").is_some() && raw.get("tokens").is_some() {
        let tokens = raw
            .get("tokens")
            .and_then(Value::as_object)
            .ok_or_else(|| "auth.json 的 tokens 必须是对象。".to_string())?;
        for key in ["access_token", "id_token", "refresh_token", "account_id"] {
            if !tokens.contains_key(key) {
                return Err(format!("auth.json 缺少 tokens.{key}。"));
            }
        }
        let account_id = tokens
            .get("account_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let email = extract_email(raw).or_else(|| {
            tokens
                .get("id_token")
                .and_then(Value::as_str)
                .and_then(parse_jwt_claims)
                .and_then(|claims| extract_email(&claims))
        });
        let oauth_metadata = oauth_metadata_from_auth_json(raw);
        return Ok((
            raw.clone(),
            AccountSummary {
                id: String::new(),
                display_name: email
                    .clone()
                    .or_else(|| account_id.clone())
                    .unwrap_or_default(),
                email,
                account_id,
                plan: None,
                expired: None,
                disabled: false,
                imported_at: String::new(),
                has_config: false,
                browser_profile_dir: None,
                oauth_metadata,
                quota_state: None,
                usage_state: None,
            },
        ));
    }

    for key in ["access_token", "id_token", "refresh_token", "account_id"] {
        if !raw.get(key).is_some_and(|value| value.is_string()) {
            return Err(format!("OAuth JSON 缺少字符串字段：{key}。"));
        }
    }

    let mut tokens = Map::new();
    for key in ["access_token", "id_token", "refresh_token", "account_id"] {
        tokens.insert(key.to_string(), raw[key].clone());
    }

    let last_refresh = raw
        .get("last_refresh")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(now_string);

    let auth_json = json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": Value::Object(tokens),
        "last_refresh": last_refresh
    });

    let id_claims = raw
        .get("id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims);
    let email = raw
        .get("email")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| id_claims.as_ref().and_then(extract_email));
    let account_id = raw
        .get("account_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| id_claims.as_ref().and_then(extract_account_id));
    let plan = raw
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .or_else(|| {
            id_claims
                .as_ref()
                .and_then(|claims| claims.get("https://api.openai.com/auth"))
                .and_then(|auth| auth.get("chatgpt_plan_type"))
                .and_then(Value::as_str)
        })
        .or_else(|| raw.get("type").and_then(Value::as_str))
        .map(ToOwned::to_owned);
    let oauth_metadata = oauth_metadata_from_flat(raw)
        .or_else(|| id_claims.as_ref().and_then(oauth_metadata_from_flat));

    Ok((
        auth_json,
        AccountSummary {
            id: String::new(),
            display_name: email
                .clone()
                .or_else(|| account_id.clone())
                .unwrap_or_default(),
            email,
            account_id,
            plan,
            expired: raw
                .get("expired")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            disabled: raw
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            imported_at: String::new(),
            has_config: false,
            browser_profile_dir: None,
            oauth_metadata,
            quota_state: None,
            usage_state: None,
        },
    ))
}

fn summary_from_auth_json(auth_json: &Value, previous: Option<AccountSummary>) -> AccountSummary {
    let previous = previous.unwrap_or_else(|| AccountSummary {
        id: String::new(),
        display_name: String::new(),
        email: None,
        account_id: None,
        plan: None,
        expired: None,
        disabled: false,
        imported_at: now_string(),
        has_config: false,
        browser_profile_dir: None,
        oauth_metadata: None,
        quota_state: None,
        usage_state: None,
    });
    let claims = auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims);
    let email = claims
        .as_ref()
        .and_then(extract_email)
        .or(previous.email.clone());
    let account_id = auth_json
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| claims.as_ref().and_then(extract_account_id))
        .or(previous.account_id.clone());
    let oauth_metadata = oauth_metadata_from_auth_json(auth_json)
        .or_else(|| claims.as_ref().and_then(oauth_metadata_from_flat))
        .or(previous.oauth_metadata.clone());
    let plan = oauth_metadata
        .as_ref()
        .and_then(|meta| meta.plan_type.clone())
        .or(previous.plan.clone());
    let expired = previous
        .expired
        .or_else(|| {
            auth_json
                .get("expires_at")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            claims
                .as_ref()
                .and_then(|value| value.get("exp"))
                .and_then(Value::as_i64)
                .and_then(|exp| DateTime::<Utc>::from_timestamp(exp, 0))
                .map(|value| value.to_rfc3339())
        });
    let id = previous.id.if_empty(
        account_id
            .clone()
            .or(email.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
    );
    let display_name = email
        .clone()
        .or(account_id.clone())
        .unwrap_or_else(|| "Codex account".to_string());
    AccountSummary {
        id: sanitize_id(&id),
        display_name,
        email,
        account_id,
        plan,
        expired,
        disabled: false,
        imported_at: previous.imported_at.if_empty(now_string()),
        has_config: previous.has_config,
        browser_profile_dir: previous.browser_profile_dir,
        oauth_metadata,
        quota_state: previous.quota_state,
        usage_state: previous.usage_state,
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn save_account_record(
    summary: &AccountSummary,
    auth_json: &Value,
    original_json: &Value,
) -> AppResult<()> {
    let dir = account_dir(&summary.id)?;
    fs::create_dir_all(&dir).map_err(stringify_io)?;
    atomic_write_json(&dir.join("metadata.json"), summary)?;
    atomic_write_json(&dir.join("auth.json"), auth_json)?;
    atomic_write_json(&dir.join("original.json"), original_json)?;
    Ok(())
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
        Command::new(browser)
            .arg(format!("--user-data-dir={}", profile_dir.to_string_lossy()))
            .arg("--new-window")
            .arg(format!("--window-size={width},{height}"))
            .arg(auth_url)
            .spawn()
            .map_err(stringify_io)?;
        return Ok(());
    }
    Command::new("rundll32")
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

fn parse_token_http_response(
    response: reqwest::blocking::Response,
    label: &str,
) -> AppResult<TokenResponse> {
    let status = response.status();
    let body = response.text().map_err(|err| err.to_string())?;
    if !status.is_success() {
        return Err(format!("{label} 失败：HTTP {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|err| format!("{label} 响应解析失败：{err}"))
}

fn auth_json_from_token_response(token: &TokenResponse) -> Value {
    let claims = parse_jwt_claims(&token.id_token);
    let account_id = claims
        .as_ref()
        .and_then(extract_account_id)
        .unwrap_or_default();
    let expires_at = if token.expires_in > 0 {
        DateTime::<Utc>::from_timestamp(Utc::now().timestamp() + token.expires_in, 0)
            .map(|value| value.to_rfc3339())
    } else {
        None
    };
    json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "access_token": token.access_token,
            "id_token": token.id_token,
            "refresh_token": token.refresh_token,
            "account_id": account_id
        },
        "last_refresh": now_string(),
        "expires_at": expires_at
    })
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
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return quota_failure(&checked_at, model, err.to_string());
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
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(err) => return usage_failure(&checked_at, None, err.to_string(), None),
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
                usage_failure(&checked_at, Some(status), format!("HTTP {status}: {text}"), None)
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

fn pick_codex_windows(rate_info: &Value, allow_order_fallback: bool) -> (Option<&Value>, Option<&Value>) {
    let primary = field(rate_info, &["primary_window", "primaryWindow"]);
    let secondary = field(rate_info, &["secondary_window", "secondaryWindow"]);
    let mut five_hour = None;
    let mut weekly = None;
    for window in [primary, secondary].into_iter().flatten() {
        match number_field(window, &["limit_window_seconds", "limitWindowSeconds"]).map(|value| value as i64) {
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

fn create_backup(settings: &Settings) -> AppResult<BackupSummary> {
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

fn merge_config_files(current_path: &Path, account_path: &Path) -> AppResult<String> {
    let current_text = if current_path.exists() {
        fs::read_to_string(current_path).map_err(stringify_io)?
    } else {
        String::new()
    };
    let account_text = if account_path.exists() {
        fs::read_to_string(account_path).map_err(stringify_io)?
    } else {
        String::new()
    };
    merge_config_text(&current_text, &account_text)
}

fn merge_config_text(current_text: &str, account_text: &str) -> AppResult<String> {
    let mut current = if current_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        parse_toml(current_text)?
    };
    let account = if account_text.trim().is_empty() {
        DocumentMut::new()
    } else {
        parse_toml(account_text)?
    };

    merge_tables_current_first(current.as_table_mut(), account.as_table());
    Ok(current.to_string())
}

fn merge_tables_current_first(current: &mut Table, account: &Table) {
    let keys: BTreeSet<String> = account.iter().map(|(key, _)| key.to_string()).collect();
    for key in keys {
        let account_item = &account[&key];
        if !current.contains_key(&key) {
            current.insert(&key, account_item.clone());
            continue;
        }
        if let (Some(current_table), Some(account_table)) =
            (current[&key].as_table_mut(), account_item.as_table())
        {
            merge_tables_current_first(current_table, account_table);
        } else if let (Item::ArrayOfTables(current_tables), Item::ArrayOfTables(account_tables)) =
            (&mut current[&key], account_item)
        {
            if current_tables.is_empty() {
                *current_tables = account_tables.clone();
            }
        }
    }
}

fn close_codex_processes(settings: &Settings, warnings: &mut Vec<String>) {
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

fn account_warnings(summary: &AccountSummary) -> Vec<String> {
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

fn load_account(id: &str) -> AppResult<StoredAccount> {
    let dir = account_dir(id)?;
    if !dir.exists() {
        return Err(format!("账号不存在：{id}"));
    }
    Ok(StoredAccount {
        summary: read_json(&dir.join("metadata.json"))?,
        auth_json: read_json(&dir.join("auth.json"))?,
        original_json: read_json(&dir.join("original.json"))?,
    })
}

fn account_dir(id: &str) -> AppResult<PathBuf> {
    Ok(app_store_dir()?.join("accounts").join(sanitize_id(id)))
}

fn load_settings() -> AppResult<Settings> {
    let path = settings_path()?;
    if path.exists() {
        read_json(&path)
    } else {
        let settings = default_settings();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(stringify_io)?;
        }
        atomic_write_json(&path, &settings)?;
        Ok(settings)
    }
}

fn default_settings() -> Settings {
    Settings {
        codex_home: default_codex_home(),
        process_names: default_process_names(),
        close_timeout_ms: default_close_timeout_ms(),
        browser_profile_dir: default_browser_profile_dir(),
        oauth_callback_port: default_oauth_callback_port(),
        keep_login_profiles: default_keep_login_profiles(),
        oauth_login_mode: default_oauth_login_mode(),
    }
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

fn default_keep_login_profiles() -> bool {
    true
}

fn default_oauth_login_mode() -> String {
    "external".to_string()
}

fn sanitize_oauth_login_mode(value: &str) -> String {
    if value.trim().eq_ignore_ascii_case("embedded") {
        "embedded".to_string()
    } else {
        "external".to_string()
    }
}

fn settings_path() -> AppResult<PathBuf> {
    Ok(app_store_dir()?.join("settings.json"))
}

fn app_store_dir() -> AppResult<PathBuf> {
    let base = dirs::data_dir()
        .or_else(dirs::config_dir)
        .ok_or_else(|| "无法定位应用数据目录。".to_string())?;
    Ok(base.join("codex-account-switcher"))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    let text = fs::read_to_string(path).map_err(stringify_io)?;
    serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败 {}：{err}", path.display()))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    atomic_write_text(path, &format!("{text}\n"))
}

fn atomic_write_text(path: &Path, text: &str) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(stringify_io)?;
    }
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|item| item.to_str())
            .unwrap_or("file")
    ));
    {
        let mut file = fs::File::create(&temp).map_err(stringify_io)?;
        file.write_all(text.as_bytes()).map_err(stringify_io)?;
        file.sync_all().map_err(stringify_io)?;
    }
    match fs::rename(&temp, path) {
        Ok(_) => Ok(()),
        Err(first_err) if path.exists() => {
            fs::remove_file(path).map_err(|err| {
                let _ = fs::remove_file(&temp);
                format!(
                    "替换 {} 失败：{first_err}；删除旧文件也失败：{err}",
                    path.display()
                )
            })?;
            fs::rename(&temp, path).map_err(stringify_io)
        }
        Err(err) => Err(stringify_io(err)),
    }
}

fn parse_toml(text: &str) -> AppResult<DocumentMut> {
    text.parse::<DocumentMut>()
        .map_err(|err| format!("TOML 解析失败：{err}"))
}

fn extract_account_id(value: &Value) -> Option<String> {
    value
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .or_else(|| value.get("account_id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn extract_email(value: &Value) -> Option<String> {
    value
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("https://api.openai.com/profile")
                .and_then(|profile| profile.get("email"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

fn current_identity_from_auth(auth_json: &Value) -> OAuthMetadata {
    let claims = auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .or_else(|| auth_json.get("id_token").and_then(Value::as_str))
        .and_then(parse_jwt_claims);
    OAuthMetadata {
        email: extract_email(auth_json).or_else(|| claims.as_ref().and_then(extract_email)),
        account_id: extract_account_id(auth_json)
            .or_else(|| {
                claims
                    .as_ref()
                    .and_then(oauth_metadata_from_flat)
                    .and_then(|metadata| metadata.account_id)
            }),
        plan_type: claims
            .as_ref()
            .and_then(oauth_metadata_from_flat)
            .and_then(|metadata| metadata.plan_type),
        subscription_until: claims
            .as_ref()
            .and_then(oauth_metadata_from_flat)
            .and_then(|metadata| metadata.subscription_until),
    }
}

fn parse_jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn oauth_metadata_from_auth_json(auth_json: &Value) -> Option<OAuthMetadata> {
    auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims)
        .as_ref()
        .and_then(oauth_metadata_from_flat)
}

fn oauth_metadata_from_flat(value: &Value) -> Option<OAuthMetadata> {
    let auth = value.get("https://api.openai.com/auth");
    let profile = value.get("https://api.openai.com/profile");
    let metadata = OAuthMetadata {
        email: value
            .get("email")
            .and_then(Value::as_str)
            .or_else(|| {
                profile
                    .and_then(|profile| profile.get("email"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        account_id: value
            .get("account_id")
            .and_then(Value::as_str)
            .or_else(|| {
                auth.and_then(|auth| auth.get("chatgpt_account_id"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        plan_type: value
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| {
                auth.and_then(|auth| auth.get("chatgpt_plan_type"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        subscription_until: auth
            .and_then(|auth| auth.get("chatgpt_subscription_active_until"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };
    if metadata.email.is_none()
        && metadata.account_id.is_none()
        && metadata.plan_type.is_none()
        && metadata.subscription_until.is_none()
    {
        None
    } else {
        Some(metadata)
    }
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

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        Uuid::new_v4().to_string()
    } else {
        out
    }
}

fn matching_config_path(json_path: &Path, toml_paths: &[PathBuf]) -> Option<PathBuf> {
    if toml_paths.is_empty() {
        return None;
    }
    if toml_paths.len() == 1 {
        return Some(toml_paths[0].clone());
    }
    let json_stem = json_path
        .file_stem()?
        .to_string_lossy()
        .to_ascii_lowercase();
    toml_paths
        .iter()
        .find(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_ascii_lowercase() == json_stem)
                .unwrap_or(false)
        })
        .cloned()
}

fn stringify_io(err: std::io::Error) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_flat_oauth_json() {
        let raw = json!({
            "access_token": "access",
            "id_token": "id",
            "refresh_token": "refresh",
            "account_id": "acct-1",
            "email": "user@example.com",
            "expired": "2026-05-26T17:13:51+08:00",
            "disabled": false,
            "type": "codex"
        });

        let (auth, summary) = normalize_auth_json(&raw).unwrap();
        assert_eq!(auth["auth_mode"], "chatgpt");
        assert_eq!(auth["OPENAI_API_KEY"], Value::Null);
        assert_eq!(auth["tokens"]["account_id"], "acct-1");
        assert_eq!(summary.email.as_deref(), Some("user@example.com"));
        assert_eq!(summary.plan.as_deref(), Some("codex"));
    }

    #[test]
    fn preserves_wrapped_auth_json() {
        let raw = json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "access_token": "access",
                "id_token": "id",
                "refresh_token": "refresh",
                "account_id": "acct-2"
            },
            "last_refresh": "2026-05-17T00:00:00Z"
        });

        let (auth, summary) = normalize_auth_json(&raw).unwrap();
        assert_eq!(auth, raw);
        assert_eq!(summary.account_id.as_deref(), Some("acct-2"));
    }

    #[test]
    fn rejects_missing_tokens() {
        let raw = json!({
            "access_token": "access",
            "id_token": "id",
            "account_id": "acct-1"
        });
        assert!(normalize_auth_json(&raw).is_err());
    }

    #[test]
    fn merges_toml_current_first() {
        let current = r#"
model = "gpt-5.5"

[plugins."browser@openai-bundled"]
enabled = true

[mcp_servers.local]
command = "current"
"#;
        let account = r#"
model = "gpt-4.1"

[plugins."superpowers@openai-curated"]
enabled = true

[plugins."browser@openai-bundled"]
enabled = false

[mcp_servers.local]
command = "account"

[mcp_servers.extra]
command = "extra"
"#;

        let merged = merge_config_text(current, account).unwrap();
        let parsed = parse_toml(&merged).unwrap();
        assert_eq!(parsed["model"].as_str(), Some("gpt-5.5"));
        assert_eq!(
            parsed["plugins"]["browser@openai-bundled"]["enabled"].as_bool(),
            Some(true)
        );
        assert_eq!(
            parsed["plugins"]["superpowers@openai-curated"]["enabled"].as_bool(),
            Some(true)
        );
        assert_eq!(
            parsed["mcp_servers"]["local"]["command"].as_str(),
            Some("current")
        );
        assert_eq!(
            parsed["mcp_servers"]["extra"]["command"].as_str(),
            Some("extra")
        );
    }

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
    fn parses_jwt_oauth_metadata() {
        let claims = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-123",
                "chatgpt_plan_type": "plus",
                "chatgpt_subscription_active_until": "2026-06-16T09:12:24+00:00"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let parsed = parse_jwt_claims(&token).unwrap();
        let meta = oauth_metadata_from_flat(&parsed).unwrap();
        assert_eq!(meta.email.as_deref(), Some("user@example.com"));
        assert_eq!(meta.account_id.as_deref(), Some("acct-123"));
        assert_eq!(meta.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn parses_current_identity_from_wrapped_auth_id_token() {
        let claims = json!({
            "email": "current@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-from-claims",
                "chatgpt_plan_type": "plus"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access",
                "id_token": token,
                "refresh_token": "refresh",
                "account_id": "acct-from-token"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert_eq!(identity.email.as_deref(), Some("current@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("acct-from-token"));
        assert_eq!(identity.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn falls_back_to_claim_account_id_for_current_identity() {
        let claims = json!({
            "email": "fallback@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-from-claims"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access",
                "id_token": token,
                "refresh_token": "refresh"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert_eq!(identity.email.as_deref(), Some("fallback@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("acct-from-claims"));
    }

    #[test]
    fn ignores_invalid_current_identity_token() {
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "not-a-jwt"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert!(identity.email.is_none());
        assert!(identity.account_id.is_none());
    }

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
