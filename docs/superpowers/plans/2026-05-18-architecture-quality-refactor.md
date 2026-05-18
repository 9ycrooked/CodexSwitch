# Codex Switch Architecture Quality Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve Codex Switch code quality by splitting the current large frontend and backend files into clear modules without changing user-visible behavior.

**Architecture:** Use a stable-first refactor: extract types, pure helpers, API wrappers, and UI views in small commits while keeping the existing command names, JSON formats, settings file, account library, OAuth flow, quota behavior, updater behavior, and release workflow unchanged. The backend becomes domain-oriented Rust modules behind the same Tauri command surface; the frontend becomes typed API/composable/view/component layers behind the same Vue single app.

**Tech Stack:** Tauri 2, Rust 2021, Vue 3 `<script setup>`, TypeScript, Vite, Yarn 4, CSS.

---

## Scope And Guardrails

This plan is a refactor plan, not a feature plan.

Do not change:

- Tauri command names or payload shapes.
- App data directory names.
- Account metadata JSON layout.
- `auth.json` output format.
- `config.toml` merge rule: current local config wins, account config only fills missing keys.
- OAuth URLs, PKCE behavior, callback server behavior, or WebView/external browser behavior.
- Quota endpoint behavior.
- Updater endpoint behavior.
- UI visual style beyond small adjustments required by component extraction.

Keep every task independently buildable. If a task starts to require behavioral redesign, stop and split it into a new plan.

---

## Current Architecture Findings

- `src-tauri/src/lib.rs` is about 2422 lines and contains models, commands, file IO, settings, account import, config merge, backup, OAuth, quota, updater-adjacent settings, helpers, and tests.
- `src/App.vue` is about 1109 lines and contains app state, all Tauri API calls, all page views, update checking, window controls, formatting helpers, and event handlers.
- `src/styles.css` is about 1405 lines and contains global tokens, layout, all pages, all cards, dialogs, titlebar, and responsive rules.
- There are 18 Tauri commands and roughly 43 Vue functions in the main component.

---

## Target File Structure

### Rust Backend

- Keep: `src-tauri/src/main.rs`
  - Only starts the Tauri app and keeps the Windows release subsystem flag.
- Replace large role of: `src-tauri/src/lib.rs`
  - Owns `run()`, plugin registration, and command registration.
  - Re-exports modules only where needed.
- Create: `src-tauri/src/error.rs`
  - Defines `type AppResult<T> = Result<T, String>` and `stringify_io`.
- Create: `src-tauri/src/models.rs`
  - Public serializable structs shared by commands.
- Create: `src-tauri/src/paths.rs`
  - App store path, settings path, account path, backup path helpers.
- Create: `src-tauri/src/io.rs`
  - JSON reading, atomic JSON/text writes.
- Create: `src-tauri/src/settings.rs`
  - `Settings`, defaults, load/update validation.
- Create: `src-tauri/src/accounts.rs`
  - Account import, normalization, metadata extraction, account load/save/list.
- Create: `src-tauri/src/config_merge.rs`
  - TOML parse and current-first merge logic.
- Create: `src-tauri/src/backups.rs`
  - Backup creation, listing, restore.
- Create: `src-tauri/src/codex_home.rs`
  - Current Codex state reading and process closing.
- Create: `src-tauri/src/oauth.rs`
  - PKCE, OAuth URL, pending login state, callback server, token exchange/refresh.
- Create: `src-tauri/src/quota.rs`
  - Probe request, usage endpoint parsing, quota state mapping.
- Create: `src-tauri/src/commands.rs`
  - Thin Tauri command wrappers that call domain modules.

### Frontend

- Keep: `src/main.ts`
  - App bootstrap and CSS imports.
- Shrink: `src/App.vue`
  - Layout shell, navigation, top-level state composition, view selection.
- Create: `src/types.ts`
  - Shared frontend TypeScript types matching Tauri command payloads.
- Create: `src/api/codexSwitchApi.ts`
  - Typed wrappers around `invoke`.
- Create: `src/utils/format.ts`
  - Date, quota label, usage label, status class helpers.
- Create: `src/composables/useWindowControls.ts`
  - Minimize, maximize, close, drag, double-click behavior.
- Create: `src/composables/useUpdater.ts`
  - Remote update policy, Tauri updater check, install, dialog state.
- Create: `src/composables/useAccounts.ts`
  - Account list state, refresh, import, switch, token refresh.
- Create: `src/composables/useQuota.ts`
  - Selected quota account, fetch usage, clear usage.
- Create: `src/composables/useBackups.ts`
  - Backup list, create, restore.
- Create: `src/components/AppTitlebar.vue`
  - Custom titlebar.
- Create: `src/components/AppSidebar.vue`
  - Brand, navigation, current Codex state.
- Create: `src/components/UpdateDialog.vue`
  - Update release notes modal.
- Create: `src/views/AccountsView.vue`
  - Account library page.
- Create: `src/views/QuotaView.vue`
  - Quota monitoring page.
- Create: `src/views/BackupsView.vue`
  - Backup list page.
- Create: `src/views/SettingsView.vue`
  - Settings form and manual update check button.
- Keep initially: `src/styles.css`
  - Split only after component extraction is stable.

---

## Task 1: Establish Baseline Quality Checks

**Files:**
- Read: `src-tauri/src/lib.rs`
- Read: `src/App.vue`
- Read: `src/styles.css`
- Modify: none

- [ ] **Step 1: Verify clean working tree**

Run:

```powershell
git status --short
```

Expected:

```text
?? logo.psd
```

`logo.psd` is a local design source file and must not be included in refactor commits.

- [ ] **Step 2: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 3: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 4: Run Tauri no-bundle build**

Run:

```powershell
yarn tauri build --no-bundle
```

Expected:

```text
Built application at:
```

