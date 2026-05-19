# Product UX Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add lightweight product polish so users can see the app version, update-check status, and open important data/backup directories from the UI.

**Architecture:** Add narrowly scoped Tauri commands for opening known safe directories instead of exposing arbitrary path opening. Extend the frontend API and existing views with small UI additions while preserving current account switching, OAuth, quota, and backup behavior.

**Tech Stack:** Tauri 2, Rust, Vue 3, TypeScript, Yarn, lucide-vue-next.

---

## File Structure

- Modify `src-tauri/src/models.rs`: add a small `AppPaths` response type for settings page path display.
- Create `src-tauri/src/locations.rs`: centralize app path reads and safe directory opening commands.
- Modify `src-tauri/src/lib.rs`: register the new locations commands.
- Modify `src/api/codexSwitchApi.ts`: add typed wrappers for path and open-directory commands.
- Modify `src/types.ts`: add the `AppPaths` frontend type.
- Modify `src/composables/useUpdater.ts`: track `lastUpdateCheckedAt`.
- Modify `src/App.vue`: load app version and app paths, wire directory open handlers, pass props/events to views.
- Modify `src/views/SettingsView.vue`: show app version, last update check time, and directory open buttons.
- Modify `src/views/BackupsView.vue`: add open backup root and per-backup directory buttons.
- Modify `src/styles/components.css` or existing relevant style files only if spacing/layout needs support.
- Add `.github/release-notes/v0.2.5.md` and bump version files only after implementation and verification.

---

### Task 1: Backend Safe Directory Commands

**Files:**
- Create: `src-tauri/src/locations.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the AppPaths model**

In `src-tauri/src/models.rs`, add this public struct near the other response models:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub codex_home: String,
    pub app_store_dir: String,
    pub backups_dir: String,
    pub browser_profile_dir: String,
}
```

- [ ] **Step 2: Create locations.rs**

Create `src-tauri/src/locations.rs`:

```rust
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{stringify_io, AppResult};
use crate::models::AppPaths;
use crate::paths::app_store_dir;
use crate::process::hidden_command;
use crate::settings::load_settings;

fn open_dir(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    if !path.is_dir() {
        return Err(format!("不是目录：{}", path.display()));
    }

    hidden_command("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(stringify_io)?;
    Ok(())
}

fn ensure_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path).map_err(stringify_io)
}

fn reject_path_escape(value: &str) -> AppResult<()> {
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        return Err("备份 ID 不能是绝对路径。".into());
    }
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err("备份 ID 不能包含路径逃逸片段。".into());
    }
    Ok(())
}

fn backups_dir() -> AppResult<PathBuf> {
    Ok(app_store_dir()?.join("backups"))
}

#[tauri::command]
pub fn read_app_paths() -> AppResult<AppPaths> {
    let settings = load_settings()?;
    let app_store_dir = app_store_dir()?;
    let backups_dir = app_store_dir.join("backups");
    Ok(AppPaths {
        codex_home: settings.codex_home,
        app_store_dir: app_store_dir.to_string_lossy().to_string(),
        backups_dir: backups_dir.to_string_lossy().to_string(),
        browser_profile_dir: settings.browser_profile_dir,
    })
}

#[tauri::command]
pub fn open_codex_home_dir() -> AppResult<()> {
    let settings = load_settings()?;
    open_dir(&PathBuf::from(settings.codex_home))
}

#[tauri::command]
pub fn open_app_store_dir() -> AppResult<()> {
    let dir = app_store_dir()?;
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_browser_profile_dir() -> AppResult<()> {
    let settings = load_settings()?;
    let dir = PathBuf::from(settings.browser_profile_dir);
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_backups_dir() -> AppResult<()> {
    let dir = backups_dir()?;
    ensure_dir(&dir)?;
    open_dir(&dir)
}

#[tauri::command]
pub fn open_backup_dir(backup_id: String) -> AppResult<()> {
    reject_path_escape(&backup_id)?;
    let root = backups_dir()?;
    let dir = root.join(backup_id);
    open_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_backup_id_escape() {
        assert!(reject_path_escape("..\\secret").is_err());
        assert!(reject_path_escape("../secret").is_err());
        assert!(reject_path_escape("C:\\secret").is_err());
    }

    #[test]
    fn accepts_normal_backup_id() {
        assert!(reject_path_escape("20260519-120000-abcdef").is_ok());
    }
}
```

