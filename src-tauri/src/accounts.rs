use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::Path;

use uuid::Uuid;

use crate::error::{stringify_io, AppResult};
use crate::io::{atomic_write_json, read_json};
use crate::models::{AccountSummary, OAuthMetadata, StoredAccount, TokenResponse};
use crate::paths::{account_dir, app_store_dir};

pub(crate) fn normalize_auth_json(raw: &Value) -> AppResult<(Value, AccountSummary)> {
    if raw.get("auth_mode").is_some() && raw.get("tokens").is_some() {
        let tokens = raw
            .get("tokens")
            .and_then(Value::as_object)
            .ok_or_else(|| "auth.json 的 tokens 必须是对象。".to_string())?;
        for key in ["access_token", "id_token", "refresh_token", "account_id"] {
            if !tokens.contains_key(key) {
                return Err(format!("auth.json 缺少 tokens.{key}。"));
            }
        }
        let account_id = tokens
            .get("account_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let email = extract_email(raw).or_else(|| {
            tokens
                .get("id_token")
                .and_then(Value::as_str)
                .and_then(parse_jwt_claims)
                .and_then(|claims| extract_email(&claims))
        });
        let oauth_metadata = oauth_metadata_from_auth_json(raw);
        return Ok((
            raw.clone(),
            AccountSummary {
                id: String::new(),
                display_name: email
                    .clone()
                    .or_else(|| account_id.clone())
                    .unwrap_or_default(),
                email,
                account_id,
                plan: None,
                expired: None,
                disabled: false,
                imported_at: String::new(),
                has_config: false,
                browser_profile_dir: None,
                oauth_metadata,
                quota_state: None,
                usage_state: None,
            },
        ));
    }

    for key in ["access_token", "id_token", "refresh_token", "account_id"] {
        if !raw.get(key).is_some_and(|value| value.is_string()) {
            return Err(format!("OAuth JSON 缺少字符串字段：{key}。"));
        }
    }

    let mut tokens = Map::new();
    for key in ["access_token", "id_token", "refresh_token", "account_id"] {
        tokens.insert(key.to_string(), raw[key].clone());
    }

    let last_refresh = raw
        .get("last_refresh")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(now_string);

    let auth_json = json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": Value::Object(tokens),
        "last_refresh": last_refresh
    });

    let id_claims = raw
        .get("id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims);
    let email = raw
        .get("email")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| id_claims.as_ref().and_then(extract_email));
    let account_id = raw
        .get("account_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| id_claims.as_ref().and_then(extract_account_id));
    let plan = raw
        .get("https://api.openai.com/auth")
        .and_then(|auth| auth.get("chatgpt_plan_type"))
        .and_then(Value::as_str)
        .or_else(|| {
            id_claims
                .as_ref()
                .and_then(|claims| claims.get("https://api.openai.com/auth"))
                .and_then(|auth| auth.get("chatgpt_plan_type"))
                .and_then(Value::as_str)
        })
        .or_else(|| raw.get("type").and_then(Value::as_str))
        .map(ToOwned::to_owned);
    let oauth_metadata = oauth_metadata_from_flat(raw)
        .or_else(|| id_claims.as_ref().and_then(oauth_metadata_from_flat));

    Ok((
        auth_json,
        AccountSummary {
            id: String::new(),
            display_name: email
                .clone()
                .or_else(|| account_id.clone())
                .unwrap_or_default(),
            email,
            account_id,
            plan,
            expired: raw
                .get("expired")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            disabled: raw
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            imported_at: String::new(),
            has_config: false,
            browser_profile_dir: None,
            oauth_metadata,
            quota_state: None,
            usage_state: None,
        },
    ))
}

pub(crate) fn summary_from_auth_json(
    auth_json: &Value,
    previous: Option<AccountSummary>,
) -> AccountSummary {
    let previous = previous.unwrap_or_else(|| AccountSummary {
        id: String::new(),
        display_name: String::new(),
        email: None,
        account_id: None,
        plan: None,
        expired: None,
        disabled: false,
        imported_at: now_string(),
        has_config: false,
        browser_profile_dir: None,
        oauth_metadata: None,
        quota_state: None,
        usage_state: None,
    });
    let claims = auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims);
    let email = claims
        .as_ref()
        .and_then(extract_email)
        .or(previous.email.clone());
    let account_id = auth_json
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| claims.as_ref().and_then(extract_account_id))
        .or(previous.account_id.clone());
    let oauth_metadata = oauth_metadata_from_auth_json(auth_json)
        .or_else(|| claims.as_ref().and_then(oauth_metadata_from_flat))
        .or(previous.oauth_metadata.clone());
    let plan = oauth_metadata
        .as_ref()
        .and_then(|meta| meta.plan_type.clone())
        .or(previous.plan.clone());
    let expired = previous
        .expired
        .or_else(|| {
            auth_json
                .get("expires_at")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            claims
                .as_ref()
                .and_then(|value| value.get("exp"))
                .and_then(Value::as_i64)
                .and_then(|exp| DateTime::<Utc>::from_timestamp(exp, 0))
                .map(|value| value.to_rfc3339())
        });
    let id = previous.id.if_empty(
        account_id
            .clone()
            .or(email.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string()),
    );
    let display_name = email
        .clone()
        .or(account_id.clone())
        .unwrap_or_else(|| "Codex account".to_string());
    AccountSummary {
        id: sanitize_id(&id),
        display_name,
        email,
        account_id,
        plan,
        expired,
        disabled: false,
        imported_at: previous.imported_at.if_empty(now_string()),
        has_config: previous.has_config,
        browser_profile_dir: previous.browser_profile_dir,
        oauth_metadata,
        quota_state: previous.quota_state,
        usage_state: previous.usage_state,
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}

