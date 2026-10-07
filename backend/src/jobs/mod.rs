//! Background worker and scheduler over the Postgres job queue.
//!
//! One loop claims jobs with FOR UPDATE SKIP LOCKED and runs them; another enqueues the
//! periodic work (hourly nudge scan, daily stalled-order alerts, daily MNB rates) using
//! dedupe keys so each period's job exists exactly once, even with several instances.

use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::AppState;
use crate::integrations::email::Mailer;
use crate::repo::jobs::{self, Job};
use crate::service::automation;
use crate::service::email::{self, Delivery};
use crate::service::invoicing;

pub mod kinds {
    pub const SEND_EMAIL: &str = "send_email";
    pub const PROCESS_IMAGE: &str = "process_image";
    /// A thumbnail for a drawing or picture filed as a document.
    pub const PROCESS_DOCUMENT: &str = "process_document";
    /// An RFC 3161 stamp over an evidence photo.
    pub const TIMESTAMP_IMAGE: &str = "timestamp_image";
    /// Daily: queue stamps for evidence photos that have none.
    pub const TIMESTAMP_BACKFILL: &str = "timestamp_backfill";
    pub const NUDGE_BLOCKERS: &str = "nudge_blockers";
    pub const STALLED_ORDERS: &str = "stalled_orders";
    /// Hourly: quote follow-up letters that are due.
    pub const QUOTE_FOLLOWUPS: &str = "quote_followups";
    /// Daily: payment reminders for overdue transfer invoices.
    pub const PAYMENT_REMINDERS: &str = "payment_reminders";
    /// Daily: tell salespeople about quotes about to expire, and HR about documents.
    pub const EXPIRY_ALERTS: &str = "expiry_alerts";
    /// Every few minutes: read customer replies from the sales mailbox.
    pub const READ_MAILBOX: &str = "read_mailbox";
    /// At a newsletter's send time: write one letter per reader.
    pub const NEWSLETTER_DISPATCH: &str = "newsletter_dispatch";
    pub const FETCH_FX_RATES: &str = "fetch_fx_rates";
    /// Monday morning: last week's report to the managers (0049).
    pub const WEEKLY_REPORT: &str = "weekly_report";
    /// A Facebook/Instagram lead form submission to fetch and file (0049).
    pub const META_LEADGEN: &str = "meta_leadgen";
    /// A won lead to report to Meta's Conversions API (0049).
    pub const META_CONVERSION: &str = "meta_conversion";
    /// Report an invoice or storno to NAV through the sidecar, then store its PDF and
    /// queue the letter. Reporting is asynchronous at NAV, so it is asynchronous here.
    pub const NAV_SUBMIT_INVOICE: &str = "nav_submit_invoice";
    /// Technically annul a data report.
    pub const NAV_ANNUL_INVOICE: &str = "nav_annul_invoice";
}

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const SCHEDULER_INTERVAL: Duration = Duration::from_secs(60);
/// Must stay below repo::jobs::LEASE, or a slow job could be claimed twice.
const JOB_TIMEOUT: Duration = Duration::from_secs(600);
/// One job per claim. Claimed jobs share one lease and run one after another, so with a
/// batch of N the last job's lease has already run for up to (N-1) × JOB_TIMEOUT when it
/// starts: past LEASE from the third job on, when another worker (a second instance, or
/// the old process during a deploy) may claim it again. The queue is small enough that
/// claiming singly costs nothing; the loop claims again straight away while work remains.
const BATCH: i64 = 1;

enum Outcome {
    Done,
    RunAt(DateTime<Utc>),
}

