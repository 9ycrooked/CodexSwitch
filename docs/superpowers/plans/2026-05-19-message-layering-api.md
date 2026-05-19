# Message Layering API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce the three-layer message model by upgrading `setMessage` from the old boolean error flag to explicit toast message types across the app.

**Architecture:** Keep page state and decision dialogs where they are, and route only immediate operation feedback through the Toast system. The frontend composables receive a strongly typed `setMessage(type, message)` callback using `ToastType`, so TypeScript rejects the old `setMessage(message, true)` and `setMessage(message)` patterns.

**Tech Stack:** Vue 3 Composition API, TypeScript, existing `useNotifications` Toast system, no new dependencies.

---

## Message Layering Rules

Use this rule before adding or changing any user-facing message:

```text
Toast: what just happened
Page state: what is currently true
Dialog/confirm: what the user must decide
```

Toast examples:

- `success`: settings saved, backup created, account switched, tokens refreshed.
- `info`: OAuth login opened, OAuth login window closed, app already up to date.
- `warning`: operation completed with partial failures or non-blocking warnings.
- `error`: operation failed or cannot continue.

Do not move these into Toast:

- Current Codex account, auth/config file existence, quota bars, quota reset time.
- Settings-page security warnings and configured paths.
- Update dialog release notes, forced update state, and update install decision.
- Confirmation before destructive or replacing operations.

---

## File Structure

- Modify: `src/App.vue`
  - Import `ToastType`.
  - Change `setMessage(message, isError?)` to `setMessage(type, message)`.
  - Reclassify direct `setMessage` calls.
- Modify: `src/composables/useAccounts.ts`
  - Update dependency type.
  - Reclassify account/import/OAuth/switch messages.
- Modify: `src/composables/useBackups.ts`
  - Update dependency type.
  - Reclassify backup messages.
- Modify: `src/composables/useQuota.ts`
  - Update dependency type.
  - Reclassify quota messages.
- Modify: `src/composables/useUpdater.ts`
  - Update callback type.
  - Reclassify update check messages.
  - Remove the empty `setMessage("")` call.

---

## Task 1: Update App Message API

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: Import `ToastType`**

Change the existing notification import in `src/App.vue` from:

```ts
import { useNotifications } from "./composables/useNotifications";
```

to:

```ts
import { useNotifications, type ToastType } from "./composables/useNotifications";
```

- [ ] **Step 2: Replace `setMessage` signature**

Replace:

```ts
function setMessage(message: string, isError = false) {
  if (isError) {
    notifications.error(message);
    return;
  }
  notifications.success(message);
}
```

with:

```ts
function setMessage(type: ToastType, message: string) {
  notifications[type](message);
}
```

- [ ] **Step 3: Reclassify direct App messages**

In `src/App.vue`, replace these direct calls:

```ts
setMessage(String(err), true);
setMessage("设置已保存。");
setMessage(`设置已保存，但应用信息刷新失败：${String(err)}`, true);
setMessage(String(err), true);
setMessage("已打开目录。");
setMessage(String(err), true);
setMessage(String(err), true);
setMessage(String(err), true);
```

with:

```ts
setMessage("error", String(err));
setMessage("success", "设置已保存。");
setMessage("warning", `设置已保存，但应用信息刷新失败：${String(err)}`);
setMessage("error", String(err));
setMessage("info", "已打开目录。");
setMessage("error", String(err));
setMessage("error", String(err));
setMessage("error", String(err));
```

Expected message semantics:

- Refresh/load failures are `error`.
- Settings saved is `success`.
- Settings saved but app info refresh failed is `warning`.
- Directory opened is `info`.

- [ ] **Step 4: Run frontend build**

```bash
yarn build
```

Expected: build fails at remaining old composable callback types until later tasks are complete, or passes if TypeScript does not check those paths yet. Continue to Task 2 either way.

- [ ] **Step 5: Commit App API change**

If the app builds after Task 1, commit:

```bash
git add src/App.vue
git commit -m "refactor: require typed app messages"
```

If the app does not build because downstream composables still use the old callback type, do not commit yet; continue to Tasks 2-5 and commit all typed-message migration together.

---

## Task 2: Update Account Messages

**Files:**
- Modify: `src/composables/useAccounts.ts`

- [ ] **Step 1: Import `ToastType`**