pub(crate) fn save_account_record(
    summary: &AccountSummary,
    auth_json: &Value,
    original_json: &Value,
) -> AppResult<()> {
    let accounts_dir = app_store_dir()?.join("accounts");
    save_account_record_to_dir(&accounts_dir, summary, auth_json, original_json)
}

pub(crate) fn save_account_record_to_dir(
    accounts_dir: &Path,
    summary: &AccountSummary,
    auth_json: &Value,
    original_json: &Value,
) -> AppResult<()> {
    let dir = accounts_dir.join(sanitize_id(&summary.id));
    fs::create_dir_all(&dir).map_err(stringify_io)?;
    atomic_write_json(&dir.join("metadata.json"), summary)?;
    atomic_write_json(&dir.join("auth.json"), auth_json)?;
    atomic_write_json(&dir.join("original.json"), original_json)?;
    Ok(())
}

pub(crate) fn auth_json_from_token_response(token: &TokenResponse) -> Value {
    let claims = parse_jwt_claims(&token.id_token);
    let account_id = claims
        .as_ref()
        .and_then(extract_account_id)
        .unwrap_or_default();
    let expires_at = if token.expires_in > 0 {
        DateTime::<Utc>::from_timestamp(Utc::now().timestamp() + token.expires_in, 0)
            .map(|value| value.to_rfc3339())
    } else {
        None
    };
    json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": null,
        "tokens": {
            "access_token": token.access_token,
            "id_token": token.id_token,
            "refresh_token": token.refresh_token,
            "account_id": account_id
        },
        "last_refresh": now_string(),
        "expires_at": expires_at
    })
}

#[tauri::command]
pub fn list_accounts() -> AppResult<Vec<AccountSummary>> {
    let accounts_dir = app_store_dir()?.join("accounts");
    if !accounts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut accounts = Vec::new();
    for entry in fs::read_dir(accounts_dir).map_err(stringify_io)? {
        let entry = entry.map_err(stringify_io)?;
        if !entry.file_type().map_err(stringify_io)?.is_dir() {
            continue;
        }
        let meta_path = entry.path().join("metadata.json");
        if meta_path.exists() {
            let summary: AccountSummary = read_json(&meta_path)?;
            accounts.push(summary);
        }
    }
    accounts.sort_by(|a, b| b.imported_at.cmp(&a.imported_at));
    Ok(accounts)
}

pub(crate) fn load_account(id: &str) -> AppResult<StoredAccount> {
    let dir = account_dir(id)?;
    if !dir.exists() {
        return Err(format!("账号不存在：{id}"));
    }
    Ok(StoredAccount {
        summary: read_json(&dir.join("metadata.json"))?,
        auth_json: read_json(&dir.join("auth.json"))?,
        original_json: read_json(&dir.join("original.json"))?,
    })
}