- [ ] **Step 5: Commit only if baseline files changed**

Do not commit if no files changed. If generated files changed unexpectedly, stop and inspect before continuing.

---

## Task 2: Extract Rust Error, IO, Paths, And Models

**Files:**
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/io.rs`
- Create: `src-tauri/src/paths.rs`
- Create: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/error.rs`**

Move the current `AppResult` alias and `stringify_io` helper into the new file:

```rust
pub type AppResult<T> = Result<T, String>;

pub fn stringify_io(err: std::io::Error) -> String {
    err.to_string()
}
```

- [ ] **Step 2: Create `src-tauri/src/io.rs`**

Move `read_json`, `atomic_write_json`, and `atomic_write_text` into the new file:

```rust
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::error::{stringify_io, AppResult};

pub fn read_json<T: DeserializeOwned>(path: &Path) -> AppResult<T> {
    let text = fs::read_to_string(path).map_err(stringify_io)?;
    serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败 {}：{err}", path.display()))
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let text = serde_json::to_string_pretty(value).map_err(|err| err.to_string())?;
    atomic_write_text(path, &format!("{text}\n"))
}

pub fn atomic_write_text(path: &Path, text: &str) -> AppResult<()> {
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
```

- [ ] **Step 3: Create `src-tauri/src/paths.rs`**

Move `app_store_dir`, `settings_path`, and `account_dir` into the new file:

```rust
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
    Ok(app_store_dir()?.join("accounts").join(crate::accounts::sanitize_id(id)))
}
```

- [ ] **Step 4: Create `src-tauri/src/models.rs`**

Move these existing structs unchanged into `models.rs`:

```rust
use serde::{Deserialize, Serialize};

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
```

Also move `TokenResponse`, `StoredAccount`, and `BackupMeta` as `pub(crate)` structs:

```rust
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    pub id_token: String,
    #[serde(default)]
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredAccount {
    pub summary: AccountSummary,
    pub auth_json: Value,
    pub original_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupMeta {
    pub id: String,
    pub created_at: String,
    pub auth_path: Option<String>,
    pub config_path: Option<String>,
}
```

- [ ] **Step 5: Wire modules in `src-tauri/src/lib.rs`**

At the top of `lib.rs`, add:

```rust
mod error;
mod io;
mod models;
mod paths;
```

Then import:

```rust
use error::AppResult;
use io::{atomic_write_json, atomic_write_text, read_json};
use models::*;
use paths::{account_dir, app_store_dir, settings_path};
```

Remove duplicate moved definitions from `lib.rs`.

- [ ] **Step 6: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/error.rs src-tauri/src/io.rs src-tauri/src/models.rs src-tauri/src/paths.rs
git commit -m "refactor: extract backend shared modules"
```

---

## Task 3: Extract Rust Settings Module

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/settings.rs`**

Move `Settings`, all settings default helpers, `load_settings`, `default_settings`, `read_settings`, and `update_settings` into `settings.rs`.

The file should expose:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::AppResult;
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
}

pub fn load_settings() -> AppResult<Settings> {
    let path = settings_path()?;
    if path.exists() {
        read_json(&path)
    } else {
        let settings = default_settings();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::stringify_io)?;
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
    }
}

#[tauri::command]
pub fn read_settings() -> AppResult<Settings> {
    load_settings()
}

#[tauri::command]
pub fn update_settings(settings: Settings) -> AppResult<Settings> {
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
        check_updates_on_startup: settings.check_updates_on_startup,
        force_update_on_startup: settings.force_update_on_startup,
    };
    atomic_write_json(&settings_path()?, &sanitized)?;
    Ok(sanitized)
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

fn default_keep_login_profiles() -> bool {
    true
}

fn default_oauth_login_mode() -> String {
    "external".to_string()
}
```

- [ ] **Step 2: Wire settings in `lib.rs`**

Add:

```rust
mod settings;
use settings::{load_settings, read_settings, update_settings, Settings};
```

Remove the moved settings definitions from `lib.rs`.

- [ ] **Step 3: Move settings test**

Move `settings_defaults_update_preferences_when_missing` from `lib.rs` tests into a `#[cfg(test)] mod tests` inside `settings.rs`.

Use this exact test:

```rust
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
}
```

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/settings.rs
git commit -m "refactor: extract backend settings module"
```

---

## Task 4: Extract Rust Account And Config Merge Modules

**Files:**
- Create: `src-tauri/src/accounts.rs`
- Create: `src-tauri/src/config_merge.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/config_merge.rs`**

Move these functions from `lib.rs` into `config_merge.rs`:

- `merge_config_files`
- `merge_config_text`
- `merge_item`
- `parse_toml`

The module should import:

```rust
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

use crate::error::{stringify_io, AppResult};
```

Expose:

```rust
pub fn merge_config_files(current_config_path: &Path, target_config_path: &Path) -> AppResult<String> {
    let current_text = fs::read_to_string(current_config_path).unwrap_or_default();
    let target_text = fs::read_to_string(target_config_path).unwrap_or_default();
    merge_config_text(&current_text, &target_text)
}

pub fn merge_config_text(current: &str, account: &str) -> AppResult<String> {
    let mut current_doc = parse_toml(current)?;
    let account_doc = parse_toml(account)?;
    merge_item(current_doc.as_item_mut(), account_doc.as_item());
    Ok(current_doc.to_string())
}
```

Keep the existing merge algorithm exactly as it is.

- [ ] **Step 2: Move config merge tests**

Move `merges_toml_current_first` into `config_merge.rs` under `#[cfg(test)]`.

- [ ] **Step 3: Create `src-tauri/src/accounts.rs`**

Move these functions from `lib.rs` into `accounts.rs`:

- `import_account_json`
- `normalize_auth_json`
- `summary_from_auth_json`
- `auth_json_from_token_response`
- `save_account_record`
- `load_account`
- `list_accounts`
- `matching_config_path`
- `extract_account_id`
- `extract_email`
- `parse_jwt_claims`
- `oauth_metadata_from_auth_json`
- `oauth_metadata_from_flat`
- `current_identity_from_auth`
- `sanitize_id`
- `now_string`

Expose only what other modules need:

```rust
pub fn list_accounts() -> AppResult<Vec<AccountSummary>>;
pub fn load_account(id: &str) -> AppResult<StoredAccount>;
pub fn import_account_json(path: &Path, config_path: Option<&Path>, accounts_dir: &Path) -> AppResult<AccountSummary>;
pub fn save_account_record(summary: &AccountSummary, auth_json: &Value, original_json: &Value) -> AppResult<()>;
pub fn sanitize_id(value: &str) -> String;
pub fn current_identity_from_auth(auth_json: &Value) -> OAuthMetadata;
pub fn auth_json_from_token_response(response: &TokenResponse) -> Value;
pub fn summary_from_auth_json(auth_json: &Value, previous: Option<AccountSummary>) -> AccountSummary;
```

- [ ] **Step 4: Wire modules in `lib.rs`**

Add:

```rust
mod accounts;
mod config_merge;
use accounts::{current_identity_from_auth, import_account_json, list_accounts, load_account, save_account_record};
use config_merge::merge_config_files;
```

If commands remain in `lib.rs` during this task, keep thin wrappers that call `accounts::list_accounts()` and `accounts::import_account_json()`.

- [ ] **Step 5: Move account tests**

Move these tests into `accounts.rs`:

- `normalizes_flat_oauth_json`
- `preserves_wrapped_auth_json`
- `rejects_missing_tokens`
- `parses_jwt_oauth_metadata`
- `parses_current_identity_from_wrapped_auth_id_token`
- `falls_back_to_claim_account_id_for_current_identity`
- `ignores_invalid_current_identity_token`

- [ ] **Step 6: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/accounts.rs src-tauri/src/config_merge.rs
git commit -m "refactor: extract account and config modules"
```

---

## Task 5: Extract Rust Backup, Codex Home, OAuth, And Quota Modules

**Files:**
- Create: `src-tauri/src/backups.rs`
- Create: `src-tauri/src/codex_home.rs`
- Create: `src-tauri/src/oauth.rs`
- Create: `src-tauri/src/quota.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `src-tauri/src/backups.rs`**

Move:

- `list_backups`
- `create_backup`
- `backup_current_state`
- `restore_backup`

Expose:

```rust
#[tauri::command]
pub fn list_backups() -> AppResult<Vec<BackupSummary>>;

#[tauri::command]
pub fn backup_current_state() -> AppResult<BackupSummary>;

#[tauri::command]
pub fn restore_backup(backup_id: String) -> AppResult<()>;

pub fn create_backup(settings: &Settings) -> AppResult<BackupSummary>;
```

- [ ] **Step 2: Create `src-tauri/src/codex_home.rs`**

Move:

- `read_current_codex_state`
- `close_codex_processes`
- `account_warnings`

Expose:

```rust
#[tauri::command]
pub fn read_current_codex_state() -> AppResult<CodexState>;

pub fn close_codex_processes(settings: &Settings, warnings: &mut Vec<String>);

pub fn account_warnings(account: &AccountSummary) -> Vec<String>;
```

- [ ] **Step 3: Create `src-tauri/src/oauth.rs`**

Move:

- OAuth constants
- `OAuthPending`
- `OAUTH_PENDING`
- `start_oauth_login`
- `close_oauth_login`
- `complete_oauth_login`
- `complete_oauth_login_internal`
- `cancel_pending_oauth_login`
- `bind_oauth_listener`
- `build_oauth_url`
- `generate_pkce_codes`
- `open_oauth_webview`
- `open_oauth_external`
- `exchange_code_for_tokens`
- `refresh_tokens`
- `refresh_account_tokens`
- `parse_query`
- `extract_query_from_request_line`

Expose Tauri commands:

```rust
#[tauri::command]
pub fn start_oauth_login(app: AppHandle, profile_id: Option<String>) -> AppResult<OAuthLoginStart>;

#[tauri::command]
pub fn close_oauth_login(app: AppHandle) -> AppResult<()>;

#[tauri::command]
pub fn complete_oauth_login(callback_query: String) -> AppResult<AccountSummary>;

#[tauri::command]
pub fn refresh_account_tokens(account_id: String) -> AppResult<AccountSummary>;
```

- [ ] **Step 4: Move OAuth tests**

Move into `oauth.rs`:

- `generates_pkce_verifier_and_challenge`
- `validates_oauth_callback_query_values`

- [ ] **Step 5: Create `src-tauri/src/quota.rs`**

Move:

- quota constants
- `check_account_quota`
- `list_quota_states`
- `fetch_codex_usage`
- `list_usage_states`
- `clear_usage_state`
- `probe_quota`
- `parse_quota_error`
- `fetch_codex_usage_for_account`
- `parse_codex_usage_payload`
- `build_codex_usage_windows`
- `quota_state_from_usage_state`
- `usage_state_from_quota_error`
- all usage parsing helper functions

Expose Tauri commands:

```rust
#[tauri::command]
pub fn check_account_quota(account_id: String, model: Option<String>) -> AppResult<QuotaState>;

#[tauri::command]
pub fn list_quota_states() -> AppResult<HashMap<String, QuotaState>>;

#[tauri::command]
pub fn fetch_codex_usage(account_id: String) -> AppResult<UsageState>;

#[tauri::command]
pub fn list_usage_states() -> AppResult<HashMap<String, UsageState>>;

#[tauri::command]
pub fn clear_usage_state(account_id: String) -> AppResult<()>;
```

