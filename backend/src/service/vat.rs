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
    /// Part of a job working through many numbers (calls spaced 500 ms apart).
    pub bulk: bool,
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
        .check(&vat, CheckOptions { interactive: req.interactive, bulk: req.bulk })
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

/// Partners per sweep job; the next batch is a new job, so a long sweep never meets the
/// worker's job timeout (at most ~40 × (0.5 s spacing + VIES's few seconds)).
pub const SWEEP_BATCH: i64 = 40;

/// How long UNAVAILABLE answers are re-asked before a person is asked to look.
const UNAVAILABLE_GIVE_UP: chrono::TimeDelta = chrono::TimeDelta::hours(24);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SweepMode {
    /// Every 15 minutes: ask again about numbers VIES could not answer for.
    Unavailable,
    /// Monthly: are the valid numbers of active partners still valid?
    Revalidate,
    /// After the MiniCRM import: check every imported EU VAT number once.
    Backfill,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SweepPayload {
    pub mode: SweepMode,
    /// Partners up to this id are done.
    #[serde(default)]
    pub after_id: i64,
    /// Revalidate: numbers checked before this (the sweep's start).
    #[serde(default)]
    pub checked_before: Option<DateTime<Utc>>,
}

#[derive(Debug, Default)]
pub struct SweepReport {
    pub checked: usize,
    /// UNAVAILABLE for a day: flagged for a manual re-check.
    pub flagged: usize,
    /// VALID until now, INVALID today.
    pub invalidated: usize,
    /// Malformed numbers the backfill skipped.
    pub skipped: usize,
    /// The next batch, when this one was full.
    pub next: Option<SweepPayload>,
}

/// One batch of a sweep.
pub async fn sweep(db: &sqlx::PgPool, config: &Config, p: &SweepPayload) -> AppResult<SweepReport> {
    let batch = match p.mode {
        SweepMode::Unavailable => vat_checks::unavailable_partners(db, p.after_id, SWEEP_BATCH).await?,
        SweepMode::Revalidate => {
            let before = p.checked_before.unwrap_or_else(Utc::now);
            vat_checks::revalidation_due(db, p.after_id, before, SWEEP_BATCH).await?
        }
        SweepMode::Backfill => vat_checks::never_checked(db, p.after_id, SWEEP_BATCH).await?,
    };
    let mut report = SweepReport::default();
    for partner in &batch {
        if p.mode == SweepMode::Unavailable
            && partner
                .vat_unavailable_since
                .is_some_and(|since| Utc::now() - since >= UNAVAILABLE_GIVE_UP)
        {
            vat_checks::flag_manual_check(db, partner.id).await?;
            tracing::warn!(partner_id = partner.id, number = %partner.number, "VIES unavailable for a day; needs a manual re-check");
            report.flagged += 1;
            continue;
        }
        let result = check(
            db,
            config,
            CheckRequest {
                input: &partner.number,
                default_country: Some(&partner.country),
                partner_id: Some(partner.id),
                // The backfill may reuse a fresh verdict on the same number; re-checks
                // exist to ask again.
                force: p.mode != SweepMode::Backfill,
                created_by: if p.mode == SweepMode::Backfill { "migration" } else { "system" }.into(),
                interactive: false,
                bulk: true,
            },
        )
        .await?;
        match result {
            Ok(r) => {
                report.checked += 1;
                if p.mode == SweepMode::Revalidate && r.status == VatCheckStatus::Invalid {
                    report.invalidated += 1;
                    if let Some(found) = crate::repo::partners::find(db, partner.id).await? {
                        crate::service::notifications::vat_number_invalidated(db, &found, &r.formatted).await?;
                    }
                }
            }
            Err(_) => report.skipped += 1,
        }
    }
    if batch.len() as i64 == SWEEP_BATCH {
        report.next = Some(SweepPayload {
            after_id: batch.last().map_or(p.after_id, |b| b.id),
            ..p.clone()
        });
    }
    Ok(report)
}

/// What the invoice gate found: the check the invoice rests on, and the reason when the
/// user issued it anyway while VIES was down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvoiceGate {
    pub vat_check_id: Option<i64>,
    pub override_reason: Option<String>,
}

