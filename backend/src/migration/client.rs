//! MiniCRM REST API (R3) client. Basic auth with SystemId + API key; strictly rate limited,
//! because the extraction runs for hours against a production account.

use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use reqwest::StatusCode;
use serde_json::Value;
use tokio::sync::Mutex;

pub struct MiniCrmClient {
    http: reqwest::Client,
    base: String,
    system_id: String,
    api_key: String,
    min_interval: Duration,
    last_request: Mutex<Option<Instant>>,
}

impl MiniCrmClient {
    pub fn new(
        base: &str,
        system_id: &str,
        api_key: &str,
        requests_per_minute: u32,
    ) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .user_agent("autocrm-migrate/0.1")
            .build()?;
        Ok(MiniCrmClient {
            http,
            base: base.trim_end_matches('/').to_string(),
            system_id: system_id.to_string(),
            api_key: api_key.to_string(),
            min_interval: Duration::from_millis(60_000 / u64::from(requests_per_minute.max(1))),
            last_request: Mutex::new(None),
        })
    }

    async fn pace(&self) {
        let mut last = self.last_request.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        *last = Some(Instant::now());
    }

    /// GET a JSON resource. `Ok(None)` for 404 (resource absent or endpoint unavailable).
    /// Retries rate limiting and server errors with backoff.
    pub async fn get_json(&self, path: &str) -> anyhow::Result<Option<Value>> {
        let url = format!("{}/{}", self.base, path.trim_start_matches('/'));
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            self.pace().await;
            let response = self
                .http
                .get(&url)
                .basic_auth(&self.system_id, Some(&self.api_key))
                .send()
                .await;
            let response = match response {
                Ok(r) => r,
                Err(e) if attempt < 6 => {
                    tracing::warn!(%url, error = %e, attempt, "request failed; retrying");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                    continue;
                }
                Err(e) => return Err(e).with_context(|| format!("GET {url}")),
            };
            let status = response.status();
            if status == StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) && attempt < 6
            {
                let wait = response
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(2u64.pow(attempt) * 5);
                tracing::warn!(%url, %status, wait, "throttled or server error; backing off");
                tokio::time::sleep(Duration::from_secs(wait)).await;
                continue;
            }
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                bail!(
                    "MiniCRM rejected the credentials for {url} ({status}); check SystemId, API key and that the API add-on is active"
                );
            }
            let text = response
                .text()
                .await
                .with_context(|| format!("reading {url}"))?;
            if !status.is_success() {
                bail!(
                    "GET {url}: HTTP {status}: {}",
                    text.chars().take(300).collect::<String>()
                );
            }
            let value = serde_json::from_str(&text)
                .with_context(|| format!("{url} did not return JSON"))?;
            return Ok(Some(value));
        }
    }

    /// Raw bytes (for attached files). Uses the same credentials; MiniCRM file URLs that are
    /// public simply ignore them.
    pub async fn get_bytes(&self, url: &str) -> anyhow::Result<(Vec<u8>, Option<String>)> {
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            let result = self
                .http
                .get(url)
                .basic_auth(&self.system_id, Some(&self.api_key))
                .send()
                .await;
            match result {
                Ok(r) if r.status().is_success() => {
                    let content_type = r
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    let bytes = r.bytes().await.with_context(|| format!("reading {url}"))?;
                    return Ok((bytes.to_vec(), content_type));
                }
                Ok(r)
                    if (r.status() == StatusCode::TOO_MANY_REQUESTS
                        || r.status().is_server_error())
                        && attempt < 6 =>
                {
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt) * 5)).await;
                }
                Ok(r) => bail!("GET {url}: HTTP {}", r.status()),
                Err(e) if attempt < 6 => {
                    tracing::warn!(%url, error = %e, attempt, "download failed; retrying");
                    tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                }
                Err(e) => return Err(e).with_context(|| format!("GET {url}")),
            }
        }
    }
}