- [ ] **Step 6: Move quota tests**

Move into `quota.rs`:

- `parses_quota_reset_metadata`
- `parses_codex_usage_windows`
- `parses_codex_usage_nested_body`
- `maps_usage_429_to_cooldown_state`

- [ ] **Step 7: Keep `switch_account` temporarily in `lib.rs`**

Do not move `switch_account` yet. It coordinates settings, accounts, process closing, backup, config merge, and atomic writes. Keeping it in `lib.rs` for one more task reduces risk.

- [ ] **Step 8: Wire modules in `lib.rs`**

Add:

```rust
mod backups;
mod codex_home;
mod oauth;
mod quota;
```

Update command registration to use module paths if necessary:

```rust
.invoke_handler(tauri::generate_handler![
    import_accounts,
    accounts::list_accounts,
    backups::list_backups,
    switch_account,
    oauth::start_oauth_login,
    oauth::close_oauth_login,
    oauth::complete_oauth_login,
    oauth::refresh_account_tokens,
    quota::check_account_quota,
    quota::list_quota_states,
    quota::fetch_codex_usage,
    quota::list_usage_states,
    quota::clear_usage_state,
    backups::backup_current_state,
    backups::restore_backup,
    codex_home::read_current_codex_state,
    settings::read_settings,
    settings::update_settings
])
```

- [ ] **Step 9: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 10: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/backups.rs src-tauri/src/codex_home.rs src-tauri/src/oauth.rs src-tauri/src/quota.rs
git commit -m "refactor: extract backend domain modules"
```

---

## Task 6: Create Thin Rust Commands Module

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Move `import_accounts` and `switch_account` into `commands.rs`**

Create `src-tauri/src/commands.rs` and move the remaining orchestration commands from `lib.rs`:

```rust
use std::fs;
use std::path::PathBuf;

use crate::accounts::{import_account_json, load_account};
use crate::backups::create_backup;
use crate::codex_home::{account_warnings, close_codex_processes};
use crate::config_merge::merge_config_files;
use crate::error::{stringify_io, AppResult};
use crate::io::{atomic_write_json, atomic_write_text};
use crate::models::{AccountSummary, SwitchResult};
use crate::paths::app_store_dir;
use crate::settings::load_settings;

#[tauri::command]
pub fn import_accounts(paths: Vec<String>) -> AppResult<Vec<AccountSummary>> {
    let store = crate::paths::app_store_dir()?;
    let accounts_dir = store.join("accounts");
    std::fs::create_dir_all(&accounts_dir).map_err(crate::error::stringify_io)?;
    let mut imported = Vec::new();
    let mut json_paths = Vec::new();
    let mut toml_paths = Vec::new();
    for raw_path in paths {
        let path = PathBuf::from(raw_path);
    match path.extension().and_then(|item| item.to_str()).map(str::to_ascii_lowercase) {
            Some(ext) if ext == "toml" => toml_paths.push(path),
            Some(ext) if ext == "json" => json_paths.push(path),
            _ => return Err(format!("不支持的文件类型：{}", path.display())),
        }
    }
    for path in json_paths {
        let matching_config = crate::accounts::matching_config_path(&path, &toml_paths);
        let account = crate::accounts::import_account_json(&path, matching_config.as_deref(), &accounts_dir)?;
        imported.push(account);
    }
    if imported.is_empty() {
        return Err("没有找到可导入的 JSON 账号文件。".into());
    }
    Ok(imported)
}