/// Whether an invoice is an EU-business 0% one (intra-EU supply / reverse charge): the
/// buyer is a business in another member state and no VAT is charged. AutoCRM has no
/// separate VAT treatment field; this combination is what stands for one.
pub fn needs_vies(partner: &crate::repo::partners::Partner, vat_rate: rust_decimal::Decimal) -> bool {
    let country = partner.country.trim().to_ascii_uppercase();
    let vies_country = if country == "GR" { "EL".to_string() } else { country };
    partner.kind == crate::domain::partner::PartnerKind::Business
        && vat_rate.is_zero()
        && vies_country != "HU"
        && crate::integrations::vies::normalize::MEMBER_STATES.contains(&vies_country.as_str())
}

/// Before a 0% invoice to an EU business is issued: the buyer's EU VAT number must have a
/// VALID VIES answer no older than `VIES_INVOICE_MAX_AGE_HOURS`, asked for now when there
/// is none. INVALID always blocks; UNAVAILABLE blocks unless overrides are allowed and the
/// user wrote down why. `Ok(None)`: the gate does not apply.
pub async fn invoice_gate(
    db: &sqlx::PgPool,
    config: &Config,
    partner: &crate::repo::partners::Partner,
    vat_rate: rust_decimal::Decimal,
    override_reason: Option<&str>,
    user_id: i64,
) -> AppResult<Option<InvoiceGate>> {
    if !needs_vies(partner, vat_rate) {
        return Ok(None);
    }
    let Some(input) = partner.eu_tax_number.as_deref().filter(|n| !n.trim().is_empty()) else {
        return Err(AppError::rule(
            "eu_vat_number_required",
            format!("partner '{}' has no EU VAT number; a 0% EU invoice needs one", partner.name),
        ));
    };
    let vat = normalize_vat(input, Some(&partner.country)).map_err(|_| {
        AppError::rule(
            "eu_vat_number_invalid",
            format!("the EU VAT number '{input}' of partner '{}' is malformed", partner.name),
        )
    })?;

    let since = Utc::now() - chrono::TimeDelta::hours(config.vies_invoice_max_age_hours.max(0));
    let fresh = vat_checks::latest_verdict(db, &vat.country_code, &vat.vat_number, since).await?;
    let (status, check_id, code) = match fresh {
        Some(row) => (
            if row.status == "VALID" { VatCheckStatus::Valid } else { VatCheckStatus::Invalid },
            Some(row.id),
            row.error_code,
        ),
        None => {
            let r = check(
                db,
                config,
                CheckRequest {
                    input,
                    default_country: Some(&partner.country),
                    partner_id: Some(partner.id),
                    force: true,
                    created_by: user_id.to_string(),
                    interactive: true,
                    bulk: false,
                },
            )
            .await?
            .map_err(|_| AppError::rule("eu_vat_number_invalid", "the buyer's EU VAT number is malformed"))?;
            (r.status, r.check_id, r.error_code)
        }
    };

    match status {
        VatCheckStatus::Valid => Ok(Some(InvoiceGate { vat_check_id: check_id, override_reason: None })),
        VatCheckStatus::Invalid => Err(AppError::rule(
            "eu_vat_number_invalid",
            format!("VIES says {} is not a valid EU VAT number", vat.formatted),
        )),
        _ => {
            let reason = override_reason.map(str::trim).filter(|r| !r.is_empty());
            match reason {
                Some(reason) if config.vies_invoice_allow_override => {
                    tracing::warn!(user_id, number = %vat.formatted, reason, "0% EU invoice issued without a VIES verdict (override)");
                    Ok(Some(InvoiceGate {
                        vat_check_id: check_id,
                        override_reason: Some(reason.to_string()),
                    }))
                }
                _ => Err(AppError::rule(
                    "eu_vat_check_unavailable",
                    format!(
                        "VIES could not confirm {} ({}); try again later",
                        vat.formatted,
                        code.as_deref().unwrap_or("no answer")
                    ),
                )),
            }
        }
    }
}
