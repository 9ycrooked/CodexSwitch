# Nonblocking Defensive Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent Codex Switch operations from freezing the UI or opening console windows by applying defensive command design, hidden Windows child processes, and background execution for blocking work.

**Architecture:** Keep existing Tauri command names and frontend behavior, but move blocking command bodies behind `tauri::async_runtime::spawn_blocking`. Add a small process helper that hides Windows console child processes and centralizes system command execution. Reduce fixed waits where possible and prepare the frontend to show operation-specific loading instead of locking the whole app.

**Tech Stack:** Tauri 2, Rust 2021, Vue 3, TypeScript, Windows process APIs.

---

## Current Findings

Blocking or UI-risky areas found by source scan:

- `src-tauri/src/codex_home.rs`
  - `Command::new("taskkill")`
  - `thread::sleep(Duration::from_millis(settings.close_timeout_ms))`
- `src-tauri/src/oauth.rs`
  - `reqwest::blocking`
  - `Command::new(browser)`
  - `Command::new("rundll32")`
  - callback server thread sleeps
- `src-tauri/src/quota.rs`
  - `reqwest::blocking`
- `src-tauri/src/commands.rs`
  - `switch_account` coordinates process closing, backup, TOML merge, file writes, rollback
- `src-tauri/src/backups.rs`
  - backup/restore file IO
- `src-tauri/src/settings.rs` and `accounts.rs`
  - mostly small local file operations, lower priority

Project already uses TypeScript:

- `tsconfig.json`
- `typescript`
- `vue-tsc`
- `.ts` files in `src/api`, `src/composables`, `src/utils`, `src/types.ts`

---

## Guardrails

Do not change:

- Tauri command names.
- JSON payload shapes.
- Account library paths.
- `auth.json` / `config.toml` behavior.
- OAuth URLs or token payloads.
- Quota endpoint payloads.
- UI visual design.

Do change:

- Whether command implementation runs synchronously on the calling command thread.
- How Windows child processes are spawned.
- Frontend loading granularity where it prevents unnecessary UI-wide locking.

---

## Task 1: Add Hidden Windows Process Helper

**Files:**
- Create: `src-tauri/src/process.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/codex_home.rs`
- Modify: `src-tauri/src/oauth.rs`

- [ ] **Step 1: Create `process.rs`**

Create:

```rust
use std::process::{Command, Output};

use crate::error::{stringify_io, AppResult};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

pub fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    hide_console(&mut command);
    command
}

pub fn hidden_output(program: &str, args: &[&str]) -> AppResult<Output> {
    hidden_command(program)
        .args(args)
        .output()
        .map_err(stringify_io)
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/lib.rs`, add:

```rust
mod process;
```

- [ ] **Step 3: Hide `taskkill` windows**

In `src-tauri/src/codex_home.rs`, replace direct `Command::new("taskkill")` calls with `crate::process::hidden_command("taskkill")`.

Expected shape:

```rust
let gentle = crate::process::hidden_command("taskkill")
    .args(["/IM", name, "/T"])
    .output();
```

and:

```rust
let forced = crate::process::hidden_command("taskkill")
    .args(["/IM", name, "/T", "/F"])
    .output();
```

- [ ] **Step 4: Hide external OAuth browser launcher windows**

In `src-tauri/src/oauth.rs`, replace:

```rust
Command::new(browser)
Command::new("rundll32")
```

with:

```rust
crate::process::hidden_command(browser)
crate::process::hidden_command("rundll32")
```

- [ ] **Step 5: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/src/process.rs src-tauri/src/codex_home.rs src-tauri/src/oauth.rs
git commit -m "fix: hide windows child process consoles"
```

---

## Task 2: Move Blocking Tauri Commands To Background Threads

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/backups.rs`
- Modify: `src-tauri/src/oauth.rs`
- Modify: `src-tauri/src/quota.rs`

- [ ] **Step 1: Add helper for blocking joins**

Create helper in `src-tauri/src/commands.rs` or `src-tauri/src/error.rs`:

```rust
pub async fn run_blocking<T, F>(work: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("后台任务执行失败：{err}"))?
}
```

If placed in `error.rs`, import it from other modules as `crate::error::run_blocking`.

- [ ] **Step 2: Convert heavy command wrappers to async**

Convert command functions whose bodies block into async wrappers:

```rust
#[tauri::command]
pub async fn switch_account(account_id: String) -> AppResult<SwitchResult> {
    crate::error::run_blocking(move || switch_account_blocking(account_id)).await
}

fn switch_account_blocking(account_id: String) -> AppResult<SwitchResult> {
    // existing body
}
```

Apply same pattern to:

- `commands::import_accounts`
- `backups::backup_current_state`
- `backups::restore_backup`
- `oauth::refresh_account_tokens`
- `quota::check_account_quota`
- `quota::fetch_codex_usage`
- `quota::clear_usage_state`

Do not wrap simple read/list commands unless they are slow in practice.

- [ ] **Step 3: Keep command names unchanged**

Do not rename functions registered in `generate_handler!`. Tauri command names must remain:

- `import_accounts`
- `switch_account`
- `backup_current_state`
- `restore_backup`
- `refresh_account_tokens`
- `check_account_quota`
- `fetch_codex_usage`
- `clear_usage_state`

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 5: Run Tauri build**

Run:

```powershell
yarn tauri build --no-bundle
```

Expected:

```text
Built application at:
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src
git commit -m "perf: run blocking commands off the command thread"
```

---

## Task 3: Replace Fixed Codex Close Sleep With Polling