fn extract_account_id(value: &Value) -> Option<String> {
    value
        .pointer("/tokens/account_id")
        .and_then(Value::as_str)
        .or_else(|| value.get("account_id").and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

fn extract_email(value: &Value) -> Option<String> {
    value
        .get("email")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("https://api.openai.com/profile")
                .and_then(|profile| profile.get("email"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned)
}

pub(crate) fn current_identity_from_auth(auth_json: &Value) -> OAuthMetadata {
    let claims = auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .or_else(|| auth_json.get("id_token").and_then(Value::as_str))
        .and_then(parse_jwt_claims);
    OAuthMetadata {
        email: extract_email(auth_json).or_else(|| claims.as_ref().and_then(extract_email)),
        account_id: extract_account_id(auth_json).or_else(|| {
            claims
                .as_ref()
                .and_then(oauth_metadata_from_flat)
                .and_then(|metadata| metadata.account_id)
        }),
        plan_type: claims
            .as_ref()
            .and_then(oauth_metadata_from_flat)
            .and_then(|metadata| metadata.plan_type),
        subscription_until: claims
            .as_ref()
            .and_then(oauth_metadata_from_flat)
            .and_then(|metadata| metadata.subscription_until),
    }
}

fn parse_jwt_claims(token: &str) -> Option<Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn oauth_metadata_from_auth_json(auth_json: &Value) -> Option<OAuthMetadata> {
    auth_json
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .and_then(parse_jwt_claims)
        .as_ref()
        .and_then(oauth_metadata_from_flat)
}

fn oauth_metadata_from_flat(value: &Value) -> Option<OAuthMetadata> {
    let auth = value.get("https://api.openai.com/auth");
    let profile = value.get("https://api.openai.com/profile");
    let metadata = OAuthMetadata {
        email: value
            .get("email")
            .and_then(Value::as_str)
            .or_else(|| {
                profile
                    .and_then(|profile| profile.get("email"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        account_id: value
            .get("account_id")
            .and_then(Value::as_str)
            .or_else(|| {
                auth.and_then(|auth| auth.get("chatgpt_account_id"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        plan_type: value
            .get("type")
            .and_then(Value::as_str)
            .or_else(|| {
                auth.and_then(|auth| auth.get("chatgpt_plan_type"))
                    .and_then(Value::as_str)
            })
            .map(ToOwned::to_owned),
        subscription_until: auth
            .and_then(|auth| auth.get("chatgpt_subscription_active_until"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    };
    if metadata.email.is_none()
        && metadata.account_id.is_none()
        && metadata.plan_type.is_none()
        && metadata.subscription_until.is_none()
    {
        None
    } else {
        Some(metadata)
    }
}

pub(crate) fn now_string() -> String {
    Utc::now().to_rfc3339()
}

pub(crate) fn sanitize_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    if out.trim_matches('_').is_empty() {
        Uuid::new_v4().to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_flat_oauth_json() {
        let raw = json!({
            "access_token": "access",
            "id_token": "id",
            "refresh_token": "refresh",
            "account_id": "acct-1",
            "email": "user@example.com",
            "expired": "2026-05-26T17:13:51+08:00",
            "disabled": false,
            "type": "codex"
        });

        let (auth, summary) = normalize_auth_json(&raw).unwrap();
        assert_eq!(auth["auth_mode"], "chatgpt");
        assert_eq!(auth["OPENAI_API_KEY"], Value::Null);
        assert_eq!(auth["tokens"]["account_id"], "acct-1");
        assert_eq!(summary.email.as_deref(), Some("user@example.com"));
        assert_eq!(summary.plan.as_deref(), Some("codex"));
    }

    #[test]
    fn preserves_wrapped_auth_json() {
        let raw = json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "access_token": "access",
                "id_token": "id",
                "refresh_token": "refresh",
                "account_id": "acct-2"
            },
            "last_refresh": "2026-05-17T00:00:00Z"
        });

        let (auth, summary) = normalize_auth_json(&raw).unwrap();
        assert_eq!(auth, raw);
        assert_eq!(summary.account_id.as_deref(), Some("acct-2"));
    }

    #[test]
    fn rejects_missing_tokens() {
        let raw = json!({
            "access_token": "access",
            "id_token": "id",
            "account_id": "acct-1"
        });
        assert!(normalize_auth_json(&raw).is_err());
    }

    #[test]
    fn parses_jwt_oauth_metadata() {
        let claims = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-123",
                "chatgpt_plan_type": "plus",
                "chatgpt_subscription_active_until": "2026-06-16T09:12:24+00:00"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let parsed = parse_jwt_claims(&token).unwrap();
        let meta = oauth_metadata_from_flat(&parsed).unwrap();
        assert_eq!(meta.email.as_deref(), Some("user@example.com"));
        assert_eq!(meta.account_id.as_deref(), Some("acct-123"));
        assert_eq!(meta.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn parses_current_identity_from_wrapped_auth_id_token() {
        let claims = json!({
            "email": "current@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-from-claims",
                "chatgpt_plan_type": "plus"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access",
                "id_token": token,
                "refresh_token": "refresh",
                "account_id": "acct-from-token"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert_eq!(identity.email.as_deref(), Some("current@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("acct-from-token"));
        assert_eq!(identity.plan_type.as_deref(), Some("plus"));
    }

    #[test]
    fn falls_back_to_claim_account_id_for_current_identity() {
        let claims = json!({
            "email": "fallback@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "acct-from-claims"
            }
        });
        let token = format!(
            "header.{}.sig",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap())
        );
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "access_token": "access",
                "id_token": token,
                "refresh_token": "refresh"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert_eq!(identity.email.as_deref(), Some("fallback@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("acct-from-claims"));
    }

    #[test]
    fn ignores_invalid_current_identity_token() {
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "not-a-jwt"
            }
        });
        let identity = current_identity_from_auth(&auth_json);
        assert!(identity.email.is_none());
        assert!(identity.account_id.is_none());
    }
}
