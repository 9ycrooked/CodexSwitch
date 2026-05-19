# OAuth Network Exit Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an advisory OAuth preflight network check that verifies the Tauri/Rust backend can reach OpenAI auth services and optionally reports the backend egress IP/country before OAuth login.

**Architecture:** Implement a focused Rust `network_check` module with small probe/parsing helpers, expose it through a Tauri command, wire the result into Vue settings and OAuth login flow. The check is advisory only: failed checks warn and let the user continue or cancel.

**Tech Stack:** Tauri 2, Rust `reqwest::blocking`, Vue 3 + TypeScript, existing toast/confirm patterns.

---

## File Structure

- Create `src-tauri/src/network_check.rs`: backend probe command, Cloudflare trace parser, result classifier, unit tests.
- Modify `src-tauri/src/models.rs`: shared serializable network check structs.
- Modify `src-tauri/src/settings.rs`: add settings for automatic pre-login checks and optional egress-region lookup.
- Modify `src-tauri/src/lib.rs`: register the new module and Tauri command.
- Modify `src-tauri/src/oauth.rs`: improve unsupported-region token exchange / refresh error messages.
- Modify `src/types.ts`: frontend types for settings and network check result.
- Modify `src/api/codexSwitchApi.ts`: frontend invoke wrapper.
- Modify `src/composables/useAccounts.ts`: run preflight before OAuth login.
- Modify `src/App.vue`: hold check state, pass settings and manual check handler.
- Modify `src/views/SettingsView.vue`: add the settings controls and manual check panel.

---

### Task 1: Add Shared Models And Settings

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/settings.rs`
- Modify: `src/types.ts`

- [ ] **Step 1: Add Rust model structs**

Add these structs to `src-tauri/src/models.rs` after `UsageState`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkProbeResult {
    pub name: String,
    pub status: String,
    pub latency_ms: Option<u128>,
    pub http_status: Option<u16>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkExitCheckResult {
    pub overall_status: String,
    pub checked_at: String,
    pub auth_reachable: bool,
    pub auth_status: Option<u16>,
    pub latency_ms: Option<u128>,
    pub backend_ip: Option<String>,
    pub backend_country: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub probes: Vec<NetworkProbeResult>,
}
```

- [ ] **Step 2: Add settings fields and defaults**

Add fields to `Settings` in `src-tauri/src/settings.rs`:

```rust
#[serde(default = "default_true")]
pub check_oauth_network_on_login: bool,
#[serde(default)]
pub check_egress_region: bool,
```

Update `default_settings()`:

```rust
check_oauth_network_on_login: true,
check_egress_region: false,
```

Update the `sanitized` value inside `update_settings()`:

```rust
check_oauth_network_on_login: settings.check_oauth_network_on_login,
check_egress_region: settings.check_egress_region,
```

- [ ] **Step 3: Extend the existing settings default test**

Update the JSON in `settings_defaults_update_preferences_when_missing()` to intentionally omit the new fields, then assert:

```rust
assert!(settings.check_oauth_network_on_login);
assert!(!settings.check_egress_region);
```

- [ ] **Step 4: Add frontend types**

Add to `src/types.ts`:

```ts
export type NetworkProbeResult = {
  name: string;
  status: string;
  latency_ms?: number | null;
  http_status?: number | null;
  detail?: string | null;
};

export type NetworkExitCheckResult = {
  overall_status: "ok" | "warning" | "failed" | string;
  checked_at: string;
  auth_reachable: boolean;
  auth_status?: number | null;
  latency_ms?: number | null;
  backend_ip?: string | null;
  backend_country?: string | null;
  warnings: string[];
  errors: string[];
  probes: NetworkProbeResult[];
};
```

Extend `Settings`:

```ts
check_oauth_network_on_login: boolean;
check_egress_region: boolean;
```

- [ ] **Step 5: Run the focused backend test**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml settings_defaults_update_preferences_when_missing
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/models.rs src-tauri/src/settings.rs src/types.ts
git commit -m "feat: add oauth network check settings"
```

---

### Task 2: Implement Backend Network Check Command

**Files:**
- Create: `src-tauri/src/network_check.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create module and write parser/classifier tests first**

Create `src-tauri/src/network_check.rs` with this test module skeleton and helper signatures above it:

```rust
use std::time::{Duration, Instant};

use crate::accounts::now_string;
use crate::error::{run_blocking, AppResult};
use crate::models::{NetworkExitCheckResult, NetworkProbeResult};
use crate::settings::load_settings;

const CLOUDFLARE_TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const OPENAI_AUTH_METADATA_URL: &str = "https://auth.openai.com/.well-known/openid-configuration";

fn parse_cloudflare_trace(text: &str) -> (Option<String>, Option<String>) {
    let mut ip = None;
    let mut loc = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("ip=") {
            let value = value.trim();
            if !value.is_empty() {
                ip = Some(value.to_string());
            }
        }
        if let Some(value) = line.strip_prefix("loc=") {
            let value = value.trim();
            if !value.is_empty() {
                loc = Some(value.to_string());
            }
        }
    }
    (ip, loc)
}

fn classify_result(result: &mut NetworkExitCheckResult) {
    if !result.errors.is_empty() {
        result.overall_status = "failed".to_string();
    } else if !result.warnings.is_empty() {
        result.overall_status = "warning".to_string();
    } else {
        result.overall_status = "ok".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cloudflare_trace_ip_and_country() {
        let (ip, loc) = parse_cloudflare_trace("fl=1\nip=203.0.113.10\nloc=JP\nwarp=off\n");
        assert_eq!(ip.as_deref(), Some("203.0.113.10"));
        assert_eq!(loc.as_deref(), Some("JP"));
    }

    #[test]
    fn parses_cloudflare_trace_missing_values_safely() {
        let (ip, loc) = parse_cloudflare_trace("warp=off\n");
        assert!(ip.is_none());
        assert!(loc.is_none());
    }

    #[test]
    fn classifies_network_check_status() {
        let mut result = NetworkExitCheckResult {
            checked_at: "2026-05-19T00:00:00Z".to_string(),
            auth_reachable: true,
            ..Default::default()
        };
        classify_result(&mut result);
        assert_eq!(result.overall_status, "ok");

        result.warnings.push("地区未知".to_string());
        classify_result(&mut result);
        assert_eq!(result.overall_status, "warning");

        result.errors.push("OpenAI auth 不可达".to_string());
        classify_result(&mut result);
        assert_eq!(result.overall_status, "failed");
    }
}
```

- [ ] **Step 2: Register the module for tests**

In `src-tauri/src/lib.rs`, add:

```rust
mod network_check;
```

- [ ] **Step 3: Run parser tests and verify they pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml network_check::
```

Expected: PASS for the three parser/classifier tests.

- [ ] **Step 4: Add command implementation**

Add this command and helpers to `src-tauri/src/network_check.rs` above the tests:

```rust
#[tauri::command]
pub async fn check_oauth_network_exit(
    include_egress_region: Option<bool>,
) -> AppResult<NetworkExitCheckResult> {
    run_blocking(move || check_oauth_network_exit_blocking(include_egress_region)).await
}

fn check_oauth_network_exit_blocking(
    include_egress_region: Option<bool>,
) -> AppResult<NetworkExitCheckResult> {
    let settings = load_settings()?;
    let include_egress_region = include_egress_region.unwrap_or(settings.check_egress_region);
    let mut result = NetworkExitCheckResult {
        checked_at: now_string(),
        overall_status: "warning".to_string(),
        auth_reachable: false,
        auth_status: None,
        latency_ms: None,
        backend_ip: None,
        backend_country: None,
        warnings: Vec::new(),
        errors: Vec::new(),
        probes: Vec::new(),
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| err.to_string())?;

    let auth_probe = probe_get(&client, "OpenAI OAuth", OPENAI_AUTH_METADATA_URL);
    result.auth_reachable = auth_probe.status == "ok";
    result.auth_status = auth_probe.http_status;
    result.latency_ms = auth_probe.latency_ms;
    if !result.auth_reachable {
        result.errors.push(format!(
            "后端无法访问 OpenAI OAuth 服务：{}",
            auth_probe
                .detail
                .clone()
                .unwrap_or_else(|| "未知错误".to_string())
        ));
    }
    result.probes.push(auth_probe);

    if include_egress_region {
        let trace_probe = probe_get(&client, "Cloudflare trace", CLOUDFLARE_TRACE_URL);
        if trace_probe.status == "ok" {
            if let Some(detail) = trace_probe.detail.as_deref() {
                let (ip, country) = parse_cloudflare_trace(detail);
                result.backend_ip = ip;
                result.backend_country = country;
                if result.backend_country.is_none() {
                    result
                        .warnings
                        .push("Cloudflare trace 可访问，但未解析到出口国家代码。".to_string());
                }
            }
        } else {
            result.warnings.push(format!(
                "出口地区查询失败：{}",
                trace_probe
                    .detail
                    .clone()
                    .unwrap_or_else(|| "未知错误".to_string())
            ));
        }
        result.probes.push(trace_probe);
    }

    classify_result(&mut result);
    Ok(result)
}