- [ ] **Step 3: Register the module and commands**

In `src-tauri/src/lib.rs`, add the module:

```rust
mod locations;
```

Add these entries to `tauri::generate_handler!`:

```rust
locations::read_app_paths,
locations::open_codex_home_dir,
locations::open_app_store_dir,
locations::open_browser_profile_dir,
locations::open_backups_dir,
locations::open_backup_dir,
```

- [ ] **Step 4: Run backend tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok.
```

- [ ] **Step 5: Commit backend task**

```powershell
git add src-tauri/src/models.rs src-tauri/src/locations.rs src-tauri/src/lib.rs
git commit -m "feat: add safe directory open commands"
```

---

### Task 2: Frontend API and Updater State

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api/codexSwitchApi.ts`
- Modify: `src/composables/useUpdater.ts`

- [ ] **Step 1: Add AppPaths frontend type**

In `src/types.ts`, add:

```ts
export type AppPaths = {
  codex_home: string;
  app_store_dir: string;
  backups_dir: string;
  browser_profile_dir: string;
};
```

- [ ] **Step 2: Add API wrappers**

In `src/api/codexSwitchApi.ts`, update the type import:

```ts
import type { AccountSummary, AppPaths, BackupSummary, CodexState, QuotaState, Settings, SwitchResult, UsageState } from "../types";
```

Add these functions:

```ts
export function readAppPaths() {
  return invoke<AppPaths>("read_app_paths");
}

export function openCodexHomeDir() {
  return invoke("open_codex_home_dir");
}

export function openAppStoreDir() {
  return invoke("open_app_store_dir");
}

export function openBrowserProfileDir() {
  return invoke("open_browser_profile_dir");
}

export function openBackupsDir() {
  return invoke("open_backups_dir");
}

export function openBackupDir(backupId: string) {
  return invoke("open_backup_dir", { backupId });
}
```

- [ ] **Step 3: Track last update check time**

In `src/composables/useUpdater.ts`, add:

```ts
const lastUpdateCheckedAt = ref<string | null>(null);
```

In `runUpdateCheck`, set it in the `finally` block after `updateChecking.value = false;`:

```ts
lastUpdateCheckedAt.value = new Date().toISOString();
```

Return it from the composable:

```ts
lastUpdateCheckedAt,
```

- [ ] **Step 4: Build frontend**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 5: Commit frontend API task**

```powershell
git add src/types.ts src/api/codexSwitchApi.ts src/composables/useUpdater.ts
git commit -m "feat: expose app paths and update check time"
```

---

### Task 3: Wire App State and Actions

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: Import APIs and Tauri version helper**

In `src/App.vue`, import `getVersion`:

```ts
import { getVersion } from "@tauri-apps/api/app";
```

Add the new API imports:

```ts
  readAppPaths,
  openAppStoreDir,
  openBackupDir,
  openBackupsDir,
  openBrowserProfileDir,
  openCodexHomeDir,
```

- [ ] **Step 2: Add state**

Add:

```ts
const appVersion = ref("");
const appPaths = ref<AppPaths | null>(null);
```

Import `AppPaths` from `./types` if needed.

- [ ] **Step 3: Load version and paths during initialization**

Add a function:

```ts
async function refreshAppInfo() {
  const [version, paths] = await Promise.all([getVersion(), readAppPaths()]);
  appVersion.value = version;
  appPaths.value = paths;
}
```

Call it during the existing startup load flow after settings are loaded:

```ts
await refreshAppInfo();
```

If the current startup flow does not have one obvious async loader, call `void refreshAppInfo();` in the same place other initial loads are triggered.

- [ ] **Step 4: Add operation wrapper for opening directories**

Add:

```ts
async function runOpenDirectory(key: string, action: () => Promise<unknown>) {
  return runOperation(`open:${key}`, async () => {
    await action();
    setMessage("已打开目录。");
  });
}
```

Add handlers:

```ts
const openCodexHome = () => runOpenDirectory("codex-home", openCodexHomeDir);
const openAppData = () => runOpenDirectory("app-data", openAppStoreDir);
const openProfiles = () => runOpenDirectory("profiles", openBrowserProfileDir);
const openBackups = () => runOpenDirectory("backups", openBackupsDir);
const openBackup = (backup: BackupSummary) => runOpenDirectory(`backup:${backup.id}`, () => openBackupDir(backup.id));
```

- [ ] **Step 5: Pass props/events to views**

Pass to `SettingsView`:

```vue
:app-version="appVersion"
:app-paths="appPaths"
:last-update-checked-at="lastUpdateCheckedAt"
@open-codex-home="openCodexHome"
@open-app-data="openAppData"
@open-profiles="openProfiles"
```

Pass to `BackupsView`:

```vue
@open-backups="openBackups"
@open-backup="openBackup"
```

- [ ] **Step 6: Build frontend**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 7: Commit app wiring task**

```powershell
git add src/App.vue
git commit -m "feat: wire directory actions into app"
```

---

### Task 4: Settings and Backups UI

**Files:**
- Modify: `src/views/SettingsView.vue`
- Modify: `src/views/BackupsView.vue`
- Modify: `src/styles/components.css` or nearest existing style file only if the current layout needs spacing support

- [ ] **Step 1: Extend SettingsView props and emits**

In `src/views/SettingsView.vue`, import `AppPaths`:

```ts
import type { AppPaths, Settings } from "../types";
import { formatDate } from "../utils/format";
```

Extend props:

```ts
  appVersion: string;
  appPaths: AppPaths | null;
  lastUpdateCheckedAt: string | null;
```

Extend emits:

```ts
  openCodexHome: [];
  openAppData: [];
  openProfiles: [];
```

- [ ] **Step 2: Add app info/update status panel**

In the template, before existing form fields, add:

```vue
<section class="settings-info-grid">
  <article class="info-card">
    <span class="eyebrow">App</span>
    <h3>应用信息</h3>
    <p>当前版本：{{ appVersion || "读取中" }}</p>
    <p>最近检查：{{ lastUpdateCheckedAt ? formatDate(lastUpdateCheckedAt) : "从未检查" }}</p>
  </article>
  <article class="info-card">
    <span class="eyebrow">Storage</span>
    <h3>数据位置</h3>
    <p>应用数据：{{ appPaths?.app_store_dir || "读取中" }}</p>
    <div class="inline-actions">
      <button class="secondary" type="button" :disabled="busy" @click="$emit('openAppData')">打开应用数据</button>
    </div>
  </article>
</section>
```

- [ ] **Step 3: Add path open buttons to existing fields**

For Codex home, add a secondary button next to the input:

```vue
<div class="field-with-action">
  <input v-model="settings.codex_home" />
  <button class="secondary" type="button" :disabled="busy" @click="$emit('openCodexHome')">打开</button>
</div>
```

For WebView2 Profile directory:

```vue
<div class="field-with-action">
  <input v-model="settings.browser_profile_dir" />
  <button class="secondary" type="button" :disabled="busy" @click="$emit('openProfiles')">打开</button>
</div>
```

Keep all other settings fields unchanged.

- [ ] **Step 4: Extend BackupsView emits**

In `src/views/BackupsView.vue`, extend emits:

```ts
  openBackups: [];
  openBackup: [BackupSummary];
```

- [ ] **Step 5: Add empty-state backup root button**

