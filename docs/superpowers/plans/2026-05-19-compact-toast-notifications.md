# Compact Toast Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current top-of-page `notice/error` banners with a compact right-bottom stacked toast notification system for operation feedback, warnings, and errors.

**Architecture:** Add a small frontend-only notification store composable, render it through a fixed `ToastViewport`, and keep the existing `setMessage(message, isError)` API as a compatibility bridge. Toasts are compact, newest-at-bottom, stacked upward with partial overlap, and expand into a readable list on hover.

**Tech Stack:** Vue 3 Composition API, TypeScript, CSS transitions, existing Tauri 2 + Vite frontend; no new dependencies.

---

## File Structure

- Create: `src/composables/useNotifications.ts`
  - Owns toast state, ids, timers, add/remove helpers, hover pause/resume, queue limit, and semantic helpers.
- Create: `src/components/ToastViewport.vue`
  - Renders the fixed right-bottom viewport, stacked/expanded layout, and passes close events to the store.
- Create: `src/components/ToastItem.vue`
  - Renders one compact toast row with status stripe, message text, close button, accessibility role, and type class.
- Modify: `src/App.vue`
  - Replace `notice/error` refs with notification store usage.
  - Keep `setMessage(message, isError)` so current composables continue to work.
  - Mount `ToastViewport` once near `UpdateDialog`.
- Modify: `src/styles/components.css`
  - Remove or leave unused `.notice` styles after template cleanup.
  - Add toast viewport/item styles, stacked transforms, hover-expanded list, reduced-motion handling.
- Verify only: `src/composables/useAccounts.ts`, `src/composables/useBackups.ts`, `src/composables/useQuota.ts`, `src/composables/useUpdater.ts`
  - These should keep calling `setMessage`; no direct toast dependency should leak into business composables.

---

## Behavior Contract

Toast shape:

```ts
export type ToastType = "success" | "info" | "warning" | "error";

export type ToastMessage = {
  id: string;
  type: ToastType;
  message: string;
  createdAt: number;
  durationMs: number;
  remainingMs: number;
  timerStartedAt: number | null;
};
```

Defaults:

```ts
const DEFAULT_DURATIONS: Record<ToastType, number> = {
  success: 3000,
  info: 3000,
  warning: 5000,
  error: 8000
};

const MAX_TOASTS = 4;
```

Layout:

- Latest toast is index `0` in the rendered display list and sits at the bottom, fully readable.
- Older toasts sit above it with slight right/up offset and partial visibility.
- Hovering the viewport expands all toasts into a normal readable vertical list.
- Each toast has a status stripe, message text, and close button only.
- No title block, no detail body, no action buttons in v1.

---

## Task 1: Add Notification Store

**Files:**
- Create: `src/composables/useNotifications.ts`

- [ ] **Step 1: Create notification types and store**

Create `src/composables/useNotifications.ts` with this complete content:

```ts
import { computed, ref } from "vue";

export type ToastType = "success" | "info" | "warning" | "error";

export type ToastMessage = {
  id: string;
  type: ToastType;
  message: string;
  createdAt: number;
  durationMs: number;
  remainingMs: number;
  timerStartedAt: number | null;
};

const DEFAULT_DURATIONS: Record<ToastType, number> = {
  success: 3000,
  info: 3000,
  warning: 5000,
  error: 8000
};

const MAX_TOASTS = 4;

let nextToastId = 1;

function createId() {
  const id = `toast-${Date.now()}-${nextToastId}`;
  nextToastId += 1;
  return id;
}

export function useNotifications() {
  const toasts = ref<ToastMessage[]>([]);
  const timers = new Map<string, number>();
  const isPaused = ref(false);

  const visibleToasts = computed(() => toasts.value.slice(0, MAX_TOASTS));

  function clearTimer(id: string) {
    const timer = timers.get(id);
    if (timer !== undefined) {
      window.clearTimeout(timer);
      timers.delete(id);
    }
  }

  function remove(id: string) {
    clearTimer(id);
    toasts.value = toasts.value.filter((toast) => toast.id !== id);
  }

  function startTimer(toast: ToastMessage) {
    clearTimer(toast.id);
    if (isPaused.value || toast.remainingMs <= 0) return;
    toast.timerStartedAt = Date.now();
    const timer = window.setTimeout(() => remove(toast.id), toast.remainingMs);
    timers.set(toast.id, timer);
  }

  function trimQueue() {
    const removed = toasts.value.slice(MAX_TOASTS);
    for (const toast of removed) clearTimer(toast.id);
    toasts.value = toasts.value.slice(0, MAX_TOASTS);
  }

  function add(type: ToastType, message: string, durationMs = DEFAULT_DURATIONS[type]) {
    const trimmedMessage = message.trim();
    if (!trimmedMessage) return null;

    const toast: ToastMessage = {
      id: createId(),
      type,
      message: trimmedMessage,
      createdAt: Date.now(),
      durationMs,
      remainingMs: durationMs,
      timerStartedAt: null
    };

    toasts.value = [toast, ...toasts.value];
    trimQueue();
    startTimer(toast);
    return toast.id;
  }

  function pause() {
    if (isPaused.value) return;
    isPaused.value = true;
    const now = Date.now();

    for (const toast of toasts.value) {
      clearTimer(toast.id);
      if (toast.timerStartedAt !== null) {
        toast.remainingMs = Math.max(0, toast.remainingMs - (now - toast.timerStartedAt));
        toast.timerStartedAt = null;
      }
    }
  }

  function resume() {
    if (!isPaused.value) return;
    isPaused.value = false;
    for (const toast of toasts.value) startTimer(toast);
  }

  function clear() {
    for (const toast of toasts.value) clearTimer(toast.id);
    toasts.value = [];
  }

  return {
    toasts: visibleToasts,
    add,
    remove,
    pause,
    resume,
    clear,
    success: (message: string) => add("success", message),
    info: (message: string) => add("info", message),
    warning: (message: string) => add("warning", message),
    error: (message: string) => add("error", message)
  };
}
```

- [ ] **Step 2: Review queue semantics**

Check these points manually in the file:

- `toasts.value = [toast, ...toasts.value]` keeps the newest toast at index `0`.
- `visibleToasts` returns at most 4 toasts.
- `pause()` subtracts elapsed timer time.
- `resume()` restarts timers using remaining time.

- [ ] **Step 3: Commit notification store**

```bash
git add src/composables/useNotifications.ts
git commit -m "feat: add toast notification store"
```

---

## Task 2: Add Toast Components

**Files:**
- Create: `src/components/ToastItem.vue`
- Create: `src/components/ToastViewport.vue`

- [ ] **Step 1: Create compact toast item**

Create `src/components/ToastItem.vue` with this complete content:

```vue
<script setup lang="ts">
import type { ToastMessage } from "../composables/useNotifications";

defineProps<{
  toast: ToastMessage;
  index: number;
}>();

defineEmits<{
  close: [string];
}>();

function ariaRole(type: ToastMessage["type"]) {
  return type === "error" ? "alert" : "status";
}
</script>

<template>
  <article
    :class="['toast-item', 'toast-' + toast.type]"
    :style="{ '--toast-index': index }"
    :role="ariaRole(toast.type)"
    aria-live="polite"
  >
    <span class="toast-stripe" aria-hidden="true"></span>
    <p class="toast-message">{{ toast.message }}</p>
    <button class="toast-close" type="button" aria-label="关闭通知" @click="$emit('close', toast.id)">×</button>
  </article>
</template>
```

- [ ] **Step 2: Create stacked viewport**

Create `src/components/ToastViewport.vue` with this complete content:

```vue
<script setup lang="ts">
import type { ToastMessage } from "../composables/useNotifications";
import ToastItem from "./ToastItem.vue";

defineProps<{
  toasts: ToastMessage[];
}>();

defineEmits<{
  close: [string];
  pause: [];
  resume: [];
}>();
</script>

<template>
  <TransitionGroup
    v-if="toasts.length"
    name="toast-stack"
    tag="section"
    class="toast-viewport"
    aria-label="应用通知"
    @mouseenter="$emit('pause')"
    @mouseleave="$emit('resume')"
  >
    <ToastItem
      v-for="(toast, index) in toasts"
      :key="toast.id"
      :toast="toast"
      :index="index"
      @close="$emit('close', $event)"
    />
  </TransitionGroup>
</template>
```

- [ ] **Step 3: Commit toast components**

```bash
git add src/components/ToastItem.vue src/components/ToastViewport.vue
git commit -m "feat: add compact toast components"
```

---

## Task 3: Wire Toasts Into App

**Files:**
- Modify: `src/App.vue`

- [ ] **Step 1: Import the toast component and composable**

In `src/App.vue`, add these imports with the other imports:

```ts
import ToastViewport from "./components/ToastViewport.vue";
import { useNotifications } from "./composables/useNotifications";
```

- [ ] **Step 2: Replace banner refs with notification store**

In `src/App.vue`, remove:

```ts
const notice = ref("");
const error = ref("");
```

Add after the settings object:

```ts
const notifications = useNotifications();
```

- [ ] **Step 3: Keep `setMessage` as compatibility bridge**

Replace the existing `setMessage` function:

```ts
function setMessage(message: string, isError = false) {
  notice.value = isError ? "" : message;
  error.value = isError ? message : "";
}
```

with:

```ts
function setMessage(message: string, isError = false) {
  if (isError) {
    notifications.error(message);
    return;
  }
  notifications.success(message);
}
```

- [ ] **Step 4: Remove banner template output**

Remove these two template lines from `src/App.vue`:

```vue
<div v-if="notice" class="notice">{{ notice }}</div>
<div v-if="error" class="notice error">{{ error }}</div>
```

- [ ] **Step 5: Mount the toast viewport**

In `src/App.vue`, place this before `<UpdateDialog ...>` so the toast system is mounted once inside the shell:

```vue
<ToastViewport
  :toasts="notifications.toasts.value"
  @close="notifications.remove"
  @pause="notifications.pause"
  @resume="notifications.resume"
/>
```

If Vue template auto-unwrapping rejects `.value` in this context, use:

```vue
<ToastViewport
  :toasts="notifications.toasts"
  @close="notifications.remove"
  @pause="notifications.pause"
  @resume="notifications.resume"
/>
```

Expected result: business composables still call `setMessage`, but the UI now shows right-bottom toasts instead of top banners.

- [ ] **Step 6: Commit App wiring**

```bash
git add src/App.vue
git commit -m "feat: wire toast notifications into app"
```

---

## Task 4: Add Toast Styling and Animation

**Files:**
- Modify: `src/styles/components.css`

- [ ] **Step 1: Add fixed viewport and stacked item styles**

Append this CSS to `src/styles/components.css`:

```css
.toast-viewport {
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 70;
  width: min(340px, calc(100vw - 32px));
  min-height: 46px;
  pointer-events: none;
}

.toast-item {
  position: absolute;
  right: 0;
  bottom: 0;
  display: grid;
  grid-template-columns: 3px minmax(0, 1fr) 28px;
  align-items: center;
  gap: 10px;
  width: 100%;
  min-height: 44px;
  padding: 8px 8px 8px 0;
  border: 1px solid color-mix(in srgb, var(--border-color) 80%, transparent);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  background: color-mix(in srgb, var(--bg-primary) 94%, transparent);
  box-shadow: var(--shadow-md);
  opacity: calc(1 - (var(--toast-index) * 0.16));
  pointer-events: auto;
  transform:
    translate(
      calc(var(--toast-index) * 8px),
      calc(var(--toast-index) * -10px)
    )
    scale(calc(1 - (var(--toast-index) * 0.015)));
  transition:
    transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
    opacity 160ms ease,
    border-color 160ms ease,
    background 160ms ease;
}

.toast-viewport:hover .toast-item {
  opacity: 1;
  transform: translateY(calc(var(--toast-index) * -54px)) scale(1);
}

.toast-stripe {
  align-self: stretch;
  border-radius: var(--radius-md) 0 0 var(--radius-md);
  background: var(--text-tertiary);
}

.toast-success .toast-stripe {
  background: var(--success-color);
}

.toast-info .toast-stripe {
  background: var(--primary-color);
}

.toast-warning .toast-stripe {
  background: var(--warning-color);
}

.toast-error .toast-stripe {
  background: var(--danger-color);
}

.toast-message {
  min-width: 0;
  color: var(--text-primary);
  font-size: 13px;
  font-weight: 650;
  line-height: 1.35;
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.toast-close {
  width: 28px;
  min-width: 28px;
  min-height: 28px;
  padding: 0;
  border: 0;
  border-radius: 7px;
  color: var(--text-secondary);
  background: transparent;
  font-size: 18px;
  line-height: 1;
}

.toast-close:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.toast-stack-enter-active {
  transition:
    opacity 180ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.toast-stack-leave-active {
  transition:
    opacity 140ms ease-in,
    transform 140ms ease-in;
}

.toast-stack-enter-from {
  opacity: 0;
  transform: translate(18px, 10px) scale(0.98);
}

.toast-stack-leave-to {
  opacity: 0;
  transform: translateX(18px) scale(0.98);
}

@media (prefers-reduced-motion: reduce) {
  .toast-item,
  .toast-stack-enter-active,
  .toast-stack-leave-active {
    transition: opacity 80ms ease;
  }

  .toast-stack-enter-from,
  .toast-stack-leave-to {
    transform: none;
  }
}
```