pub async fn run(state: AppState, mut shutdown: watch::Receiver<bool>) {
    let scheduler = tokio::spawn(scheduler_loop(state.clone(), shutdown.clone()));
    tracing::info!(worker_id = %state.config.worker_id, "background worker started");

    while !*shutdown.borrow() {
        // Rebuilt every cycle from the effective config (admin settings row,
        // else environment), so transport changes apply without a restart.
        // Building the client performs no I/O; a broken config only skips
        // this cycle with an error log, it never stops the worker.
        let (email_config, source) = email::effective_email_config(&state.db, &state.config).await;
        tracing::debug!(?source, "effective email transport");
        let mailer = match Mailer::from_config(&email_config) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!(error = %e, "email transport is misconfigured; skipping this cycle");
                tokio::select! {
                    _ = tokio::time::sleep(POLL_INTERVAL) => {}
                    _ = shutdown.changed() => {}
                }
                continue;
            }
        };
        let claimed = match jobs::claim(&state.db, &state.config.worker_id, BATCH).await {
            Ok(jobs) => jobs,
            Err(e) => {
                tracing::error!(error = %e, "claiming jobs failed");
                Vec::new()
            }
        };
        if claimed.is_empty() {
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = shutdown.changed() => {}
            }
            continue;
        }
        // Finish claimed jobs even if shutdown arrives meanwhile; abandoning them would
        // leave them locked until the lease expires.
        for job in claimed {
            run_one(&state, &mailer, job).await;
        }
    }
    scheduler.abort();
    tracing::info!("background worker stopped");
}

async fn run_one(state: &AppState, mailer: &Mailer, job: Job) {
    let span = tracing::info_span!("job", id = job.id, kind = %job.kind, attempt = job.attempts);
    let _enter = span.enter();
    let result = match tokio::time::timeout(JOB_TIMEOUT, dispatch(state, mailer, &job)).await {
        Ok(r) => r,
        Err(_) => Err(anyhow!("timed out after {}s", JOB_TIMEOUT.as_secs())),
    };
    let recorded = match result {
        Ok(Outcome::Done) => jobs::complete(&state.db, job.id).await,
        Ok(Outcome::RunAt(at)) => jobs::defer(&state.db, job.id, at).await,
        Err(e) => {
            let message = format!("{e:#}");
            if job.attempts >= job.max_attempts {
                tracing::error!(error = %message, "job dead-lettered after final attempt");
                // The email log must not keep saying "queued" for a letter no job will
                // send any more (MAIL-L4).
                if job.kind == kinds::SEND_EMAIL {
                    if let Ok(p) = payload::<EmailPayload>(&job) {
                        if let Err(e) = email::job_dead_lettered(state, p.email_id, &message).await
                        {
                            tracing::error!(error = %e, email_id = p.email_id, "could not mark the email of a dead send job");
                        }
                    }
                }
                // An invoice whose request could never be built must not stay
                // `submitting` forever, blocking the order (INV-L10).
                if job.kind == kinds::NAV_SUBMIT_INVOICE
                    && invoicing::is_unbuildable_dead_letter(&message)
                {
                    if let Ok(p) = payload::<invoicing::SubmitPayload>(&job) {
                        if let Err(e) =
                            invoicing::job_dead_lettered_unbuildable(state, p.invoice_id, &message)
                                .await
                        {
                            tracing::error!(error = %e, invoice_id = p.invoice_id, "could not reject an unbuildable invoice");
                        }
                    }
                }
                jobs::fail(&state.db, job.id, &message, None).await
            } else {
                let retry_at = Utc::now() + jobs::backoff(job.attempts);
                tracing::warn!(error = %message, %retry_at, "job failed; will retry");
                jobs::fail(&state.db, job.id, &message, Some(retry_at)).await
            }
        }
    };
    if let Err(e) = recorded {
        tracing::error!(error = %e, "could not record job outcome; the lease will expire and it will run again");
    }
}

#[derive(Deserialize)]
struct EmailPayload {
    email_id: i64,
}

#[derive(Deserialize)]
struct ImagePayload {
    image_id: i64,
}

#[derive(Deserialize)]
struct DocumentPayload {
    document_id: i64,
}

#[derive(Deserialize)]
struct FxPayload {
    from: NaiveDate,
    to: NaiveDate,
}

#[derive(Deserialize)]
struct NewsletterPayload {
    send_id: i64,
}

#[derive(Deserialize)]
struct LeadgenPayload {
    leadgen_id: String,
}

#[derive(Deserialize)]
struct ConversionPayload {
    lead_id: i64,
    order_id: i64,
}

