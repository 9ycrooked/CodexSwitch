use std::time::{Duration, Instant};

use crate::accounts::now_string;
use crate::error::{run_blocking, AppResult};
use crate::models::{NetworkExitCheckResult, NetworkProbeResult};
use crate::settings::load_settings;

const CLOUDFLARE_TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const OPENAI_AUTH_METADATA_URL: &str = "https://auth.openai.com/.well-known/openid-configuration";

fn parse_cloudflare_trace(text: &str) -> (Option<String>, Option<String>) {
    let mut ip = None;
    let mut country = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim() {
            "ip" => ip = Some(value.to_string()),
            "loc" => country = Some(value.to_string()),
            _ => {}
        }
    }
    (ip, country)
}

fn classify_result(result: &mut NetworkExitCheckResult) {
    result.overall_status = if !result.errors.is_empty() {
        "failed"
    } else if !result.warnings.is_empty() {
        "warning"
    } else {
        "ok"
    }
    .to_string();
}

fn probe_get(client: &reqwest::blocking::Client, name: &str, url: &str) -> NetworkProbeResult {
    let started_at = Instant::now();
    match client.get(url).send() {
        Ok(response) => {
            let elapsed_ms = started_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX);
            let status = response.status();
            let status_code = status.as_u16();
            let text = response.text().unwrap_or_default();
            let reachable =
                status.is_success() || status.is_redirection() || status.is_client_error();
            let detail = if url == CLOUDFLARE_TRACE_URL && reachable {
                text
            } else if reachable {
                format!("HTTP {status_code}")
            } else if text.trim().is_empty() {
                format!("HTTP {status_code}")
            } else {
                format!("HTTP {status_code}: {text}")
            };
            NetworkProbeResult {
                name: name.to_string(),
                status: if reachable { "reachable" } else { "failed" }.to_string(),
                latency_ms: Some(elapsed_ms),
                http_status: Some(status_code),
                detail: Some(detail),
            }
        }
        Err(err) => NetworkProbeResult {
            name: name.to_string(),
            status: "failed".to_string(),
            latency_ms: Some(
                started_at
                    .elapsed()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
            ),
            http_status: None,
            detail: Some(err.to_string()),
        },
    }
}

#[tauri::command]
pub async fn check_oauth_network_exit(
    include_egress_region: Option<bool>,
) -> AppResult<NetworkExitCheckResult> {
    run_blocking(move || check_oauth_network_exit_blocking(include_egress_region)).await
}

fn check_oauth_network_exit_blocking(
    include_egress_region: Option<bool>,
) -> AppResult<NetworkExitCheckResult> {
    let settings = load_settings()?;
    let include_egress_region = include_egress_region.unwrap_or(settings.check_egress_region);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| err.to_string())?;
    let mut result = NetworkExitCheckResult {
        checked_at: now_string(),
        ..NetworkExitCheckResult::default()
    };

    let auth_probe = probe_get(&client, "OpenAI OAuth", OPENAI_AUTH_METADATA_URL);
    result.auth_reachable = auth_probe.status == "reachable";
    result.auth_status = auth_probe.http_status;
    result.latency_ms = auth_probe.latency_ms;
    if !result.auth_reachable {
        let detail = auth_probe
            .detail
            .clone()
            .unwrap_or_else(|| "unknown error".to_string());
        result
            .errors
            .push(format!("后端无法访问 OpenAI OAuth 服务：{detail}"));
    }
    result.probes.push(auth_probe);

    if include_egress_region {
        let trace_probe = probe_get(&client, "Cloudflare trace", CLOUDFLARE_TRACE_URL);
        if trace_probe.status == "reachable" {
            let (ip, country) =
                parse_cloudflare_trace(trace_probe.detail.as_deref().unwrap_or_default());
            result.backend_ip = ip;
            result.backend_country = country;
            if result.backend_country.is_none() {
                result
                    .warnings
                    .push("Cloudflare trace 可访问，但未解析到出口国家代码。".to_string());
            }
        } else {
            let detail = trace_probe
                .detail
                .clone()
                .unwrap_or_else(|| "unknown error".to_string());
            result.warnings.push(format!("出口地区查询失败：{detail}"));
        }
        result.probes.push(trace_probe);
    }

    classify_result(&mut result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cloudflare_trace_ip_and_country() {
        let (ip, country) = parse_cloudflare_trace("fl=abc\nip=203.0.113.10\nloc=US\nwarp=off\n");

        assert_eq!(ip.as_deref(), Some("203.0.113.10"));
        assert_eq!(country.as_deref(), Some("US"));
    }

    #[test]
    fn parses_cloudflare_trace_missing_values_safely() {
        let (ip, country) = parse_cloudflare_trace("fl=abc\nip=\nloc=\n");

        assert_eq!(ip, None);
        assert_eq!(country, None);
    }

    #[test]
    fn classifies_network_check_status() {
        let mut result = NetworkExitCheckResult::default();
        classify_result(&mut result);
        assert_eq!(result.overall_status, "ok");

        result.warnings.push("slow".to_string());
        classify_result(&mut result);
        assert_eq!(result.overall_status, "warning");

        result.errors.push("down".to_string());
        classify_result(&mut result);
        assert_eq!(result.overall_status, "failed");
    }
}
