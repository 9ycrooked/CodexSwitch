use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountSummary {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub expired: Option<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub imported_at: String,
    #[serde(default)]
    pub has_config: bool,
    #[serde(default)]
    pub browser_profile_dir: Option<String>,
    #[serde(default)]
    pub oauth_metadata: Option<OAuthMetadata>,
    #[serde(default)]
    pub quota_state: Option<QuotaState>,
    #[serde(default)]
    pub usage_state: Option<UsageState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSummary {
    pub id: String,
    pub created_at: String,
    pub auth_path: Option<String>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexState {
    pub codex_home: String,
    pub auth_exists: bool,
    pub config_exists: bool,
    pub current_account_id: Option<String>,
    pub current_email: Option<String>,
    pub current_auth_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub codex_home: String,
    pub app_store_dir: String,
    pub backups_dir: String,
    pub browser_profile_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwitchResult {
    pub account: AccountSummary,
    pub backup_id: String,
    pub warnings: Vec<String>,
    pub codex_reopened: bool,
    pub codex_reopen_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBundleExportResult {
    pub path: String,
    pub exported_count: usize,
    pub included_profile_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBundleImportFailure {
    pub id: Option<String>,
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBundleImportResult {
    pub imported: Vec<AccountSummary>,
    pub failed: Vec<AccountBundleImportFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OAuthMetadata {
    pub email: Option<String>,
    pub account_id: Option<String>,
    pub plan_type: Option<String>,
    pub subscription_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthLoginStart {
    pub auth_url: String,
    pub profile_id: String,
    pub browser_profile_dir: String,
    pub callback_port: u16,
    pub state: String,
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowOAuthServerStatus {
    pub running: bool,
    pub port: u16,
    pub url: String,
    pub admin_key_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowGenerateAuthUrlResponse {
    pub auth_url: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoFlowExchangeCodeResponse {
    pub message: String,
    pub id: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaState {
    pub status: String,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub resets_at: Option<String>,
    pub resets_in_seconds: Option<i64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageState {
    pub status: String,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub http_status: Option<u16>,
    pub resets_at: Option<String>,
    pub raw_plan_type: Option<String>,
    #[serde(default)]
    pub windows: Vec<CodexQuotaWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkProbeResult {
    pub name: String,
    pub status: String,
    pub latency_ms: Option<u64>,
    pub http_status: Option<u16>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkExitCheckResult {
    pub overall_status: String,
    pub checked_at: String,
    pub auth_reachable: bool,
    pub auth_status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub backend_ip: Option<String>,
    pub backend_country: Option<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub probes: Vec<NetworkProbeResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodexQuotaWindow {
    pub id: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub reset_at: Option<String>,
    pub reset_label: String,
    pub limit_reached: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    pub id_token: String,
    #[serde(default)]
    pub expires_in: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct StoredAccount {
    pub summary: AccountSummary,
    pub auth_json: Value,
    pub original_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupMeta {
    pub id: String,
    pub created_at: String,
    pub auth_path: Option<String>,
    pub config_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switch_result_serializes_codex_reopen_status() {
        let result = SwitchResult {
            account: AccountSummary {
                id: "acct-1".to_string(),
                display_name: "User".to_string(),
                email: None,
                account_id: None,
                plan: None,
                expired: None,
                disabled: false,
                imported_at: "2026-05-22T00:00:00Z".to_string(),
                has_config: false,
                browser_profile_dir: None,
                oauth_metadata: None,
                quota_state: None,
                usage_state: None,
            },
            backup_id: "backup-1".to_string(),
            warnings: Vec::new(),
            codex_reopened: true,
            codex_reopen_path: Some(
                r"C:\Program Files\WindowsApps\OpenAI.Codex\app\Codex.exe".to_string(),
            ),
        };

        let value = serde_json::to_value(result).expect("switch result should serialize");

        assert_eq!(value["codex_reopened"], true);
        assert_eq!(
            value["codex_reopen_path"],
            r"C:\Program Files\WindowsApps\OpenAI.Codex\app\Codex.exe"
        );
    }
}