fn probe_get(
    client: &reqwest::blocking::Client,
    name: &str,
    url: &str,
) -> NetworkProbeResult {
    let started = Instant::now();
    match client.get(url).send() {
        Ok(response) => {
            let status = response.status();
            let http_status = Some(status.as_u16());
            let text = response.text().unwrap_or_default();
            let reachable = status.is_success() || status.is_redirection() || status.is_client_error();
            NetworkProbeResult {
                name: name.to_string(),
                status: if reachable { "ok" } else { "failed" }.to_string(),
                latency_ms: Some(started.elapsed().as_millis()),
                http_status,
                detail: if name == "Cloudflare trace" {
                    Some(text)
                } else if reachable {
                    Some(format!("HTTP {status}"))
                } else {
                    Some(format!("HTTP {status}: {text}"))
                },
            }
        }
        Err(err) => NetworkProbeResult {
            name: name.to_string(),
            status: "failed".to_string(),
            latency_ms: Some(started.elapsed().as_millis()),
            http_status: None,
            detail: Some(err.to_string()),
        },
    }
}
```

- [ ] **Step 5: Register command**

Add to `tauri::generate_handler!`:

```rust
network_check::check_oauth_network_exit,
```

- [ ] **Step 6: Run backend tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all tests PASS.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/network_check.rs src-tauri/src/lib.rs
git commit -m "feat: add oauth network exit check"
```

---

### Task 3: Improve Unsupported Region Error Text

**Files:**
- Modify: `src-tauri/src/oauth.rs`

- [ ] **Step 1: Add failing tests for token error formatting**

Add this test to `src-tauri/src/oauth.rs` test module:

```rust
#[test]
fn formats_unsupported_region_token_error() {
    let body = r#"{"error":{"code":"unsupported_country_region_territory","message":"Country, region, or territory not supported","type":"request_forbidden"}}"#;
    let message = format_token_error("token exchange", 403, body);
    assert!(message.contains("后端请求被 OpenAI 判定为不支持地区"));
    assert!(message.contains("浏览器登录窗口和软件后端可能没有使用同一个网络出口"));
}
```

- [ ] **Step 2: Implement helper**

Add this helper above `parse_token_http_response()`:

```rust
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
```

Update `parse_token_http_response()`:

```rust
if !status.is_success() {
    return Err(format_token_error(label, status.as_u16(), &body));
}
```

- [ ] **Step 3: Run focused OAuth tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml oauth::
```

Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/oauth.rs
git commit -m "fix: explain unsupported oauth region errors"
```

---

### Task 4: Wire Frontend API And OAuth Preflight

**Files:**
- Modify: `src/api/codexSwitchApi.ts`
- Modify: `src/composables/useAccounts.ts`
- Modify: `src/App.vue`

- [ ] **Step 1: Add API wrapper**

Update import in `src/api/codexSwitchApi.ts`:

```ts
import type {
  AccountSummary,
  AppPaths,
  BackupSummary,
  CodexState,
  NetworkExitCheckResult,
  QuotaState,
  Settings,
  SwitchResult,
  UsageState
} from "../types";
```

Add:

```ts
export function checkOauthNetworkExit(includeEgressRegion?: boolean) {
  return invoke<NetworkExitCheckResult>("check_oauth_network_exit", {
    includeEgressRegion: includeEgressRegion ?? null
  });
}
```

- [ ] **Step 2: Update `useAccounts` dependencies**

Update imports in `src/composables/useAccounts.ts`:

```ts
import type { AccountSummary, CodexState, Settings } from "../types";
```

Add to `deps`:

```ts
settings: Settings;
```

Add helper inside `useAccounts()` before `startOAuthLogin()`:

```ts
function summarizeNetworkCheck(result: Awaited<ReturnType<typeof api.checkOauthNetworkExit>>) {
  const lines = [
    `状态：${result.overall_status}`,
    result.backend_country ? `后端出口地区：${result.backend_country}` : null,
    result.backend_ip ? `后端出口 IP：${result.backend_ip}` : null,
    result.auth_status ? `OpenAI OAuth HTTP：${result.auth_status}` : null,
    ...result.errors,
    ...result.warnings
  ].filter(Boolean);
  return lines.join("\n");
}
```

Replace `startOAuthLogin()` with:

```ts
async function startOAuthLogin() {
  await runOperation("oauth:start", async () => {
    try {
      if (deps.settings.check_oauth_network_on_login) {
        const check = await api.checkOauthNetworkExit(deps.settings.check_egress_region);
        if (check.overall_status !== "ok") {
          const ok = window.confirm(
            `登录前网络检查提示异常：\n\n${summarizeNetworkCheck(check)}\n\n这不会阻止登录，但 token exchange 可能失败。是否继续 OAuth 登录？`
          );
          if (!ok) {
            deps.setMessage("info", "OAuth 登录已取消");
            return;
          }
        }
      }
      const result = await api.startOauthLogin();
      const modeText = result.mode === "embedded" ? "内置 WebView2" : "外部隔离浏览器";
      deps.setMessage("info", `已打开 ${modeText} OAuth 登录 Profile: ${result.browser_profile_dir}`);
    } catch (err) {
      deps.setMessage("error", String(err));
    }
  });
}
```

- [ ] **Step 3: Pass settings from App**

In `src/App.vue`, extend the initial `settings` reactive object:

```ts
check_oauth_network_on_login: true,
check_egress_region: false
```

Pass settings into `useAccounts`:

```ts
settings,
```

Update `saveSettings()` payload:

```ts
check_oauth_network_on_login: settings.check_oauth_network_on_login,
check_egress_region: settings.check_egress_region
```

- [ ] **Step 4: Run frontend type check/build**

Run:

```powershell
yarn build
```

Expected: `vue-tsc --noEmit` and Vite build PASS.

- [ ] **Step 5: Commit**

```powershell
git add src/api/codexSwitchApi.ts src/composables/useAccounts.ts src/App.vue
git commit -m "feat: run oauth network check before login"
```

---

### Task 5: Add Settings UI For Manual Check

