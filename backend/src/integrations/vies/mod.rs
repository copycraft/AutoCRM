//! EU VIES VAT-number validation (REST API). All calls are server-side.
//!
//! "VIES couldn't answer" is `Unavailable`, never `Invalid`. Retries, the per-process and
//! per-member-state concurrency caps, and in-flight dedupe live here; persistence and
//! caching are the caller's.

pub mod address;
pub mod normalize;
pub mod parse;

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

pub use address::{AddressParts, split_address};
pub use normalize::{FormatError, NormalizedVat, normalize_vat};
use parse::{Reply, parse_check_reply};

pub const DEFAULT_BASE_URL: &str = "https://ec.europa.eu/taxation_customs/vies/rest-api";
pub const HINT_REQUESTER_REJECTED: &str = "REQUESTER_REJECTED";

const STATUS_TTL: Duration = Duration::from_secs(5 * 60);
const INTERACTIVE_BUDGET: Duration = Duration::from_secs(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VatCheckStatus {
    Valid,
    Invalid,
    Unavailable,
    Error,
    FormatInvalid,
}

impl VatCheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            VatCheckStatus::Valid => "VALID",
            VatCheckStatus::Invalid => "INVALID",
            VatCheckStatus::Unavailable => "UNAVAILABLE",
            VatCheckStatus::Error => "ERROR",
            VatCheckStatus::FormatInvalid => "FORMAT_INVALID",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ViesConfig {
    pub base_url: String,
    /// Routes checks to `/check-vat-test-service` (canned answers by number).
    pub use_test_service: bool,
    /// Our own VAT number; with it VIES returns a consultation number.
    pub requester_country: String,
    pub requester_number: Option<String>,
    pub http_timeout: Duration,
    pub max_attempts: u32,
    /// First backoff step (1 s); doubles per retry, plus up to half of it as jitter.
    pub backoff_base: Duration,
    /// Bulk jobs (re-checks, backfill) leave at least this long between calls (500 ms).
    pub bulk_interval: Duration,
}

impl Default for ViesConfig {
    fn default() -> Self {
        ViesConfig {
            base_url: DEFAULT_BASE_URL.into(),
            use_test_service: false,
            requester_country: "HU".into(),
            requester_number: None,
            http_timeout: Duration::from_millis(15_000),
            max_attempts: 3,
            backoff_base: Duration::from_secs(1),
            bulk_interval: Duration::from_millis(500),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CheckVatRequest {
    pub country_code: String,
    pub vat_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester_member_state_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requester_number: Option<String>,
}

/// What one upstream check produced, before persistence gives it an id.
#[derive(Debug, Clone)]
pub struct CheckOutcome {
    pub status: VatCheckStatus,
    pub error_code: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    pub request_identifier: Option<String>,
    pub request_date: Option<String>,
    /// `HU87654321` when the requester fields were sent and kept.
    pub requester: Option<String>,
    pub attempts: u32,
    pub duration_ms: u64,
    /// Last raw body, success or error.
    pub raw_response: serde_json::Value,
    pub hints: Vec<String>,
}

/// The API shape (spec §5).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct VatCheckResult {
    pub status: VatCheckStatus,
    pub source: &'static str,
    pub country_code: String,
    pub vat_number: String,
    pub formatted: String,
    pub hu_domestic_tax_number: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    /// `address` split into the partner fields (best effort; see vies::address).
    pub address_parts: Option<AddressParts>,
    pub request_identifier: Option<String>,
    pub request_date: Option<String>,
    pub checked_at: chrono::DateTime<chrono::Utc>,
    pub error_code: Option<String>,
    pub cached: bool,
    pub check_id: Option<i64>,
    pub hints: Vec<String>,
}

impl VatCheckResult {
    pub fn from_outcome(
        vat: &NormalizedVat,
        outcome: &CheckOutcome,
        checked_at: chrono::DateTime<chrono::Utc>,
        check_id: Option<i64>,
    ) -> Self {
        let mut hints = vat.hints.clone();
        hints.extend(outcome.hints.iter().cloned());
        VatCheckResult {
            status: outcome.status,
            source: "VIES",
            country_code: vat.country_code.clone(),
            vat_number: vat.vat_number.clone(),
            formatted: vat.formatted.clone(),
            hu_domestic_tax_number: vat.hu_domestic_tax_number.clone(),
            name: outcome.name.clone(),
            address: outcome.address.clone(),
            address_parts: outcome.address.as_deref().map(split_address),
            request_identifier: outcome.request_identifier.clone(),
            request_date: outcome.request_date.clone(),
            checked_at,
            error_code: outcome.error_code.clone(),
            cached: false,
            check_id,
            hints,
        }
    }

    /// Never calls VIES and is never persisted.
    pub fn format_invalid(input: &str, error: FormatError) -> Self {
        VatCheckResult {
            status: VatCheckStatus::FormatInvalid,
            source: "VIES",
            country_code: String::new(),
            vat_number: String::new(),
            formatted: input.trim().to_string(),
            hu_domestic_tax_number: None,
            name: None,
            address: None,
            address_parts: None,
            request_identifier: None,
            request_date: None,
            checked_at: chrono::Utc::now(),
            error_code: Some(error.code().into()),
            cached: false,
            check_id: None,
            hints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ViesStatus {
    pub vow: VowStatus,
    #[serde(default)]
    pub countries: Vec<CountryStatus>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
pub struct VowStatus {
    pub available: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct CountryStatus {
    pub country_code: String,
    pub availability: String,
}

impl ViesStatus {
    /// Anything but "Available" counts as down; an unlisted country is assumed up.
    pub fn is_down(&self, country: &str) -> bool {
        self.countries
            .iter()
            .find(|c| c.country_code.eq_ignore_ascii_case(country))
            .is_some_and(|c| c.availability != "Available")
    }
}

/// The HTTP leg, swappable so the retry policy is testable without the network.
/// `body: None` is a GET. Returns (status, body) or a transport error code.
pub trait Transport: Send + Sync + 'static {
    fn call(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> impl Future<Output = Result<(u16, String), String>> + Send;
}

pub struct HttpTransport {
    http: reqwest::Client,
    base_url: String,
}

impl HttpTransport {
    pub fn new(base_url: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("autocrm/", env!("CARGO_PKG_VERSION"), " (vies)"))
            .build()?;
        Ok(HttpTransport {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }
}

impl Transport for HttpTransport {
    async fn call(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<(u16, String), String> {
        let url = format!("{}{path}", self.base_url);
        let request = match body {
            Some(b) => self.http.post(url).json(&b),
            None => self.http.get(url),
        };
        let response = request
            .header("Accept", "application/json")
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| if e.is_timeout() { "CLIENT_TIMEOUT" } else { "NETWORK_ERROR" }.to_string())?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|_| "NETWORK_ERROR".to_string())?;
        Ok((status, text))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// Final; no retry.
    Error,
    Retry,
    /// Unknown code: unavailable, one retry at most.
    RetryOnce,
    RequesterRejected,
}

fn classify(code: &str) -> Class {
    match code {
        "INVALID_INPUT" | "VAT_BLOCKED" | "IP_BLOCKED" => Class::Error,
        "INVALID_REQUESTER_INFO" => Class::RequesterRejected,
        "SERVICE_UNAVAILABLE"
        | "MS_UNAVAILABLE"
        | "TIMEOUT"
        | "GLOBAL_MAX_CONCURRENT_REQ"
        | "GLOBAL_MAX_CONCURRENT_REQ_TIME"
        | "MS_MAX_CONCURRENT_REQ"
        | "MS_MAX_CONCURRENT_REQ_TIME"
        | "NETWORK_ERROR"
        | "CLIENT_TIMEOUT" => Class::Retry,
        c if c.starts_with("HTTP_5") => Class::Retry,
        _ => Class::RetryOnce,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CheckOptions {
    /// User-triggered: the whole check must finish within 25 s.
    pub interactive: bool,
    /// A job working through many numbers: calls are spaced by `bulk_interval`.
    pub bulk: bool,
}

type InFlight = Shared<BoxFuture<'static, Arc<CheckOutcome>>>;

struct Inner<T> {
    transport: T,
    config: ViesConfig,
    global: Semaphore,
    per_country: Mutex<HashMap<String, Arc<Semaphore>>>,
    in_flight: Mutex<HashMap<String, InFlight>>,
    status: Mutex<Option<(Instant, Arc<ViesStatus>)>>,
    /// When the last bulk call went out.
    bulk_last: tokio::sync::Mutex<Option<Instant>>,
}

/// Cheap to clone; share one per process so the concurrency caps hold.
pub struct ViesClient<T: Transport = HttpTransport> {
    inner: Arc<Inner<T>>,
}

impl<T: Transport> Clone for ViesClient<T> {
    fn clone(&self) -> Self {
        ViesClient { inner: self.inner.clone() }
    }
}

impl ViesClient<HttpTransport> {
    pub fn new(config: ViesConfig) -> anyhow::Result<Self> {
        let transport = HttpTransport::new(&config.base_url)?;
        Ok(Self::with_transport(transport, config))
    }
}

impl<T: Transport> ViesClient<T> {
    pub fn with_transport(transport: T, config: ViesConfig) -> Self {
        ViesClient {
            inner: Arc::new(Inner {
                transport,
                config,
                global: Semaphore::new(2),
                per_country: Mutex::new(HashMap::new()),
                in_flight: Mutex::new(HashMap::new()),
                status: Mutex::new(None),
                bulk_last: tokio::sync::Mutex::new(None),
            }),
        }
    }

    /// One upstream check. Concurrent calls for the same number share one request.
    pub async fn check(&self, vat: &NormalizedVat, options: CheckOptions) -> Arc<CheckOutcome> {
        let key = vat.formatted.clone();
        let shared = {
            let mut in_flight = self.inner.in_flight.lock().unwrap();
            if let Some(existing) = in_flight.get(&key) {
                existing.clone()
            } else {
                let client = self.clone();
                let vat = vat.clone();
                let key2 = key.clone();
                let fut = async move {
                    let outcome = Arc::new(client.run(&vat, options).await);
                    client.inner.in_flight.lock().unwrap().remove(&key2);
                    outcome
                }
                .boxed()
                .shared();
                in_flight.insert(key, fut.clone());
                fut
            }
        };
        shared.await
    }

    /// `/check-status`, cached for 5 minutes. `None` when it can't be fetched.
    pub async fn status(&self) -> Option<Arc<ViesStatus>> {
        if let Some((at, status)) = self.inner.status.lock().unwrap().as_ref()
            && at.elapsed() < STATUS_TTL
        {
            return Some(status.clone());
        }
        let (code, body) = self
            .inner
            .transport
            .call("/check-status", None, self.inner.config.http_timeout)
            .await
            .inspect_err(|e| tracing::warn!(error = %e, "VIES check-status failed"))
            .ok()?;
        if !(200..300).contains(&code) {
            tracing::warn!(http_status = code, "VIES check-status failed");
            return None;
        }
        let status: ViesStatus = serde_json::from_str(&body)
            .inspect_err(|e| tracing::warn!(error = %e, "VIES check-status unparsable"))
            .ok()?;
        let status = Arc::new(status);
        *self.inner.status.lock().unwrap() = Some((Instant::now(), status.clone()));
        Some(status)
    }

    fn country_gate(&self, country: &str) -> Arc<Semaphore> {
        self.inner
            .per_country
            .lock()
            .unwrap()
            .entry(country.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }

    async fn run(&self, vat: &NormalizedVat, options: CheckOptions) -> CheckOutcome {
        let config = &self.inner.config;
        let path = if config.use_test_service {
            "/check-vat-test-service"
        } else {
            "/check-vat-number"
        };
        let started = Instant::now();
        let deadline = options.interactive.then(|| started + INTERACTIVE_BUDGET);
        let mut requester = config
            .requester_number
            .as_deref()
            .filter(|n| !n.trim().is_empty())
            .map(|n| (config.requester_country.clone(), n.trim().to_string()));
        let mut hints = Vec::new();
        let mut attempts = 0u32;
        let mut retried_unknown = false;
        let mut retried_requester = false;
        let mut retries = 0u32;

        let finish = |status, error_code: Option<String>, verdict: Option<parse::Verdict>, raw, requester: &Option<(String, String)>, attempts, hints: Vec<String>| {
            let v = verdict.unwrap_or(parse::Verdict {
                valid: false,
                name: None,
                address: None,
                request_identifier: None,
                request_date: None,
            });
            CheckOutcome {
                status,
                error_code,
                name: v.name,
                address: v.address,
                request_identifier: v.request_identifier,
                request_date: v.request_date,
                requester: requester.as_ref().map(|(c, n)| format!("{c}{n}")),
                attempts,
                duration_ms: started.elapsed().as_millis() as u64,
                raw_response: raw,
                hints,
            }
        };

        loop {
            let mut timeout = config.http_timeout;
            if let Some(d) = deadline {
                let left = d.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break finish(VatCheckStatus::Unavailable, Some("CLIENT_TIMEOUT".into()), None, serde_json::Value::Null, &requester, attempts, hints);
                }
                timeout = timeout.min(left);
            }

            let request = CheckVatRequest {
                country_code: vat.country_code.clone(),
                vat_number: vat.vat_number.clone(),
                requester_member_state_code: requester.as_ref().map(|(c, _)| c.clone()),
                requester_number: requester.as_ref().map(|(_, n)| n.clone()),
            };
            if options.bulk {
                // Spaced across every bulk caller in the process, retries included.
                let mut last = self.inner.bulk_last.lock().await;
                if let Some(at) = *last {
                    let ready = at + config.bulk_interval;
                    if let Some(wait) = ready.checked_duration_since(Instant::now()) {
                        tokio::time::sleep(wait).await;
                    }
                }
                *last = Some(Instant::now());
            }
            attempts += 1;
            let attempt_started = Instant::now();
            let response = {
                let gate = self.country_gate(&vat.country_code);
                let _country = gate.acquire().await.expect("semaphore never closed");
                let _global = self.inner.global.acquire().await.expect("semaphore never closed");
                self.inner
                    .transport
                    .call(path, Some(serde_json::to_value(&request).expect("serialisable")), timeout)
                    .await
            };
            let (http_status, reply, raw) = match response {
                Ok((code, body)) => {
                    let (reply, raw) = parse_check_reply(code, &body);
                    (Some(code), reply, raw)
                }
                Err(code) => (None, Reply::Fault(code), serde_json::Value::Null),
            };
            let code = match &reply {
                Reply::Verdict(v) => if v.valid { "VALID" } else { "INVALID" },
                Reply::Fault(c) => c.as_str(),
            };
            tracing::info!(
                country = %vat.country_code,
                code,
                http_status,
                duration_ms = attempt_started.elapsed().as_millis() as u64,
                attempt = attempts,
                "VIES check attempt"
            );

            let code = match reply {
                Reply::Verdict(v) => {
                    let status = if v.valid { VatCheckStatus::Valid } else { VatCheckStatus::Invalid };
                    break finish(status, None, Some(v), raw, &requester, attempts, hints);
                }
                Reply::Fault(code) => code,
            };

            let retry = match classify(&code) {
                Class::Error => {
                    if code == "IP_BLOCKED" {
                        tracing::error!(alert = true, "VIES has blocked this server's IP address");
                    }
                    break finish(VatCheckStatus::Error, Some(code), None, raw, &requester, attempts, hints);
                }
                Class::RequesterRejected => {
                    if requester.is_some() && !retried_requester {
                        tracing::error!(
                            "VIES rejected our requester VAT number (VIES_REQUESTER_*); retrying without it"
                        );
                        requester = None;
                        retried_requester = true;
                        hints.push(HINT_REQUESTER_REJECTED.into());
                        // Not a VIES-side failure: retry at once, outside the attempt budget.
                        continue;
                    }
                    break finish(VatCheckStatus::Error, Some(code), None, raw, &requester, attempts, hints);
                }
                Class::RetryOnce => {
                    let first = !retried_unknown;
                    retried_unknown = true;
                    first
                }
                Class::Retry => {
                    if code == "MS_UNAVAILABLE"
                        && self.status().await.is_some_and(|s| s.is_down(&vat.country_code))
                    {
                        false
                    } else {
                        true
                    }
                }
            };
            if !retry || retries + 1 >= config.max_attempts {
                break finish(VatCheckStatus::Unavailable, Some(code), None, raw, &requester, attempts, hints);
            }

            let step = config.backoff_base * 2u32.pow(retries);
            let jitter_cap = (config.backoff_base / 2).as_millis() as u64;
            let jitter = Duration::from_millis(if jitter_cap == 0 { 0 } else { rand::rng().random_range(0..=jitter_cap) });
            let wait = step + jitter;
            if deadline.is_some_and(|d| Instant::now() + wait >= d) {
                break finish(VatCheckStatus::Unavailable, Some(code), None, raw, &requester, attempts, hints);
            }
            retries += 1;
            tokio::time::sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests;