**Files:**
- Modify: `src-tauri/src/codex_home.rs`

- [ ] **Step 1: Add hidden process query helper**

Add:

```rust
fn is_process_running(name: &str) -> bool {
    let output = crate::process::hidden_command("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {name}"), "/FO", "CSV", "/NH"])
        .output();

    let Ok(output) = output else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    text.to_ascii_lowercase().contains(&name.to_ascii_lowercase())
}
```

- [ ] **Step 2: Add bounded wait**

Add:

```rust
fn wait_until_processes_exit(process_names: &[String], timeout_ms: u64) {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    while std::time::Instant::now() < deadline {
        if process_names.iter().all(|name| !is_process_running(name)) {
            return;
        }
        thread::sleep(Duration::from_millis(150));
    }
}
```

- [ ] **Step 3: Replace fixed sleep**

Replace:

```rust
thread::sleep(Duration::from_millis(settings.close_timeout_ms));
```

with:

```rust
wait_until_processes_exit(&settings.process_names, settings.close_timeout_ms);
```

This preserves the max timeout but returns sooner when Codex exits quickly.

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
git add src-tauri/src/codex_home.rs
git commit -m "perf: poll codex process shutdown"
```

---

## Task 4: Add Frontend Operation-Specific Loading State

**Files:**
- Modify: `src/composables/useAccounts.ts`
- Modify: `src/composables/useQuota.ts`
- Modify: `src/composables/useBackups.ts`
- Modify: `src/App.vue`
- Modify: `src/views/AccountsView.vue`
- Modify: `src/views/QuotaView.vue`
- Modify: `src/views/BackupsView.vue`

- [ ] **Step 1: Add operation key state**

In `App.vue`, add:

```ts
const activeOperation = ref("");
```

Pass it into composables.

- [ ] **Step 2: Use scoped operation keys**

In composables, replace broad `busy.value = true` for long operations with:

```ts
activeOperation.value = `switch:${account.id}`;
```

and clear in `finally`:

```ts
activeOperation.value = "";
```

Use keys:

- `switch:<account_id>`
- `refresh-token:<account_id>`
- `quota:<account_id>`
- `backup:create`
- `backup:restore:<backup_id>`
- `oauth:start`
- `oauth:close`
- `settings:save`
- `update:check`
- `update:install`

- [ ] **Step 3: Keep global busy only for initial refresh**

Keep `busy` for startup loading and full refresh only. Long single-account operations should not disable unrelated account cards.

- [ ] **Step 4: Add view helper props**

Pass helpers to views:

```ts
const isOperationActive = (key: string) => activeOperation.value === key;
const hasActiveOperation = computed(() => Boolean(activeOperation.value));
```

Disable only relevant buttons:

```vue
:disabled="isOperationActive(`switch:${account.id}`)"
```

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
git add src/App.vue src/composables src/views
git commit -m "perf: scope frontend loading states"
```

---

## Task 5: Add Defensive Command Checklist Documentation

**Files:**
- Create: `docs/architecture/defensive-tauri-commands.md`

- [ ] **Step 1: Create architecture note**

Create:

```md
# Defensive Tauri Commands

Before adding or editing a Tauri command, classify the work:

## Fast

Small reads, pure formatting, settings validation.

Allowed:
- sync command

## Blocking

File IO, process execution, `thread::sleep`, blocking HTTP, TOML/JSON processing on large files.

Required:
- `async fn` command
- `tauri::async_runtime::spawn_blocking`
- no visible child console windows on Windows

## Waiting

Network, OAuth callback, timers, process shutdown waits.

Required:
- async or background thread
- bounded timeout
- user-visible status

## Long Running

Batch import, batch quota refresh, future multi-account scans.

Required:
- operation id
- progress/status event or scoped loading state
- cancellation plan when practical

## Windows Process Rule

Use `crate::process::hidden_command()` for child processes that should not show a console window.

## UI Rule

Do not use one global `busy` flag for account-specific operations. Use operation-specific keys such as `switch:<account_id>` or `quota:<account_id>`.
```

- [ ] **Step 2: Commit**

```powershell
git add docs/architecture/defensive-tauri-commands.md
git commit -m "docs: add defensive command guidelines"
```

---

## Task 6: Final Verification

**Files:**
- Read only unless a verification failure requires a fix.

- [ ] **Step 1: Run Rust tests**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok. 15 passed; 0 failed
```

- [ ] **Step 2: Run frontend build**

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 3: Run Tauri no-bundle build**

```powershell
yarn tauri build --no-bundle
```

Expected:

```text
Built application at:
```

- [ ] **Step 4: Manual smoke test**

Run:

```powershell
yarn tauri dev
```

Verify:

- Switching account no longer opens a visible console window.
- Closing Codex returns faster when Codex exits quickly.
- Quota check does not freeze unrelated account cards.
- Token refresh does not freeze unrelated account cards.
- Backup restore still works.
- Update check dialog still works.

- [ ] **Step 5: Push**

```powershell
git push
```

Do not move `v0.1.0` tag unless the user explicitly asks for a release rebuild.

---

## Self-Review

- Coverage: The plan addresses visible console windows, blocking backend commands, fixed sleep, frontend UI-wide busy state, and defensive documentation.
- Safety: Command names and payloads remain unchanged.
- TypeScript: The project already uses TypeScript; this plan keeps TypeScript and improves state typing around operations.
- Risk: The biggest risk is converting command functions to async while preserving Tauri command names. The plan uses wrapper/private blocking functions to make behavior preservation straightforward.