#[tauri::command]
pub fn switch_account(account_id: String) -> AppResult<SwitchResult> {
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
```

For `switch_account`, keep the shown behavior and update only names that moved into modules.

- [ ] **Step 2: Reduce `lib.rs` to app wiring**

After this task, `lib.rs` should contain:

```rust
mod accounts;
mod backups;
mod codex_home;
mod commands;
mod config_merge;
mod error;
mod io;
mod models;
mod oauth;
mod paths;
mod quota;
mod settings;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::import_accounts,
            accounts::list_accounts,
            backups::list_backups,
            commands::switch_account,
            oauth::start_oauth_login,
            oauth::close_oauth_login,
            oauth::complete_oauth_login,
            oauth::refresh_account_tokens,
            quota::check_account_quota,
            quota::list_quota_states,
            quota::fetch_codex_usage,
            quota::list_usage_states,
            quota::clear_usage_state,
            backups::backup_current_state,
            backups::restore_backup,
            codex_home::read_current_codex_state,
            settings::read_settings,
            settings::update_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 4: Run Tauri no-bundle build**

Run:

```powershell
yarn tauri build --no-bundle
```

Expected:

```text
Built application at:
```

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src
git commit -m "refactor: isolate backend command layer"
```

---

## Task 7: Extract Frontend Types, API, And Pure Formatting Helpers

**Files:**
- Create: `src/types.ts`
- Create: `src/api/codexSwitchApi.ts`
- Create: `src/utils/format.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Create `src/types.ts`**

Move all TypeScript type definitions from `App.vue` into `src/types.ts`:

```ts
export type AccountSummary = {
  id: string;
  display_name: string;
  email?: string | null;
  account_id?: string | null;
  plan?: string | null;
  expired?: string | null;
  disabled: boolean;
  imported_at: string;
  has_config: boolean;
  browser_profile_dir?: string | null;
  oauth_metadata?: OAuthMetadata | null;
  quota_state?: QuotaState | null;
  usage_state?: UsageState | null;
};

export type OAuthMetadata = {
  email?: string | null;
  account_id?: string | null;
  plan_type?: string | null;
  subscription_until?: string | null;
};

export type QuotaState = {
  status: string;
  last_checked_at?: string | null;
  last_error?: string | null;
  resets_at?: string | null;
  resets_in_seconds?: number | null;
  model?: string | null;
};

export type UsageState = {
  status: string;
  last_checked_at?: string | null;
  last_error?: string | null;
  http_status?: number | null;
  resets_at?: string | null;
  raw_plan_type?: string | null;
  windows: CodexQuotaWindow[];
};

export type CodexQuotaWindow = {
  id: string;
  label: string;
  used_percent?: number | null;
  reset_at?: string | null;
  reset_label: string;
  limit_reached: boolean;
};

export type BackupSummary = {
  id: string;
  created_at: string;
  auth_path?: string | null;
  config_path?: string | null;
};

export type Settings = {
  codex_home: string;
  process_names: string[];
  close_timeout_ms: number;
  browser_profile_dir: string;
  oauth_callback_port: number;
  keep_login_profiles: boolean;
  oauth_login_mode: string;
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
};

export type CodexState = {
  codex_home: string;
  auth_exists: boolean;
  config_exists: boolean;
  current_account_id?: string | null;
  current_email?: string | null;
  current_auth_mode?: string | null;
};

export type SwitchResult = {
  account: AccountSummary;
  backup_id: string;
  warnings: string[];
};

export type UpdatePolicy = {
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
  message?: string | null;
};
```

- [ ] **Step 2: Create `src/api/codexSwitchApi.ts`**

Create typed Tauri API wrappers:

```ts
import { invoke } from "@tauri-apps/api/core";
import type { AccountSummary, BackupSummary, CodexState, QuotaState, Settings, SwitchResult, UsageState } from "../types";

export function listAccounts() {
  return invoke<AccountSummary[]>("list_accounts");
}

export function listBackups() {
  return invoke<BackupSummary[]>("list_backups");
}

export function readCurrentCodexState() {
  return invoke<CodexState>("read_current_codex_state");
}

export function readSettings() {
  return invoke<Settings>("read_settings");
}

export function updateSettings(settings: Settings) {
  return invoke<Settings>("update_settings", { settings });
}

export function importAccounts(paths: string[]) {
  return invoke<AccountSummary[]>("import_accounts", { paths });
}

export function startOauthLogin(profileId: string | null = null) {
  return invoke<{ auth_url: string; browser_profile_dir: string; mode: string }>("start_oauth_login", { profileId });
}

export function closeOauthLogin() {
  return invoke("close_oauth_login");
}

export function refreshAccountTokens(accountId: string) {
  return invoke<AccountSummary>("refresh_account_tokens", { accountId });
}

export function checkAccountQuota(accountId: string, model = "gpt-5.5") {
  return invoke<QuotaState>("check_account_quota", { accountId, model });
}

export function fetchCodexUsage(accountId: string) {
  return invoke<UsageState>("fetch_codex_usage", { accountId });
}

export function clearUsageState(accountId: string) {
  return invoke("clear_usage_state", { accountId });
}

export function switchCodexAccount(accountId: string) {
  return invoke<SwitchResult>("switch_account", { accountId });
}

export function backupCurrentState() {
  return invoke<BackupSummary>("backup_current_state");
}

export function restoreBackup(backupId: string) {
  return invoke("restore_backup", { backupId });
}
```

- [ ] **Step 3: Create `src/utils/format.ts`**

Move pure formatting helpers from `App.vue` into this file:

```ts
import type { AccountSummary, CodexQuotaWindow, CodexState, QuotaState, UsageState } from "../types";

export function formatDate(value?: string | null) {
  if (!value) return "未知";
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return value;
  return date.toLocaleString();
}

export function quotaLabel(state?: QuotaState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    ok: "正常",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

export function quotaClass(state?: QuotaState | null) {
  if (!state) return "muted";
  if (state.status === "ok") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

export function quotaTimestamp(state?: QuotaState | null) {
  if (!state) return "未检查";
  if (state.resets_at) return formatDate(state.resets_at);
  if (state.last_checked_at) return formatDate(state.last_checked_at);
  return "无时间记录";
}

export function usageLabel(state?: UsageState | null) {
  if (!state) return "未检查";
  const labels: Record<string, string> = {
    success: "已更新",
    cooldown: "冷却中",
    token_invalid: "认证失效",
    check_failed: "检查失败"
  };
  return labels[state.status] || state.status;
}

export function usageClass(state?: UsageState | null) {
  if (!state) return "muted";
  if (state.status === "success") return "ok";
  if (state.status === "cooldown") return "warn";
  return "bad";
}

export function usageWindowWidth(window: CodexQuotaWindow) {
  const value = Math.max(0, Math.min(100, Number(window.used_percent ?? 0)));
  return `${value}%`;
}

export function usageWindowClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "quota-bar-fill quota-bar-fill-high";
  if (window.limit_reached || value >= 100) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 90) return "quota-bar-fill quota-bar-fill-low";
  if (value >= 70) return "quota-bar-fill quota-bar-fill-medium";
  if (value > 0) return "quota-bar-fill quota-bar-fill-high";
  return "quota-bar-fill quota-bar-fill-muted";
}

export function usageWindowPercentClass(window: CodexQuotaWindow) {
  const value = Number(window.used_percent ?? 0);
  const isWeeklyWindow = window.id.includes("weekly") || window.label.includes("周");
  if (isWeeklyWindow && value < 100) return "ok";
  return window.limit_reached || value >= 100 ? "bad" : "ok";
}

export function usagePercentLabel(window: CodexQuotaWindow) {
  if (window.used_percent === null || window.used_percent === undefined) return "未知";
  return `已用 ${Math.round(Number(window.used_percent))}%`;
}

export function usageResetLabel(window: CodexQuotaWindow) {
  return window.reset_at ? formatDate(window.reset_at) : window.reset_label || "-";
}

export function isCurrentAccount(account: AccountSummary, current?: CodexState | null) {
  return Boolean(account.account_id && current?.current_account_id === account.account_id);
}

export function accountStatusLabel(account: AccountSummary, current?: CodexState | null) {
  if (isCurrentAccount(account, current)) return "当前";
  if (account.disabled) return "禁用";
  if (account.quota_state?.status === "cooldown") return "冷却";
  if (account.quota_state?.status === "token_invalid") return "失效";
  if (account.quota_state?.status === "check_failed") return "警告";
  return "可用";
}

export function accountStatusClass(account: AccountSummary, current?: CodexState | null) {
  if (isCurrentAccount(account, current)) return "state-badge-active";
  if (account.disabled) return "state-badge-disabled";
  if (account.quota_state?.status === "cooldown") return "state-badge-warning";
  if (account.quota_state?.status === "token_invalid" || account.quota_state?.status === "check_failed") {
    return "state-badge-disabled";
  }
  return "state-badge-active";
}

export function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
```

- [ ] **Step 4: Update `App.vue` imports**

Remove local type definitions and pure helper functions from `App.vue`.

Add:

```ts
import type { AccountSummary, BackupSummary, CodexState, Settings, UpdatePolicy } from "./types";
import * as api from "./api/codexSwitchApi";
import {
  accountStatusClass,
  accountStatusLabel,
  formatDate,
  formatError,
  isCurrentAccount,
  quotaClass,
  quotaLabel,
  quotaTimestamp,
  usageClass,
  usageLabel,
  usagePercentLabel,
  usageResetLabel,
  usageWindowClass,
  usageWindowPercentClass,
  usageWindowWidth
} from "./utils/format";
```

Update template calls that currently use `isCurrentAccount(account)` to `isCurrentAccount(account, current)`.

Update `accountStatusClass(account)` and `accountStatusLabel(account)` calls to pass `current`.

- [ ] **Step 5: Replace direct invokes**

Replace direct `invoke` calls with `api` functions:

```ts
const [nextAccounts, nextBackups, nextCurrent, nextSettings] = await Promise.all([
  api.listAccounts(),
  api.listBackups(),
  api.readCurrentCodexState(),
  api.readSettings()
]);
```

Apply equivalent replacements for import, OAuth, token refresh, quota, backup, restore, switch, and settings.

- [ ] **Step 6: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 7: Commit**

```powershell
git add src/App.vue src/types.ts src/api/codexSwitchApi.ts src/utils/format.ts
git commit -m "refactor: extract frontend types api and formatters"
```

---

## Task 8: Extract Frontend Composables

**Files:**
- Create: `src/composables/useWindowControls.ts`
- Create: `src/composables/useUpdater.ts`
- Create: `src/composables/useAccounts.ts`
- Create: `src/composables/useQuota.ts`
- Create: `src/composables/useBackups.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Create `src/composables/useWindowControls.ts`**

Move window controls:

```ts
import { getCurrentWindow } from "@tauri-apps/api/window";

export function useWindowControls() {
  const appWindow = getCurrentWindow();

  async function minimizeWindow() {
    await appWindow.minimize();
  }

  async function toggleMaximizeWindow() {
    await appWindow.toggleMaximize();
  }

  async function closeWindow() {
    await appWindow.close();
  }

  async function startWindowDrag(event: MouseEvent) {
    if (event.button !== 0) return;
    await appWindow.startDragging();
  }

  async function handleTitlebarDoubleClick(event: MouseEvent) {
    if (event.button !== 0) return;
    await appWindow.toggleMaximize();
  }

  return {
    minimizeWindow,
    toggleMaximizeWindow,
    closeWindow,
    startWindowDrag,
    handleTitlebarDoubleClick
  };
}
```

- [ ] **Step 2: Create `src/composables/useUpdater.ts`**

Move update policy and updater functions from `App.vue`:

```ts
import { computed, reactive, ref } from "vue";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import type { Settings, UpdatePolicy } from "../types";
import { formatError } from "../utils/format";

const UPDATE_POLICY_URL = "https://github.com/9ycrooked/CodexSwitch/releases/latest/download/update-policy.json";

type UpdateInfo = Update & {
  body?: string;
  notes?: string;
  version?: string;
  currentVersion?: string;
};

export function useUpdater(settings: Settings, setMessage: (message: string, isError?: boolean) => void) {
  const updatePolicy = reactive<UpdatePolicy>({
    check_updates_on_startup: true,
    force_update_on_startup: false,
    message: null
  });
  const updatePolicySource = ref("默认策略");
  const updatePolicyError = ref("");
  const updateDialogOpen = ref(false);
  const updateChecking = ref(false);
  const updateDownloading = ref(false);
  const updateError = ref("");
  const pendingUpdate = ref<Update | null>(null);
  const updateDownloadedBytes = ref(0);
  const updateTotalBytes = ref(0);

  const pendingUpdateInfo = computed(() => pendingUpdate.value as UpdateInfo | null);
  const pendingUpdateNotes = computed(() => pendingUpdateInfo.value?.body || pendingUpdateInfo.value?.notes || "这个版本没有填写更新说明。");
  const updateProgressPercent = computed(() => {
    if (!updateTotalBytes.value) return 0;
    return Math.min(100, Math.round((updateDownloadedBytes.value / updateTotalBytes.value) * 100));
  });
  const updateIsForced = computed(() => Boolean(updatePolicy.force_update_on_startup && pendingUpdate.value));

  function toBoolean(value: unknown, fallback: boolean) {
    return typeof value === "boolean" ? value : fallback;
  }

  async function loadUpdatePolicy(): Promise<UpdatePolicy> {
    const fallback: UpdatePolicy = {
      check_updates_on_startup: settings.check_updates_on_startup ?? true,
      force_update_on_startup: settings.force_update_on_startup ?? false,
      message: null
    };

    try {
      const response = await fetch(UPDATE_POLICY_URL, { cache: "no-store" });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const remote = (await response.json()) as Partial<UpdatePolicy>;
      const nextPolicy = {
        check_updates_on_startup: toBoolean(remote.check_updates_on_startup, fallback.check_updates_on_startup),
        force_update_on_startup: toBoolean(remote.force_update_on_startup, fallback.force_update_on_startup),
        message: typeof remote.message === "string" ? remote.message : null
      };
      Object.assign(updatePolicy, nextPolicy);
      updatePolicySource.value = "远程发布配置";
      updatePolicyError.value = "";
      return nextPolicy;
    } catch (err) {
      Object.assign(updatePolicy, fallback);
      updatePolicySource.value = "默认策略";
      updatePolicyError.value = `发布配置读取失败，已使用默认策略：${formatError(err)}`;
      return fallback;
    }
  }

  async function runUpdateCheck(options: { manual?: boolean } = {}) {
    const manual = Boolean(options.manual);
    const policy = await loadUpdatePolicy();
    if (!manual && !policy.check_updates_on_startup) return;

    updateChecking.value = true;
    updateError.value = "";
    try {
      const update = await check();
      if (!update) {
        if (manual) setMessage("当前已经是最新版本。");
        return;
      }
      pendingUpdate.value = update;
      updateDialogOpen.value = true;
      if (manual) setMessage("");
    } catch (err) {
      const message = `更新检查失败：${formatError(err)}`;
      updateError.value = message;
      if (manual) setMessage(message, true);
    } finally {
      updateChecking.value = false;
    }
  }

  async function installPendingUpdate() {
    if (!pendingUpdate.value) return;
    updateDownloading.value = true;
    updateError.value = "";
    updateDownloadedBytes.value = 0;
    updateTotalBytes.value = 0;
    try {
      await pendingUpdate.value.downloadAndInstall((event) => {
        if (event.event === "Started") updateTotalBytes.value = event.data.contentLength ?? 0;
        if (event.event === "Progress") updateDownloadedBytes.value += event.data.chunkLength;
      });
      await relaunch();
    } catch (err) {
      updateError.value = `更新安装失败：${formatError(err)}`;
    } finally {
      updateDownloading.value = false;
    }
  }

  function dismissUpdateDialog() {
    if (updateIsForced.value) return;
    updateDialogOpen.value = false;
  }

  return {
    updatePolicy,
    updatePolicySource,
    updatePolicyError,
    updateDialogOpen,
    updateChecking,
    updateDownloading,
    updateError,
    pendingUpdate,
    pendingUpdateInfo,
    pendingUpdateNotes,
    updateDownloadedBytes,
    updateTotalBytes,
    updateProgressPercent,
    updateIsForced,
    runUpdateCheck,
    checkForUpdatesManually: () => runUpdateCheck({ manual: true }),
    installPendingUpdate,
    dismissUpdateDialog
  };
}
```

- [ ] **Step 3: Create account, quota, and backup composables**

Keep these composables thin. They should receive shared refs from `App.vue` and call the API wrappers.

For `src/composables/useAccounts.ts`, expose:

```ts
export function useAccounts(deps: {
  accounts: Ref<AccountSummary[]>;
  current: Ref<CodexState | null>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
}) {
  // move chooseAndImport, startOAuthLogin, closeOAuthLogin, refreshTokens, switchAccount
}
```

For `src/composables/useQuota.ts`, expose:

```ts
export function useQuota(deps: {
  accounts: Ref<AccountSummary[]>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
}) {
  // move selectedQuotaAccountId, selectedQuotaAccount, selectedUsageState, selectQuotaAccount, fetchUsage, clearUsage
}
```

For `src/composables/useBackups.ts`, expose:

```ts
export function useBackups(deps: {
  backups: Ref<BackupSummary[]>;
  busy: Ref<boolean>;
  refreshAll: () => Promise<void>;
  setMessage: (message: string, isError?: boolean) => void;
}) {
  // move createBackup and restoreBackup
}
```

- [ ] **Step 4: Update `App.vue` to use composables**

Import the composables and destructure their returned functions.

Keep `refreshAll`, `saveSettings`, `setMessage`, `notice`, `error`, `busy`, and `selectedTab` in `App.vue`.

- [ ] **Step 5: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 6: Commit**

```powershell
git add src/App.vue src/composables
git commit -m "refactor: extract frontend composables"
```

---

## Task 9: Extract Frontend Components And Views

**Files:**
- Create: `src/components/AppTitlebar.vue`
- Create: `src/components/AppSidebar.vue`
- Create: `src/components/UpdateDialog.vue`
- Create: `src/views/AccountsView.vue`
- Create: `src/views/QuotaView.vue`
- Create: `src/views/BackupsView.vue`
- Create: `src/views/SettingsView.vue`
- Modify: `src/App.vue`

- [ ] **Step 1: Extract `AppTitlebar.vue`**

Move the titlebar template into `src/components/AppTitlebar.vue`.

Props:

```ts
defineProps<{
  onMinimize: () => void;
  onToggleMaximize: () => void;
  onClose: () => void;
  onStartDrag: (event: MouseEvent) => void;
  onDoubleClick: (event: MouseEvent) => void;
}>();
```

Use the existing `Minus`, `Square`, `X` lucide icons inside the component.

- [ ] **Step 2: Extract `AppSidebar.vue`**

Props:

```ts
defineProps<{
  selectedTab: "accounts" | "quota" | "backups" | "settings";
  current: CodexState | null;
}>();
```

Emits:

```ts
defineEmits<{
  select: ["accounts" | "quota" | "backups" | "settings"];
}>();
```

- [ ] **Step 3: Extract `UpdateDialog.vue`**

Props:

```ts
defineProps<{
  open: boolean;
  forced: boolean;
  downloading: boolean;
  error: string;
  policyMessage?: string | null;
  currentVersion?: string;
  nextVersion?: string;
  notes: string;
  progressPercent: number;
  hasTotalBytes: boolean;
}>();
```

Emits:

```ts
defineEmits<{
  dismiss: [];
  install: [];
  closeApp: [];
}>();
```

- [ ] **Step 4: Extract `AccountsView.vue`**

Props:

```ts
defineProps<{
  accounts: AccountSummary[];
  filteredAccounts: AccountSummary[];
  current: CodexState | null;
  busy: boolean;
  query: string;
}>();
```

Emits:

```ts
defineEmits<{
  "update:query": [string];
  switchAccount: [AccountSummary];
  refreshTokens: [AccountSummary];
  selectQuotaAccount: [AccountSummary];
}>();
```

- [ ] **Step 5: Extract `QuotaView.vue`**

Props:

```ts
defineProps<{
  accounts: AccountSummary[];
  selectedQuotaAccountId: string;
  selectedQuotaAccount: AccountSummary | null;
  busy: boolean;
}>();
```

Emits:

```ts
defineEmits<{
  "update:selectedQuotaAccountId": [string];
  refreshAll: [];
  fetchUsage: [AccountSummary | null];
  refreshTokens: [AccountSummary];
  clearUsage: [AccountSummary];
}>();
```

- [ ] **Step 6: Extract `BackupsView.vue`**

Props:

```ts
defineProps<{
  backups: BackupSummary[];
  busy: boolean;
}>();
```

Emits:

```ts
defineEmits<{
  restoreBackup: [BackupSummary];
}>();
```

- [ ] **Step 7: Extract `SettingsView.vue`**

Props:

```ts
defineProps<{
  settings: Settings;
  busy: boolean;
  updateChecking: boolean;
  updateDownloading: boolean;
  updatePolicySource: string;
  updatePolicyError: string;
}>();
```

Emits:

```ts
defineEmits<{
  updateProcessNames: [Event];
  checkForUpdates: [];
  saveSettings: [];
}>();
```

- [ ] **Step 8: Reduce `App.vue` template**

After extraction, `App.vue` should contain only:

- `<AppTitlebar />`
- `<AppSidebar />`
- topbar
- notices
- selected view component
- `<UpdateDialog />`

- [ ] **Step 9: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 10: Commit**

```powershell
git add src/App.vue src/components src/views
git commit -m "refactor: extract frontend views and components"
```

---

## Task 10: Split CSS Into Themed Sections

**Files:**
- Create: `src/styles/tokens.css`
- Create: `src/styles/base.css`
- Create: `src/styles/layout.css`
- Create: `src/styles/components.css`
- Create: `src/styles/views.css`
- Modify: `src/styles.css`

- [ ] **Step 1: Create CSS section files**

Split `src/styles.css` by responsibility:

- `tokens.css`: `:root` variables and typography base variables.
- `base.css`: reset, body, input, select, button basics.
- `layout.css`: shell, titlebar, sidebar, content, topbar, responsive layout.
- `components.css`: notices, panels, badges, cards, modal, buttons, form groups.
- `views.css`: account page, quota page, backup page, settings page specific rules.

- [ ] **Step 2: Replace `src/styles.css` with imports**

Make `src/styles.css`:

```css
@import "./styles/tokens.css";
@import "./styles/base.css";
@import "./styles/layout.css";
@import "./styles/components.css";
@import "./styles/views.css";
```

- [ ] **Step 3: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 4: Commit**

```powershell
git add src/styles.css src/styles
git commit -m "refactor: split stylesheet modules"
```

---

## Task 11: Final Verification And Quality Pass

**Files:**
- Read all changed files.
- Modify only if verification reveals issues.

- [ ] **Step 1: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 2: Run frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 3: Run Tauri no-bundle build**

Run:

```powershell
yarn tauri build --no-bundle
```

Expected:

```text
Built application at:
```

- [ ] **Step 4: Check file sizes**

Run:

```powershell
Get-ChildItem -Recurse -File src,src-tauri\src | Select-Object FullName,Length
```

Expected:

- `src/App.vue` is substantially smaller than before.
- `src-tauri/src/lib.rs` is substantially smaller than before.
- No new module has grown beyond roughly 700 lines unless justified by tests or parsing complexity.

- [ ] **Step 5: Manual smoke test**

Run:

```powershell
yarn tauri dev
```

Verify:

- App opens without blank screen.
- Sidebar navigation works.
- Account list renders.
- Quota page renders.
- Settings page renders.
- Manual update check button is visible.
- OAuth login button still calls the existing backend command.
- Custom titlebar drag/minimize/maximize/close still works.

- [ ] **Step 6: Commit fixes if needed**

If Step 1-5 reveal issues, fix them and commit:

```powershell
git add src src-tauri
git commit -m "fix: stabilize architecture refactor"
```

- [ ] **Step 7: Push**

Run:

```powershell
git push
```

Expected:

```text
main -> main
```

Do not move the `v0.1.0` tag for this refactor unless the user explicitly asks for a new release build.

---

## Self-Review

- Spec coverage: The plan covers code quality inspection, backend architecture split, frontend architecture split, CSS modularization, and verification.
- Behavior safety: The plan explicitly preserves command names, JSON layouts, switching behavior, OAuth behavior, quota behavior, updater behavior, and UI style.
- Placeholder scan: The plan avoids vague follow-up work. Task 6 includes concrete target bodies for `import_accounts` and `switch_account`.
- Type consistency: Frontend type names match current `App.vue` types and backend serialized models.
- Scope: The plan intentionally avoids fixing quota endpoint correctness, adding new sub-agent features, or redesigning UI behavior. Those should be separate plans.