**Files:**
- Modify: `src/App.vue`
- Modify: `src/views/SettingsView.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Add state and handler in App**

Update the `src/App.vue` import:

```ts
import type { AccountSummary, AppPaths, BackupSummary, CodexState, NetworkExitCheckResult, Settings } from "./types";
```

Add refs:

```ts
const networkCheckResult = ref<NetworkExitCheckResult | null>(null);
const networkCheckRunning = ref(false);
```

Add handler:

```ts
async function checkNetworkExitManually() {
  networkCheckRunning.value = true;
  try {
    networkCheckResult.value = await api.checkOauthNetworkExit(settings.check_egress_region);
    const type = networkCheckResult.value.overall_status === "failed"
      ? "error"
      : networkCheckResult.value.overall_status === "warning"
        ? "warning"
        : "success";
    setMessage(type, "登录前网络检查完成");
  } catch (err) {
    setMessage("error", String(err));
  } finally {
    networkCheckRunning.value = false;
  }
}
```

Pass props/emits to `SettingsView`:

```vue
:network-check-result="networkCheckResult"
:network-check-running="networkCheckRunning"
@check-network-exit="checkNetworkExitManually"
```

- [ ] **Step 2: Extend SettingsView props and emits**

In `src/views/SettingsView.vue`, update imports:

```ts
import type { AppPaths, NetworkExitCheckResult, Settings } from "../types";
```

Add props:

```ts
networkCheckResult: NetworkExitCheckResult | null;
networkCheckRunning: boolean;
```

Add emit:

```ts
checkNetworkExit: [];
```

- [ ] **Step 3: Add SettingsView template section**

Add this section after OAuth 登录方式 and before the Profile checkbox:

```vue
<section class="network-check-panel">
  <div class="panel-heading-row">
    <div>
      <span class="eyebrow">OAuth Network</span>
      <h3>登录前网络检查</h3>
      <p>检查软件后端是否能访问 OpenAI OAuth 服务；出口地区查询默认关闭。</p>
    </div>
    <button class="secondary" type="button" :disabled="busy || networkCheckRunning" @click="$emit('checkNetworkExit')">
      {{ networkCheckRunning ? "检查中" : "立即检查" }}
    </button>
  </div>
  <label class="checkbox-row">
    <input v-model="settings.check_oauth_network_on_login" type="checkbox" />
    <span>OAuth 登录前自动检查后端网络</span>
  </label>
  <label class="checkbox-row">
    <input v-model="settings.check_egress_region" type="checkbox" />
    <span>显示后端出口 IP 和国家代码（使用 Cloudflare trace）</span>
  </label>
  <div v-if="networkCheckResult" class="network-check-result">
    <strong>最近结果：{{ networkCheckResult.overall_status }}</strong>
    <span v-if="networkCheckResult.backend_country">出口地区：{{ networkCheckResult.backend_country }}</span>
    <span v-if="networkCheckResult.backend_ip">出口 IP：{{ networkCheckResult.backend_ip }}</span>
    <span v-if="networkCheckResult.auth_status">OAuth HTTP：{{ networkCheckResult.auth_status }}</span>
    <small v-for="item in networkCheckResult.errors" :key="`error-${item}`">{{ item }}</small>
    <small v-for="item in networkCheckResult.warnings" :key="`warning-${item}`">{{ item }}</small>
    <small v-if="networkCheckResult.backend_country">出口地区仅供参考，OpenAI token exchange 的最终判定可能不同。</small>
  </div>
</section>
```

- [ ] **Step 4: Add compact styles**

Add to `src/styles.css`:

```css
.network-check-panel {
  border: 1px solid var(--border-color);
  border-radius: 10px;
  padding: 16px;
  background: var(--bg-primary);
}

.panel-heading-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.panel-heading-row p {
  color: var(--text-secondary);
  margin: 4px 0 0;
}

.network-check-result {
  display: grid;
  gap: 6px;
  margin-top: 12px;
  color: var(--text-secondary);
}

.network-check-result strong {
  color: var(--text-primary);
}

.network-check-result small {
  color: var(--text-muted);
}
```

- [ ] **Step 5: Run frontend build**

Run:

```powershell
yarn build
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add src/App.vue src/views/SettingsView.vue src/styles.css
git commit -m "feat: add oauth network check settings ui"
```

---

### Task 6: Full Verification

**Files:**
- No code changes unless verification finds a defect.

- [ ] **Step 1: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected: all tests PASS.

- [ ] **Step 2: Run frontend build**

Run:

```powershell
yarn build
```

Expected: `vue-tsc --noEmit` and Vite build PASS.

- [ ] **Step 3: Run Tauri no-bundle build**

Run:

```powershell
yarn tauri build --no-bundle
```

Expected: Windows app binary builds successfully.

- [ ] **Step 4: Manual acceptance**

Start dev mode:

```powershell
yarn tauri dev
```

Verify:

- Settings page shows “登录前网络检查”.
- Default state: automatic OAuth network check enabled; egress-region lookup disabled.
- Manual check without egress-region lookup shows OpenAI OAuth reachability only.
- Enabling egress-region lookup shows backend IP/country when Cloudflare trace is reachable.
- Clicking OAuth 登录 runs preflight first.
- If preflight fails or warns, a confirmation dialog appears and still allows continuing.
- `unsupported_country_region_territory` token exchange error explains backend egress mismatch.

- [ ] **Step 5: Final commit if verification required fixes**

If any verification fix was made:

```powershell
git add <changed-files>
git commit -m "fix: stabilize oauth network check"
```

If no verification fix was made, do not create an empty commit.

---

## Rollback Notes

- The feature is isolated behind two settings fields.
- If the network check causes unexpected issues, disable `check_oauth_network_on_login` in settings.
- The OAuth login command itself remains unchanged except for clearer error text; removing the preflight call in `useAccounts.ts` restores previous behavior.
