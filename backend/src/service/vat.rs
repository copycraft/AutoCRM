//! VAT-number checks: format, cache, VIES, audit log, and the verdict on the partner.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::integrations::vies::{
    CheckOptions, NormalizedVat, VatCheckResult, VatCheckStatus, ViesClient, ViesConfig,
    ViesStatus, normalize_vat,
};
use crate::repo::vat_checks::{self, NewVatCheck, VatCheck};

/// Per-user cap on checks (spec: 30 per minute).
const RATE_LIMIT: usize = 30;
const RATE_WINDOW: Duration = Duration::from_secs(60);

/// One client per distinct VIES configuration, shared by the whole process so the
/// concurrency caps and in-flight dedupe hold across requests.
pub fn client(config: &ViesConfig) -> ViesClient {
    static CLIENTS: OnceLock<Mutex<HashMap<String, ViesClient>>> = OnceLock::new();
    let key = format!("{config:?}");
    let mut clients = CLIENTS.get_or_init(Default::default).lock().unwrap();
    clients
        .entry(key)
        .or_insert_with(|| ViesClient::new(config.clone()).expect("building the VIES HTTP client"))
        .clone()
}

/// Refuses the 31st check a user makes within a minute.
pub fn rate_limit(user_id: i64) -> AppResult<()> {
    static SEEN: OnceLock<Mutex<HashMap<i64, VecDeque<Instant>>>> = OnceLock::new();
    let mut seen = SEEN.get_or_init(Default::default).lock().unwrap();
    let now = Instant::now();
    let times = seen.entry(user_id).or_default();
    while times.front().is_some_and(|t| now.duration_since(*t) >= RATE_WINDOW) {
        times.pop_front();
    }
    if times.len() >= RATE_LIMIT {
        return Err(AppError::TooManyRequests);
    }
    times.push_back(now);
    Ok(())
}

pub struct CheckRequest<'a> {
    pub input: &'a str,
    pub default_country: Option<&'a str>,
    /// Only partners for now; invoices join with the invoice gate.
    pub partner_id: Option<i64>,
    pub force: bool,
    /// A user id, `system` or `migration`.
    pub created_by: String,
    pub interactive: bool,
}

/// `Err(result)` is FORMAT_INVALID: VIES was not asked and nothing was stored.
pub async fn check(
    db: &sqlx::PgPool,
    config: &Config,
    req: CheckRequest<'_>,
) -> AppResult<Result<VatCheckResult, VatCheckResult>> {
    let vat = match normalize_vat(req.input, req.default_country) {
        Ok(vat) => vat,
        Err(e) => return Ok(Err(VatCheckResult::format_invalid(req.input, e))),
    };
    // Refused before VIES is asked, so an audit row never points at nothing.
    if let Some(id) = req.partner_id
        && crate::repo::partners::find(db, id).await?.is_none()
    {
        return Err(AppError::NotFound("partner"));
    }

    if !req.force {
        let since = Utc::now() - chrono::Duration::days(config.vies_cache_max_age_days.max(0));
        if let Some(hit) = vat_checks::latest_verdict(db, &vat.country_code, &vat.vat_number, since).await? {
            if let Some(partner) = req.partner_id {
                link_partner(db, partner, &hit, &vat).await?;
            }
            return Ok(Ok(cached_result(&vat, &hit)));
        }
    }

    let outcome = client(&config.vies)
        .check(&vat, CheckOptions { interactive: req.interactive })
        .await;
    let mut hints = vat.hints.clone();
    hints.extend(outcome.hints.iter().cloned());
    let request_date = outcome
        .request_date
        .as_deref()
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc));
    let row = vat_checks::insert(
        db,
        NewVatCheck {
            subject: req.partner_id.map(|id| ("partner", id)),
            input_raw: req.input,
            country_code: &vat.country_code,
            vat_number: &vat.vat_number,
            hu_domestic_tax_number: vat.hu_domestic_tax_number.as_deref(),
            status: outcome.status.as_str(),
            error_code: outcome.error_code.as_deref(),
            name: outcome.name.as_deref(),
            address: outcome.address.as_deref(),
            request_identifier: outcome.request_identifier.as_deref(),
            request_date,
            requester: outcome.requester.as_deref(),
            attempts: outcome.attempts as i32,
            duration_ms: outcome.duration_ms.min(i32::MAX as u64) as i32,
            raw_response: &outcome.raw_response,
            hints: &hints,
            created_by: &req.created_by,
        },
    )
    .await?;
    if let Some(partner) = req.partner_id {
        link_partner(db, partner, &row, &vat).await?;
    }
    Ok(Ok(VatCheckResult::from_outcome(&vat, &outcome, row.created_at, Some(row.id))))
}

async fn link_partner(db: &sqlx::PgPool, partner: i64, row: &VatCheck, vat: &NormalizedVat) -> AppResult<()> {
    if vat_checks::set_partner_status(db, partner, row, &vat.formatted).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("partner"))
    }
}

fn cached_result(vat: &NormalizedVat, row: &VatCheck) -> VatCheckResult {
    let status = if row.status == "VALID" { VatCheckStatus::Valid } else { VatCheckStatus::Invalid };
    let mut hints = vat.hints.clone();
    for h in &row.hints {
        if !hints.contains(h) {
            hints.push(h.clone());
        }
    }
    VatCheckResult {
        status,
        source: "VIES",
        country_code: vat.country_code.clone(),
        vat_number: vat.vat_number.clone(),
        formatted: vat.formatted.clone(),
        hu_domestic_tax_number: vat.hu_domestic_tax_number.clone(),
        name: row.name.clone(),
        address: row.address.clone(),
        address_parts: row.address.as_deref().map(crate::integrations::vies::split_address),
        request_identifier: row.request_identifier.clone(),
        request_date: row.request_date.map(|d| d.to_rfc3339()),
        checked_at: row.created_at,
        error_code: row.error_code.clone(),
        cached: true,
        check_id: Some(row.id),
        hints,
    }
}

/// VIES's own availability report (cached 5 min). None when VIES can't be reached.
pub async fn status(config: &Config) -> Option<Arc<ViesStatus>> {
    client(&config.vies).status().await
}