Inside the empty state:

```vue
<button class="secondary" type="button" :disabled="busy" @click="$emit('openBackups')">
  打开备份目录
</button>
```

- [ ] **Step 6: Add per-backup open button**

In each backup row actions area, add:

```vue
<div class="row-actions">
  <button
    class="secondary"
    type="button"
    :disabled="busy || isOperationActive('open:backup:' + backup.id)"
    @click="$emit('openBackup', backup)"
  >
    打开目录
  </button>
  <button
    class="secondary"
    :disabled="busy || isOperationActive('backup:restore:' + backup.id)"
    @click="$emit('restoreBackup', backup)"
  >
    恢复
  </button>
</div>
```

- [ ] **Step 7: Add minimal layout CSS if needed**

If no equivalent classes exist, add:

```css
.settings-info-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 12px;
}

.info-card {
  display: grid;
  gap: 8px;
  padding: 14px;
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  background: var(--bg-secondary);
}

.field-with-action,
.inline-actions,
.row-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.field-with-action input {
  min-width: 0;
  flex: 1 1 auto;
}
```

- [ ] **Step 8: Build frontend**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 9: Commit UI task**

```powershell
git add src/views/SettingsView.vue src/views/BackupsView.vue src/styles
git commit -m "feat: add app info and directory shortcuts"
```

---

### Task 5: Release 0.2.5 Preparation

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.lock`
- Create: `.github/release-notes/v0.2.5.md`

- [ ] **Step 1: Bump versions to 0.2.5**

Change all current `0.2.4` app versions to:

```text
0.2.5
```

Files:

```text
package.json
src-tauri/Cargo.toml
src-tauri/tauri.conf.json
src-tauri/Cargo.lock
```

- [ ] **Step 2: Add release notes**

Create `.github/release-notes/v0.2.5.md`:

```md
## Codex Switch v0.2.5

这是一个产品体验轻量打磨版本，重点让版本信息、更新检查状态和本地数据位置更容易查看。

### 新增

- 设置页新增当前版本显示。
- 设置页新增最近一次更新检查时间。
- 设置页新增 Codex home、应用数据目录、WebView2 Profile 目录快捷打开入口。
- 备份页新增备份根目录和单个备份目录快捷打开入口。

### 改进

- 打开目录操作使用固定范围后端命令，不暴露任意路径打开能力。
- 打开目录时复用隐藏进程逻辑，避免弹出命令行窗口。

### 验证

- Rust 后端测试通过：`15 passed`
- 前端生产构建通过：`yarn build`
- Tauri release 构建通过：`yarn tauri build --no-bundle`
```

- [ ] **Step 3: Run full verification**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
yarn build
yarn tauri build --no-bundle
```

Expected:

```text
test result: ok.
✓ built
Finished `release` profile
```

- [ ] **Step 4: Commit release prep**

```powershell
git add package.json src-tauri\Cargo.toml src-tauri\tauri.conf.json src-tauri\Cargo.lock .github\release-notes\v0.2.5.md
git commit -m "chore: prepare release 0.2.5"
```

- [ ] **Step 5: Push and tag**

```powershell
git push origin main
git tag -a v0.2.5 -m "Codex Switch v0.2.5"
git push origin v0.2.5
```

---

## Final Verification Checklist

- [ ] `cargo test --manifest-path src-tauri\Cargo.toml` passes.
- [ ] `yarn build` passes.
- [ ] `yarn tauri build --no-bundle` passes.
- [ ] Settings page shows app version.
- [ ] Settings page shows last update check time.
- [ ] Settings page opens Codex home, app data, and WebView2 Profile directories.
- [ ] Backups page opens backup root when empty.
- [ ] Backups page opens individual backup directories when backups exist.
- [ ] Directory open commands do not show a console window.
- [ ] `open_backup_dir("..\\x")` is rejected by tests.
- [ ] `logo.psd` remains untracked and untouched.