Add this import near the existing imports:

```ts
import type { ToastType } from "./useNotifications";
```

- [ ] **Step 2: Update dependency type**

Replace:

```ts
setMessage: (message: string, isError?: boolean) => void;
```

with:

```ts
setMessage: (type: ToastType, message: string) => void;
```

- [ ] **Step 3: Reclassify account messages**

Replace all old calls in `src/composables/useAccounts.ts`:

```ts
deps.setMessage(`已导入 ${imported.length} 个账号。`);
deps.setMessage(String(err), true);
deps.setMessage(`已打开 ${modeText} OAuth 登录。Profile: ${result.browser_profile_dir}`);
deps.setMessage(String(err), true);
deps.setMessage("已关闭等待中的 OAuth 登录窗口。");
deps.setMessage(String(err), true);
deps.setMessage(`已刷新 ${updated.display_name} 的认证状态。`);
deps.setMessage(String(err), true);
deps.setMessage(`已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建。${warningText}`);
deps.setMessage(String(err), true);
```

with:

```ts
deps.setMessage("success", `已导入 ${imported.length} 个账号。`);
deps.setMessage("error", String(err));
deps.setMessage("info", `已打开 ${modeText} OAuth 登录。Profile: ${result.browser_profile_dir}`);
deps.setMessage("error", String(err));
deps.setMessage("info", "已关闭等待中的 OAuth 登录窗口。");
deps.setMessage("error", String(err));
deps.setMessage("success", `已刷新 ${updated.display_name} 的认证状态。`);
deps.setMessage("error", String(err));
deps.setMessage(result.warnings.length ? "warning" : "success", `已切换到 ${result.account.display_name}，备份 ${result.backup_id} 已创建。${warningText}`);
deps.setMessage("error", String(err));
```

Expected message semantics:

- Import/refresh successful operations are `success`.
- OAuth window opening/closing are `info`.
- Switch account with warnings is `warning`; without warnings is `success`.
- Exceptions are `error`.

---

## Task 3: Update Backup Messages

**Files:**
- Modify: `src/composables/useBackups.ts`

- [ ] **Step 1: Import `ToastType`**

Add:

```ts
import type { ToastType } from "./useNotifications";
```

- [ ] **Step 2: Update dependency type**

Replace:

```ts
setMessage: (message: string, isError?: boolean) => void;
```

with:

```ts
setMessage: (type: ToastType, message: string) => void;
```

- [ ] **Step 3: Reclassify backup messages**

Replace:

```ts
deps.setMessage(`已创建备份 ${backup.id}。`);
deps.setMessage(String(err), true);
deps.setMessage(`已恢复备份 ${backup.id}。`);
deps.setMessage(String(err), true);
```

with:

```ts
deps.setMessage("success", `已创建备份 ${backup.id}。`);
deps.setMessage("error", String(err));
deps.setMessage("success", `已恢复备份 ${backup.id}。`);
deps.setMessage("error", String(err));
```

---

## Task 4: Update Quota Messages

**Files:**
- Modify: `src/composables/useQuota.ts`

- [ ] **Step 1: Import `ToastType`**

Add:

```ts
import type { ToastType } from "./useNotifications";
```

- [ ] **Step 2: Update dependency type**

Replace:

```ts
setMessage: (message: string, isError?: boolean) => void;
```

with:

```ts
setMessage: (type: ToastType, message: string) => void;
```

- [ ] **Step 3: Reclassify quota messages**

Replace:

```ts
deps.setMessage(`额度状态：${quotaLabel(quota)}。`);
deps.setMessage(String(err), true);
deps.setMessage("请先选择一个账号。", true);
deps.setMessage(`额度状态：${usageLabel(state)}。`);
deps.setMessage(String(err), true);
deps.setMessage("没有可检查的账号。", true);
deps.setMessage(`全部额度检查完成：成功 ${succeeded} 个，失败 ${failed} 个。`, failed > 0);
deps.setMessage("已清除该账号的额度记录。");
deps.setMessage(String(err), true);
```

with:

```ts
deps.setMessage("success", `额度状态：${quotaLabel(quota)}。`);
deps.setMessage("error", String(err));
deps.setMessage("warning", "请先选择一个账号。");
deps.setMessage("success", `额度状态：${usageLabel(state)}。`);
deps.setMessage("error", String(err));
deps.setMessage("warning", "没有可检查的账号。");
deps.setMessage(failed > 0 ? "warning" : "success", `全部额度检查完成：成功 ${succeeded} 个，失败 ${failed} 个。`);
deps.setMessage("success", "已清除该账号的额度记录。");
deps.setMessage("error", String(err));
```

Expected message semantics:

- Successful quota checks and clear operations are `success`.
- Missing selection or no checkable accounts are `warning`.
- Batch partial failures are `warning`.
- Exceptions are `error`.

---

## Task 5: Update Updater Messages

**Files:**
- Modify: `src/composables/useUpdater.ts`

- [ ] **Step 1: Import `ToastType`**

Add:

```ts
import type { ToastType } from "./useNotifications";
```

- [ ] **Step 2: Update callback type**

Change:

```ts
export function useUpdater(settings: Settings, setMessage: (message: string, isError?: boolean) => void) {
```

to:

```ts
export function useUpdater(settings: Settings, setMessage: (type: ToastType, message: string) => void) {
```

- [ ] **Step 3: Reclassify update messages**

Replace:

```ts
if (manual) setMessage("当前已经是最新版本。");
if (manual) setMessage("");
if (manual) setMessage(message, true);
```

with:

```ts
if (manual) setMessage("info", "当前已经是最新版本。");
if (manual) setMessage("error", message);
```

Remove the empty `setMessage("")` call entirely. The branch where an update exists should show the update dialog, not a blank Toast.

Expected message semantics:

- Manual check with no update is `info`.
- Manual check failure is `error`.
- Found update is handled by `UpdateDialog`, not Toast.

---

## Task 6: Enforce No Old Calls Remain

**Files:**
- Verify: `src/App.vue`
- Verify: `src/composables/useAccounts.ts`
- Verify: `src/composables/useBackups.ts`
- Verify: `src/composables/useQuota.ts`
- Verify: `src/composables/useUpdater.ts`

- [ ] **Step 1: Search for old boolean-style calls**

Run:

```bash
rg "setMessage\\([^\"']|setMessage\\([^,]+\\)|setMessage\\([^\\n]+,\\s*true\\)|setMessage\\([^\\n]+,\\s*false\\)" src
```

Expected: no matches for old style calls.

- [ ] **Step 2: Search for empty Toast messages**

Run:

```bash
rg "setMessage\\([^\\n]*\"\"|setMessage\\([^\\n]*''" src
```

Expected: no matches.

- [ ] **Step 3: Search all current message calls**

Run:

```bash
rg "setMessage" src
```

Expected: every call should match this shape:

```ts
setMessage("success", "...");
setMessage("info", "...");
setMessage("warning", "...");
setMessage("error", "...");
```

or a dynamic first argument that evaluates to `ToastType`, such as:

```ts
deps.setMessage(failed > 0 ? "warning" : "success", message);
```

---

## Task 7: Build and Test

**Files:**
- Verify all modified frontend files.

- [ ] **Step 1: Run frontend build**

```bash
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 2: Run Rust tests**

```bash
cargo test --manifest-path src-tauri\Cargo.toml
```

Expected:

```text
test result: ok
```

- [ ] **Step 3: Commit migration**

If Task 1 was not committed independently, commit the full migration:

```bash
git add src/App.vue src/composables/useAccounts.ts src/composables/useBackups.ts src/composables/useQuota.ts src/composables/useUpdater.ts
git commit -m "refactor: use typed toast message levels"
```

If Task 1 was already committed, commit the remaining composable migration:

```bash
git add src/composables/useAccounts.ts src/composables/useBackups.ts src/composables/useQuota.ts src/composables/useUpdater.ts
git commit -m "refactor: classify toast message levels"
```

---

## Self-Review

Spec coverage:

- Three-layer messaging rule documented and scoped.
- Toast-only operation feedback is enforced through typed `setMessage`.
- Page state and dialog responsibilities are explicitly preserved.
- Old boolean-style API is removed without compatibility.
- All known call sites are covered.

Placeholder scan:

- No placeholder implementation steps remain.
- Search commands define exact expected results.

Type consistency:

- `ToastType` is imported from `useNotifications`.
- `setMessage` signature is consistently `(type: ToastType, message: string) => void`.
- Dynamic message types use literals that are valid `ToastType` values.
