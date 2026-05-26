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

export type AppPaths = {
  codex_home: string;
  app_store_dir: string;
  backups_dir: string;
  browser_profile_dir: string;
};

export type Settings = {
  codex_home: string;
  process_names: string[];
  browser_profile_dir: string;
  oauth_callback_port: number;
  keep_login_profiles: boolean;
  oauth_login_mode: string;
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
  manual_update_check_only: boolean;
  check_oauth_network_on_login: boolean;
  check_egress_region: boolean;
  autoflow_oauth_server_enabled: boolean;
  autoflow_oauth_server_port: number;
  autoflow_oauth_admin_key: string;
};

export type AutoFlowOAuthServerStatus = {
  running: boolean;
  port: number;
  url: string;
  admin_key_configured: boolean;
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
  codex_reopened: boolean;
  codex_reopen_path?: string | null;
};

export type AccountBundleExportResult = {
  path: string;
  exported_count: number;
  included_profile_count: number;
};

export type AuthJsonExportResult = {
  path: string;
  exported_count: number;
  folder_names: string[];
};

export type AccountBundleImportFailure = {
  id?: string | null;
  path?: string | null;
  message: string;
};

export type AccountBundleImportResult = {
  imported: AccountSummary[];
  failed: AccountBundleImportFailure[];
};

export type UpdatePolicy = {
  check_updates_on_startup: boolean;
  force_update_on_startup: boolean;
  message?: string | null;
};
