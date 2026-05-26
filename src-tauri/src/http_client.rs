use std::time::Duration;

use crate::error::AppResult;
use crate::settings::load_settings;

pub(crate) fn backend_client(timeout: Duration) -> AppResult<reqwest::blocking::Client> {
    let settings = load_settings()?;
    backend_client_builder(&settings.backend_proxy_url, timeout)?
        .build()
        .map_err(|err| err.to_string())
}

pub(crate) fn backend_client_builder(
    backend_proxy_url: &str,
    timeout: Duration,
) -> AppResult<reqwest::blocking::ClientBuilder> {
    let mut builder = reqwest::blocking::Client::builder().timeout(timeout);
    let proxy = backend_proxy_url.trim();
    if !proxy.is_empty() {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy)
                .map_err(|err| format!("后端代理地址无效：{err}"))?,
        );
    }
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_client_rejects_invalid_proxy_url() {
        let err = backend_client_builder("not a url", Duration::from_secs(1))
            .expect_err("invalid proxy URL should be rejected");

        assert!(err.contains("后端代理地址无效"));
    }
}