- [ ] **Step 2: Check CSS token names**

Run:

```bash
yarn build
```

Expected: build fails if any CSS variable is not defined by the project.

If the build or visual check reveals that `--shadow-md`, `--success-color`, `--primary-color`, `--warning-color`, or `--danger-color` does not exist, replace only missing variables with existing equivalents from `src/styles/tokens.css`:

```css
--shadow-md        -> --shadow-lg
--success-color    -> --success-badge-text
--primary-color    -> --text-secondary
--warning-color    -> --warning-text
--danger-color     -> --failure-badge-text
```

- [ ] **Step 3: Commit toast styling**

```bash
git add src/styles/components.css
git commit -m "style: add stacked toast animations"
```

---

## Task 5: Verify Integration and UX

**Files:**
- Verify: `src/App.vue`
- Verify: `src/components/ToastViewport.vue`
- Verify: `src/components/ToastItem.vue`
- Verify: `src/composables/useNotifications.ts`
- Verify: `src/styles/components.css`

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

Expected: all existing tests pass. This feature is frontend-only, so failures likely indicate unrelated environment or existing backend issues.

- [ ] **Step 3: Run app manually**

```bash
yarn tauri dev
```

Manual checks:

- Click refresh: right-bottom toast appears.
- Save settings: success toast appears.
- Trigger an operation error, such as invalid Codex home path then save: error toast appears.
- Add several notifications quickly: latest notification stays at the bottom and full width; older notifications stack upward with partial visibility.
- Hover the toast area: notifications expand into a readable vertical list.
- Move the pointer away: notifications collapse into stacked form.
- Click `×`: that toast disappears and remaining toasts reposition smoothly.

- [ ] **Step 4: Check small window behavior**

Resize app to a narrow window around 375px wide.

Expected:

- Toast width never overflows horizontally.
- Toast remains below the titlebar and does not cover window controls.
- Toast text clamps to two lines.

- [ ] **Step 5: Commit verification-only fixes if needed**

If verification finds small style or binding issues, fix only the toast-related files and commit:

```bash
git add src/App.vue src/components/ToastViewport.vue src/components/ToastItem.vue src/composables/useNotifications.ts src/styles/components.css
git commit -m "fix: polish toast notification behavior"
```

---

## Task 6: Optional Cleanup After Confirmation

**Files:**
- Modify: `src/styles/components.css`

- [ ] **Step 1: Remove unused banner styles after confirming no `notice` template remains**

Search:

```bash
rg "notice" src
```

Expected after App wiring:

```text
src/styles/components.css
```

If only `.notice` CSS remains and there are no templates using it, remove:

```css
.notice {
  border: 1px solid var(--success-badge-border);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  font-size: 13px;
  color: var(--success-badge-text);
  background: var(--success-badge-bg);
  animation: fade-in-up 240ms ease both;
}

.notice.error {
  border-color: var(--failure-badge-border);
  color: var(--failure-badge-text);
  background: var(--failure-badge-bg);
}
```

- [ ] **Step 2: Run frontend build**

```bash
yarn build
```

Expected:

```text
✓ built
```

- [ ] **Step 3: Commit cleanup**

```bash
git add src/styles/components.css
git commit -m "chore: remove legacy notice banner styles"
```

---

## Self-Review

Spec coverage:

- Compact notification shape: covered by Task 2 and Task 4.
- Right-bottom placement: covered by Task 4.
- Newest at bottom, old messages stacked upward: covered by Task 4 transform rules.
- Hover expansion: covered by Task 4 `.toast-viewport:hover .toast-item`.
- Auto-dismiss and hover pause: covered by Task 1.
- Close button: covered by Task 2.
- Compatibility with current business code: covered by Task 3.
- No new dependencies: all tasks use Vue + CSS only.

Placeholder scan:

- No `TBD`, `TODO`, or undefined future work is required for implementation.
- Task 4 includes exact fallback replacements for missing CSS tokens.

Type consistency:

- `ToastMessage`, `ToastType`, `useNotifications`, `ToastViewport`, and `ToastItem` names are consistent across tasks.
- Events are consistently named `close`, `pause`, and `resume`.
