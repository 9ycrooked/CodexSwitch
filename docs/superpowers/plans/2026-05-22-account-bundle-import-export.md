# Account Bundle Import Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add ZIP-based account import/export with bound credentials and isolated login profiles.

**Architecture:** Add a focused Rust `account_bundle` module for ZIP format, validation, and profile copy logic. Add thin Tauri commands in `commands.rs`, then wire Vue API/composable/UI controls to call those commands.

**Tech Stack:** Tauri 2, Rust, `zip`, `walkdir`, `sha2`, Vue 3, TypeScript, Tauri dialog plugin.

---

### Task 1: Backend Bundle Module

**Files:**
- Create: `src-tauri/src/account_bundle.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

- [ ] Write failing tests for bundle path validation, auth hash validation, and profile file round-trip.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml account_bundle` and confirm failures.
- [ ] Implement ZIP manifest structs, export, import, path validation, SHA-256, and profile extraction.
- [ ] Run the account bundle tests and confirm they pass.

### Task 2: Tauri Commands

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/models.rs`

- [ ] Add `export_account_bundle`, `import_account_bundle`, and `relogin_account` command surfaces.
- [ ] Keep old `.json` / `.toml` import unsupported.
- [ ] Run Rust tests for commands and bundle behavior.

### Task 3: Frontend API And State

**Files:**
- Modify: `src/types.ts`
- Modify: `src/api/codexSwitchApi.ts`
- Modify: `src/composables/useAccounts.ts`

- [ ] Add TypeScript types for export/import results.
- [ ] Add API wrappers for export/import/relogin.
- [ ] Replace file picker filters with ZIP-only import and add drag-drop path handling.

### Task 4: Account UI

**Files:**
- Modify: `src/App.vue`
- Modify: `src/views/AccountsView.vue`
- Modify: `src/styles/views.css`

- [ ] Add export modal with account multi-select.
- [ ] Add ZIP drop zone on the account page.
- [ ] Add per-account relogin button.
- [ ] Build and visually check the account page.

### Task 5: Verification

**Files:**
- Modify: `README.md`

- [ ] Document the bundle format and sensitive-data warning.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run `yarn build`.