fn payload<T: for<'de> Deserialize<'de>>(job: &Job) -> anyhow::Result<T> {
    serde_json::from_value(job.payload.clone())
        .map_err(|e| anyhow!("bad payload for {}: {e}", job.kind))
}

async fn dispatch(state: &AppState, mailer: &Mailer, job: &Job) -> anyhow::Result<Outcome> {
    match job.kind.as_str() {
        kinds::SEND_EMAIL => {
            let p: EmailPayload = payload(job)?;
            Ok(match email::deliver(state, mailer, p.email_id).await? {
                Delivery::Done => Outcome::Done,
                Delivery::RunAt(at) => Outcome::RunAt(at),
            })
        }
        kinds::PROCESS_IMAGE => {
            let p: ImagePayload = payload(job)?;
            automation::process_image(state, p.image_id).await?;
            Ok(Outcome::Done)
        }
        kinds::PROCESS_DOCUMENT => {
            let p: DocumentPayload = payload(job)?;
            automation::process_document(state, p.document_id).await?;
            Ok(Outcome::Done)
        }
        kinds::TIMESTAMP_IMAGE => {
            let p: ImagePayload = payload(job)?;
            automation::timestamp_image(state, p.image_id).await?;
            Ok(Outcome::Done)
        }
        kinds::TIMESTAMP_BACKFILL => {
            automation::timestamp_backfill(state).await?;
            Ok(Outcome::Done)
        }
        kinds::NUDGE_BLOCKERS => {
            automation::nudge_blockers(state).await?;
            Ok(Outcome::Done)
        }
        kinds::STALLED_ORDERS => {
            automation::stalled_order_alerts(state).await?;
            Ok(Outcome::Done)
        }
        kinds::QUOTE_FOLLOWUPS => {
            crate::service::followups::send_due(state).await?;
            Ok(Outcome::Done)
        }
        kinds::PAYMENT_REMINDERS => {
            crate::service::reminders::send_payment_reminders(state).await?;
            Ok(Outcome::Done)
        }
        kinds::EXPIRY_ALERTS => {
            crate::service::reminders::quote_expiry_alerts(state).await?;
            crate::service::reminders::document_expiry_alerts(state).await?;
            Ok(Outcome::Done)
        }
        kinds::READ_MAILBOX => {
            crate::service::mailbox::read_mailbox(state).await?;
            Ok(Outcome::Done)
        }
        kinds::NEWSLETTER_DISPATCH => {
            let p: NewsletterPayload = payload(job)?;
            crate::service::newsletter::dispatch(state, p.send_id).await?;
            Ok(Outcome::Done)
        }
        kinds::NAV_SUBMIT_INVOICE => {
            let p: invoicing::SubmitPayload = payload(job)?;
            invoicing::submit_invoice(state, &p).await?;
            Ok(Outcome::Done)
        }
        kinds::NAV_ANNUL_INVOICE => {
            let p: invoicing::AnnulPayload = payload(job)?;
            invoicing::annul_job(state, &p, job.attempts > 1).await?;
            Ok(Outcome::Done)
        }
        kinds::FETCH_FX_RATES => {
            let p: FxPayload = payload(job)?;
            automation::fetch_fx_rates(state, p.from, p.to).await?;
            Ok(Outcome::Done)
        }
        kinds::WEEKLY_REPORT => {
            crate::service::weekly_report::send(state).await?;
            Ok(Outcome::Done)
        }
        kinds::META_LEADGEN => {
            let p: LeadgenPayload = payload(job)?;
            crate::service::ads::fetch_leadgen(state, &p.leadgen_id).await?;
            Ok(Outcome::Done)
        }
        kinds::META_CONVERSION => {
            let p: ConversionPayload = payload(job)?;
            crate::service::ads::send_conversion(state, p.lead_id, p.order_id).await?;
            Ok(Outcome::Done)
        }
        other => Err(anyhow!("unknown job kind '{other}'")),
    }
}

async fn scheduler_loop(state: AppState, mut shutdown: watch::Receiver<bool>) {
    loop {
        if let Err(e) = schedule_tick(&state).await {
            tracing::warn!(error = %e, "scheduler tick failed");
        }
        tokio::select! {
            _ = tokio::time::sleep(SCHEDULER_INTERVAL) => {}
            _ = shutdown.changed() => {}
        }
        if *shutdown.borrow() {
            break;
        }
    }
}

