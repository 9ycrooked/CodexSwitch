# Codex Switch Updater Settings And Release Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add configurable startup update checks to Codex Switch, show a release-notes dialog when a new version is available, optionally enforce updates, and build Windows installers from tag-triggered GitHub Draft Releases.

**Architecture:** Use the official Tauri v2 updater plugin for update checks, downloads, signatures, and installation. Store user-facing update preferences in the existing app settings, run a frontend startup check after settings load, and use GitHub Release notes as the update dialog body. Build and publish updater artifacts through GitHub Actions as a Draft Release so the release body can be reviewed before publishing.

**Tech Stack:** Tauri 2, Vue 3, TypeScript, Yarn 4, Rust, `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process`, `tauri-plugin-updater`, GitHub Actions, GitHub Releases.

---

## File Structure

- Modify `package.json`
  - Add updater/process frontend plugin dependencies.
- Modify `src-tauri/Cargo.toml`
  - Add Rust updater plugin dependency.
- Modify `src-tauri/src/lib.rs`
  - Register updater plugin in the Tauri builder.
  - Extend `Settings` with update preferences.
  - Keep defaults backward-compatible when old settings files do not contain updater fields.
- Modify `src-tauri/capabilities/default.json`
  - Add updater and process permissions.
- Modify `src-tauri/tauri.conf.json`
  - Add updater config with `createUpdaterArtifacts`, `pubkey`, and GitHub Release endpoint.
  - Keep the app version synchronized with release tags.
- Modify `src/App.vue`
  - Import updater/process APIs.
  - Add startup update check flow.
  - Add update dialog UI state and handlers.
  - Add settings controls for update preferences.
- Modify `src/styles.css`
  - Style the update modal using the existing warm dark UI system.
- Create `.github/workflows/release.yml`
  - Build Windows installers on version tag push.
  - Upload signed updater artifacts to a Draft GitHub Release.
- Modify `README.md`
  - Document release steps: bump version, tag, wait for Draft Release, edit release notes, publish.

---

## Task 1: Add Updater Dependencies

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add frontend dependencies**

Run:

```powershell
yarn add @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

Expected:

```text
success Saved lockfile
```

This updates `package.json` and `yarn.lock`.

- [ ] **Step 2: Add Rust updater dependency**

Open `src-tauri/Cargo.toml` and add this dependency under `[dependencies]`:

```toml
tauri-plugin-updater = "2"
```

Keep existing dependencies unchanged.

- [ ] **Step 3: Verify dependency resolution**

Run:

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
Finished `dev` profile
```

- [ ] **Step 4: Commit**

```powershell
git add package.json yarn.lock src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: add updater dependencies"
```

---

## Task 2: Configure Tauri Updater

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Register the updater plugin**

In `src-tauri/src/lib.rs`, find the `tauri::Builder::default()` chain and add the updater plugin next to the existing dialog plugin registration:

```rust
.plugin(tauri_plugin_updater::Builder::new().build())
```

The builder chain should include both:

```rust
.plugin(tauri_plugin_dialog::init())
.plugin(tauri_plugin_updater::Builder::new().build())
```

- [ ] **Step 2: Add updater permissions**

In `src-tauri/capabilities/default.json`, add these permissions to the existing `permissions` array:

```json
"updater:default",
"process:default"
```

Keep existing window and dialog permissions.

- [ ] **Step 3: Add updater config**

In `src-tauri/tauri.conf.json`, add this top-level `plugins` object after `bundle`:

```json
"plugins": {
  "updater": {
    "pubkey": "REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY",
    "endpoints": [
      "https://github.com/9ycrooked/CodexSwitch/releases/latest/download/latest.json"
    ]
  }
}
```

In the existing `bundle` object, add:

```json
"createUpdaterArtifacts": true
```

The `bundle` block should keep `active`, `targets`, and `icon`.

- [ ] **Step 4: Generate signing keys before production release**

Run:

```powershell
yarn tauri signer generate -w ~/.tauri/codex-switch.key
```

Expected:

```text
Private key written to ...
Public key: ...
```

Replace `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY` in `src-tauri/tauri.conf.json` with the generated public key.

Store the private key value in GitHub Actions secret:

```text
TAURI_SIGNING_PRIVATE_KEY
```

Store the private key password, if generated, in:

```text
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

- [ ] **Step 5: Verify Tauri config**

Run:

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
Finished `dev` profile
```

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/lib.rs src-tauri/capabilities/default.json src-tauri/tauri.conf.json
git commit -m "chore: configure tauri updater"
```

---

## Task 3: Persist Update Settings

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Extend settings type**

In `src-tauri/src/lib.rs`, find the `Settings` struct and add:

```rust
#[serde(default = "default_true")]
check_updates_on_startup: bool,
#[serde(default)]
force_update_on_startup: bool,
```

Add this helper near the existing default helpers:

```rust
fn default_true() -> bool {
    true
}
```

- [ ] **Step 2: Update `Default for Settings`**

In `impl Default for Settings`, set:

```rust
check_updates_on_startup: true,
force_update_on_startup: false,
```

Do not change existing Codex home, process name, OAuth, or profile defaults.

- [ ] **Step 3: Add a settings migration test**

In the existing Rust tests module, add a test that deserializes older settings JSON without update fields:

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
        "oauth_login_mode": "webview"
    }"#;

    let settings: Settings = serde_json::from_str(raw).expect("settings should deserialize");

    assert!(settings.check_updates_on_startup);
    assert!(!settings.force_update_on_startup);
}
```

If field names differ in the actual `Settings` struct, use the exact existing field names and add only the two new updater fields.

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/lib.rs
git commit -m "feat: persist updater preferences"
```

---

## Task 4: Add Startup Update Check In Vue

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: Add updater imports**

At the top of `src/App.vue`, add:

```ts
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
```

- [ ] **Step 2: Add update state**

Inside the Vue script setup area, add:

```ts
const updateDialogOpen = ref(false);
const updateChecking = ref(false);
const updateDownloading = ref(false);
const updateError = ref("");
const pendingUpdate = ref<Update | null>(null);
const updateDownloadedBytes = ref(0);
const updateTotalBytes = ref(0);
```

- [ ] **Step 3: Add release notes helper**

Add this computed value:

```ts
const pendingUpdateNotes = computed(() => {
  const update = pendingUpdate.value as unknown as {
    body?: string;
    notes?: string;
    version?: string;
    currentVersion?: string;
  } | null;

  return update?.body || update?.notes || "这个版本没有填写更新说明。";
});
```

- [ ] **Step 4: Add startup check function**

Add:

```ts
async function checkForUpdatesOnStartup() {
  if (!settings.value.check_updates_on_startup) return;

  updateChecking.value = true;
  updateError.value = "";

  try {
    const update = await check();
    if (!update) return;

    pendingUpdate.value = update;
    updateDialogOpen.value = true;
  } catch (error) {
    updateError.value = `更新检查失败：${formatError(error)}`;
    if (settings.value.force_update_on_startup) {
      updateDialogOpen.value = true;
    }
  } finally {
    updateChecking.value = false;
  }
}
```

Use the existing project error formatter if it already has one. If the project uses `String(error)` everywhere, implement:

```ts
function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
```

- [ ] **Step 5: Add install handler**

Add:

```ts
async function installPendingUpdate() {
  if (!pendingUpdate.value) return;

  updateDownloading.value = true;
  updateError.value = "";
  updateDownloadedBytes.value = 0;
  updateTotalBytes.value = 0;

  try {
    await pendingUpdate.value.downloadAndInstall((event) => {
      if (event.event === "Started") {
        updateTotalBytes.value = event.data.contentLength ?? 0;
      }

      if (event.event === "Progress") {
        updateDownloadedBytes.value += event.data.chunkLength;
      }
    });

    await relaunch();
  } catch (error) {
    updateError.value = `更新安装失败：${formatError(error)}`;
  } finally {
    updateDownloading.value = false;
  }
}
```

- [ ] **Step 6: Add dismiss handler**

Add:

```ts
function dismissUpdateDialog() {
  if (settings.value.force_update_on_startup && pendingUpdate.value) return;
  updateDialogOpen.value = false;
}
```

- [ ] **Step 7: Call startup check after settings load**

Find the existing startup lifecycle where accounts/settings/current state are loaded. After settings are loaded into `settings.value`, call:

```ts
void checkForUpdatesOnStartup();
```

Do not run update checks before settings are available.

- [ ] **Step 8: Build the frontend**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 9: Commit**

```powershell
git add src/App.vue
git commit -m "feat: check for updates on startup"
```

---

## Task 5: Add Update Dialog UI

**Files:**
- Modify: `src/App.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Add update modal markup**

In `src/App.vue`, near existing modal/dialog markup, add:

```vue
<div v-if="updateDialogOpen" class="modal-backdrop">
  <section class="modal update-modal" role="dialog" aria-modal="true" aria-labelledby="update-modal-title">
    <div class="modal-header">
      <div>
        <p class="eyebrow">软件更新</p>
        <h2 id="update-modal-title">发现新版本</h2>
      </div>
      <button
        v-if="!settings.force_update_on_startup"
        class="icon-button"
        type="button"
        aria-label="关闭更新提示"
        @click="dismissUpdateDialog"
      >
        ×
      </button>
    </div>

    <div v-if="pendingUpdate" class="update-version-row">
      <span>当前版本 {{ pendingUpdate.currentVersion }}</span>
      <span>新版本 {{ pendingUpdate.version }}</span>
    </div>

    <pre class="update-notes">{{ pendingUpdateNotes }}</pre>

    <div v-if="updateDownloading" class="update-progress">
      <div class="quota-track">
        <div
          class="quota-fill ok"
          :style="{ width: updateTotalBytes ? `${Math.min(100, Math.round((updateDownloadedBytes / updateTotalBytes) * 100))}%` : '35%' }"
        ></div>
      </div>
      <p>{{ updateTotalBytes ? `${Math.round((updateDownloadedBytes / updateTotalBytes) * 100)}%` : '正在下载更新...' }}</p>
    </div>

    <p v-if="updateError" class="notice danger">{{ updateError }}</p>

    <div class="modal-actions">
      <button
        v-if="!settings.force_update_on_startup"
        class="btn secondary"
        type="button"
        :disabled="updateDownloading"
        @click="dismissUpdateDialog"
      >
        稍后
      </button>
      <button
        class="btn primary"
        type="button"
        :disabled="updateDownloading || !pendingUpdate"
        @click="installPendingUpdate"
      >
        {{ updateDownloading ? "正在更新" : "立即更新" }}
      </button>
    </div>
  </section>
</div>
```

If the project already has a modal class with different names, adapt the class names while preserving the same structure and behavior.

- [ ] **Step 2: Add modal styles**

In `src/styles.css`, add:

```css
.update-modal {
  width: min(560px, calc(100vw - 32px));
}

.update-version-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border-color);
  border-radius: 10px;
  color: var(--text-secondary);
  background: var(--bg-tertiary);
}

.update-notes {
  max-height: 260px;
  overflow: auto;
  margin: 14px 0 0;
  padding: 14px;
  border: 1px solid var(--border-color);
  border-radius: 10px;
  color: var(--text-primary);
  background: var(--bg-primary);
  font-family: inherit;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
}

.update-progress {
  margin-top: 14px;
}

.update-progress p {
  margin: 8px 0 0;
  color: var(--text-secondary);
  font-size: 13px;
}
```

- [ ] **Step 3: Verify frontend build**

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
git add src/App.vue src/styles.css
git commit -m "feat: show update release notes dialog"
```

---

## Task 6: Add Update Settings Controls

**Files:**
- Modify: `src/App.vue`
- Modify: `src/styles.css`

- [ ] **Step 1: Add controls to settings page**

In the settings page section of `src/App.vue`, add a settings card:

```vue
<section class="settings-card">
  <div class="settings-card-header">
    <div>
      <p class="eyebrow">UPDATER</p>
      <h3>自动更新</h3>
    </div>
  </div>

  <label class="toggle-row">
    <input v-model="settings.check_updates_on_startup" type="checkbox" />
    <span>
      <strong>启动时检查更新</strong>
      <small>软件启动后自动检查 GitHub Release 是否有新版本。</small>
    </span>
  </label>

  <label class="toggle-row">
    <input
      v-model="settings.force_update_on_startup"
      type="checkbox"
      :disabled="!settings.check_updates_on_startup"
    />
    <span>
      <strong>发现新版本时要求更新</strong>
      <small>开启后，发现新版本时必须更新或退出，不能继续使用旧版本。</small>
    </span>
  </label>
</section>
```

Use the app's existing save-settings action. Do not add a separate save button if settings already save through one shared button.

- [ ] **Step 2: Add settings styles if missing**

If `toggle-row` does not already exist in `src/styles.css`, add:

```css
.toggle-row {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--border-color);
  border-radius: 10px;
  background: var(--bg-tertiary);
}

.toggle-row input {
  width: 18px;
  height: 18px;
  margin-top: 2px;
  accent-color: var(--primary-color);
}

.toggle-row span {
  display: grid;
  gap: 4px;
}

.toggle-row strong {
  color: var(--text-primary);
  font-size: 13px;
}

.toggle-row small {
  color: var(--text-secondary);
  font-size: 12px;
  line-height: 1.45;
}
```

- [ ] **Step 3: Verify settings serialization**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Verify frontend build**

Run:

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 5: Commit**

```powershell
git add src/App.vue src/styles.css src-tauri/src/lib.rs
git commit -m "feat: add updater settings"
```

---

## Task 7: Add GitHub Actions Draft Release Build

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Create workflow file**

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  build-windows:
    runs-on: windows-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Enable Corepack
        run: corepack enable

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: yarn

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: src-tauri

      - name: Install frontend dependencies
        run: yarn install --immutable

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Codex Switch ${{ github.ref_name }}"
          releaseBody: |
            ## 更新内容
            - 请在发布前编辑这段 Release 正文。

            ## 注意事项
            - 发布后，Codex Switch 的更新弹窗会显示这里的内容。
          releaseDraft: true
          prerelease: false
```

