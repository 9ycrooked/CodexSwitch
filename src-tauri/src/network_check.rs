use std::time::{Duration, Instant};

use crate::accounts::now_string;
use crate::error::{run_blocking, AppResult};
use crate::models::{NetworkExitCheckResult, NetworkProbeResult};
use crate::settings::load_settings;

const CLOUDFLARE_TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const OPENAI_AUTH_METADATA_URL: &str = "https://auth.openai.com/.well-known/openid-configuration";

struct ProbeResponse {
    result: NetworkProbeResult,
    body: String,
}

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

fn probe_response_result(
    name: &str,
    status_code: u16,
    text: &str,
    latency_ms: u64,
    require_success: bool,
) -> NetworkProbeResult {
    let is_success = (200..300).contains(&status_code);
    let reachable = if require_success {
        is_success
    } else {
        (200..500).contains(&status_code)
    };
    let detail = if reachable || text.trim().is_empty() {
        format!("HTTP {status_code}")
    } else {
        format!("HTTP {status_code}: {text}")
    };

    NetworkProbeResult {
        name: name.to_string(),
        status: if reachable { "ok" } else { "failed" }.to_string(),
        latency_ms: Some(latency_ms),
        http_status: Some(status_code),
        detail: Some(detail),
    }
}

fn probe_get(
    client: &reqwest::blocking::Client,
    name: &str,
    url: &str,
    require_success: bool,
) -> ProbeResponse {
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
            ProbeResponse {
                result: probe_response_result(
                    name,
                    status_code,
                    &text,
                    elapsed_ms,
                    require_success,
                ),
                body: text,
            }
        }
        Err(err) => ProbeResponse {
            result: NetworkProbeResult {
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
            body: String::new(),
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

    let auth_probe = probe_get(&client, "OpenAI OAuth", OPENAI_AUTH_METADATA_URL, true).result;
    result.auth_reachable = auth_probe.status == "ok";
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
        let trace_response = probe_get(&client, "Cloudflare trace", CLOUDFLARE_TRACE_URL, false);
        let trace_probe = trace_response.result;
        if trace_probe.status == "ok" {
            let (ip, country) = parse_cloudflare_trace(&trace_response.body);
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

    #[test]
    fn cloudflare_probe_detail_is_sanitized_without_losing_parseable_trace() {
        let trace = "fl=abc\nip=203.0.113.10\nloc=US\nwarp=off\n";
        let probe = probe_response_result("Cloudflare trace", 200, trace, 42, false);

        assert_eq!(probe.status, "ok");
        assert_eq!(probe.http_status, Some(200));
        assert_eq!(probe.detail.as_deref(), Some("HTTP 200"));

        let (ip, country) = parse_cloudflare_trace(trace);
        assert_eq!(ip.as_deref(), Some("203.0.113.10"));
        assert_eq!(country.as_deref(), Some("US"));
    }

    #[test]
    fn oauth_metadata_probe_requires_success_status() {
        let redirect = probe_response_result("OpenAI OAuth", 302, "", 42, true);
        let client_error = probe_response_result("OpenAI OAuth", 404, "not found", 42, true);
        let success = probe_response_result("OpenAI OAuth", 200, "{}", 42, true);

        assert_eq!(redirect.status, "failed");
        assert_eq!(redirect.detail.as_deref(), Some("HTTP 302"));
        assert_eq!(client_error.status, "failed");
        assert_eq!(client_error.detail.as_deref(), Some("HTTP 404: not found"));
        assert_eq!(success.status, "ok");
        assert_eq!(success.detail.as_deref(), Some("HTTP 200"));
    }
}