async fn schedule_tick(state: &AppState) -> anyhow::Result<()> {
    let local_now = Utc::now().with_timezone(&state.config.business_tz);
    let today = local_now.date_naive();
    let time = local_now.time();

    once(
        state,
        &format!(
            "{}:{}",
            kinds::NUDGE_BLOCKERS,
            local_now.format("%Y-%m-%dT%H")
        ),
        kinds::NUDGE_BLOCKERS,
        json!({}),
    )
    .await?;

    once(
        state,
        &format!(
            "{}:{}",
            kinds::QUOTE_FOLLOWUPS,
            local_now.format("%Y-%m-%dT%H")
        ),
        kinds::QUOTE_FOLLOWUPS,
        json!({}),
    )
    .await?;

    // Monday, after seven: last week's report.
    if time >= NaiveTime::from_hms_opt(7, 0, 0).expect("valid time")
        && today.weekday() == chrono::Weekday::Mon
    {
        once(
            state,
            &format!("{}:{today}", kinds::WEEKLY_REPORT),
            kinds::WEEKLY_REPORT,
            json!({}),
        )
        .await?;
    }
    if time >= NaiveTime::from_hms_opt(7, 0, 0).expect("valid time") {
        once(
            state,
            &format!("{}:{today}", kinds::STALLED_ORDERS),
            kinds::STALLED_ORDERS,
            json!({}),
        )
        .await?;
        once(
            state,
            &format!("{}:{today}", kinds::EXPIRY_ALERTS),
            kinds::EXPIRY_ALERTS,
            json!({}),
        )
        .await?;
    }
    // Payment reminders go out in office hours, after the morning's bank statements.
    if time >= NaiveTime::from_hms_opt(9, 0, 0).expect("valid time") {
        once(
            state,
            &format!("{}:{today}", kinds::PAYMENT_REMINDERS),
            kinds::PAYMENT_REMINDERS,
            json!({}),
        )
        .await?;
    }
    // Evidence photos without a stamp get one, once a day, when an authority is configured.
    if state.config.tsa.is_some() {
        once(
            state,
            &format!("{}:{today}", kinds::TIMESTAMP_BACKFILL),
            kinds::TIMESTAMP_BACKFILL,
            json!({}),
        )
        .await?;
    }
    // The sales mailbox is read every ten minutes, when one is configured.
    if state.config.imap.is_some() {
        let minute = local_now
            .format("%M")
            .to_string()
            .parse::<u32>()
            .unwrap_or(0);
        once(
            state,
            &format!(
                "{}:{}:{}",
                kinds::READ_MAILBOX,
                local_now.format("%Y-%m-%dT%H"),
                minute / 10
            ),
            kinds::READ_MAILBOX,
            json!({}),
        )
        .await?;
    }
    // MNB publishes the day's rates around noon.
    if time >= NaiveTime::from_hms_opt(12, 30, 0).expect("valid time") {
        let from = today - TimeDelta::days(10);
        once(
            state,
            &format!("{}:{today}", kinds::FETCH_FX_RATES),
            kinds::FETCH_FX_RATES,
            json!({ "from": from, "to": today }),
        )
        .await?;
    }
    Ok(())
}

async fn once(state: &AppState, key: &str, kind: &str, payload: Value) -> anyhow::Result<()> {
    if !jobs::key_used(&state.db, key).await? {
        jobs::enqueue(&state.db, kind, payload, None, Some(key)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_claimed_job_starts_and_finishes_inside_its_lease() {
        // Claimed jobs share one lease and run in sequence, so the last one can run until
        // BATCH × JOB_TIMEOUT after the claim. Past LEASE, another worker may claim it too.
        let worst_case = JOB_TIMEOUT.as_secs() * BATCH as u64;
        assert!(
            worst_case < jobs::LEASE.num_seconds() as u64,
            "BATCH × JOB_TIMEOUT ({worst_case}s) must stay below the lease"
        );
    }
}
