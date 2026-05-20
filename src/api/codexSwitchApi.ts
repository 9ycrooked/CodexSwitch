import { invoke } from "@tauri-apps/api/core";
import type {
  AccountSummary,
  AppPaths,
  AutoFlowOAuthServerStatus,
  BackupSummary,
  CodexState,
  NetworkExitCheckResult,
  QuotaState,
  Settings,
  SwitchResult,
  UsageState
} from "../types";

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

export function updateSettings(settings: Settings) {
  return invoke<Settings>("update_settings", { settings });
}

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

export function importAccounts(paths: string[]) {
  return invoke<AccountSummary[]>("import_accounts", { paths });
}

export function deleteAccount(accountId: string, deleteProfile: boolean) {
  return invoke("delete_account", { accountId, deleteProfile });
}

export function startOauthLogin(profileId: string | null = null) {
  return invoke<{ auth_url: string; browser_profile_dir: string; mode: string }>("start_oauth_login", { profileId });
}

export function checkOauthNetworkExit(includeEgressRegion?: boolean) {
  return invoke<NetworkExitCheckResult>("check_oauth_network_exit", {
    includeEgressRegion: includeEgressRegion ?? null
  });
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
