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

export type AppPaths = {
  codex_home: string;
  app_store_dir: string;
  backups_dir: string;
  browser_profile_dir: string;
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