- [ ] **Step 2: Commit**

```powershell
git add .github/workflows/release.yml
git commit -m "ci: build draft releases on tags"
```

- [ ] **Step 3: Push workflow**

```powershell
git push
```

Expected:

```text
main -> main
```

---

## Task 8: Document Release Workflow

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add release section**

Append this section to `README.md` before `## License`:

```markdown
## Release Workflow

Codex Switch uses Tauri updater artifacts and GitHub Draft Releases.

1. Update the version in:
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`

2. Commit and push:

   ```powershell
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
   git commit -m "chore: release v0.1.1"
   git push
   ```

3. Create and push a tag:

   ```powershell
   git tag v0.1.1
   git push origin v0.1.1
   ```

4. Wait for GitHub Actions to create a Draft Release.

5. Open the Draft Release and edit the Release body. This body is displayed in the app update dialog.

6. Confirm the installer and updater artifacts are attached.

7. Publish the Release.

After the Release is published, Codex Switch can detect it on startup and show the update dialog.
```

- [ ] **Step 2: Commit**

```powershell
git add README.md
git commit -m "docs: document release workflow"
```

---

## Task 9: End-To-End Verification

**Files:**
- Read only unless fixes are needed.

- [ ] **Step 1: Run Rust tests**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok
```

- [ ] **Step 2: Run frontend build**

```powershell
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 3: Run Tauri build**

```powershell
cargo build --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
Finished `dev` profile
```

- [ ] **Step 4: Manual app checks**

Run:

```powershell
yarn tauri dev
```

Verify:

- Settings page shows `启动时检查更新`.
- Settings page shows `发现新版本时要求更新`.
- Disabling startup checks prevents updater checks after restart.
- With no published newer release, startup does not block normal app usage.
- With a mocked or published newer release, update dialog shows the GitHub Release body.
- In non-force mode, `稍后` closes the dialog.
- In force mode, `稍后` is hidden and old-version usage is blocked by the modal.
- `立即更新` starts download/install and relaunches after install.

- [ ] **Step 5: Commit fixes if verification required changes**

```powershell
git add src src-tauri README.md .github
git commit -m "fix: stabilize updater flow"
```

- [ ] **Step 6: Push all commits**

```powershell
git push
```

Expected:

```text
main -> main
```

---

## Task 10: First Release Dry Run

**Files:**
- Modify version files only when ready for the release.

- [ ] **Step 1: Bump version to `0.1.1`**

Update:

```json
// package.json
"version": "0.1.1"
```

```json
// src-tauri/tauri.conf.json
"version": "0.1.1"
```

```toml
# src-tauri/Cargo.toml
version = "0.1.1"
```

- [ ] **Step 2: Commit version bump**

```powershell
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore: release v0.1.1"
git push
```

- [ ] **Step 3: Tag release**

```powershell
git tag v0.1.1
git push origin v0.1.1
```

- [ ] **Step 4: Review Draft Release**

On GitHub:

1. Open `https://github.com/9ycrooked/CodexSwitch/releases`.
2. Open the `v0.1.1` Draft Release.
3. Replace the placeholder body with:

```markdown
## 更新内容
- 新增启动时自动检查更新。
- 新增可配置的强制更新模式。
- 新增更新弹窗，显示 GitHub Release 正文。
- 新增 tag 触发的 Windows 安装包自动构建。

## 注意事项
- 如果开启强制更新，发现新版本后需要更新才能继续使用。
- 自动更新依赖 GitHub Release 中的 updater artifacts。
```

4. Confirm Windows installer and updater files are attached.
5. Click `Publish release`.

- [ ] **Step 5: Verify published release**

Open:

```text
https://github.com/9ycrooked/CodexSwitch/releases/latest/download/latest.json
```

Expected:

- JSON is downloadable.
- Version is `0.1.1`.
- Notes contain the edited GitHub Release body.
- Signature metadata is present.

---

## Self-Review

- Spec coverage: The plan covers configurable startup checks, optional forced updates, update dialog release notes, Tauri updater setup, GitHub Actions tag builds, Draft Release review, and README documentation.
- Placeholder scan: The only `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY` string is intentional and has a concrete generation step immediately after it.
- Type consistency: Frontend settings fields are consistently named `check_updates_on_startup` and `force_update_on_startup`, matching the Rust settings fields.
- Scope: The plan does not implement unrelated app behavior, quota changes, OAuth changes, or UI redesign outside the update dialog/settings controls.
