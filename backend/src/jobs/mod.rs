//! Background worker and scheduler over the Postgres job queue.
//!
//! One loop claims jobs with FOR UPDATE SKIP LOCKED and runs them; another enqueues the
//! periodic work (hourly nudge scan, daily stalled-order alerts, daily MNB rates) using
//! dedupe keys so each period's job exists exactly once, even with several instances.

use std::time::Duration;

use anyhow::anyhow;
use chrono::{DateTime, NaiveDate, NaiveTime, TimeDelta, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::AppState;
use crate::integrations::email::Mailer;
use crate::repo::jobs::{self, Job};
use crate::service::automation;
use crate::service::email::{self, Delivery};

pub mod kinds {
    pub const SEND_EMAIL: &str = "send_email";
    pub const PROCESS_IMAGE: &str = "process_image";
    pub const NUDGE_BLOCKERS: &str = "nudge_blockers";
    pub const STALLED_ORDERS: &str = "stalled_orders";
    pub const FETCH_FX_RATES: &str = "fetch_fx_rates";
}

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const SCHEDULER_INTERVAL: Duration = Duration::from_secs(60);
/// Must stay below repo::jobs::LEASE, or a slow job could be claimed twice.
const JOB_TIMEOUT: Duration = Duration::from_secs(600);
const BATCH: i64 = 5;

enum Outcome {
    Done,
    RunAt(DateTime<Utc>),
}

pub async fn run(state: AppState, mut shutdown: watch::Receiver<bool>) {
    let mailer = match Mailer::from_config(&state.config.email) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error = %e, "email transport is misconfigured; background worker NOT started");
            return;
        }
    };
    let scheduler = tokio::spawn(scheduler_loop(state.clone(), shutdown.clone()));
    tracing::info!(worker_id = %state.config.worker_id, dry_run = mailer.is_dry_run(), "background worker started");

    while !*shutdown.borrow() {
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
struct FxPayload {
    from: NaiveDate,
    to: NaiveDate,
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
        kinds::NUDGE_BLOCKERS => {
            automation::nudge_blockers(state).await?;
            Ok(Outcome::Done)
        }
        kinds::STALLED_ORDERS => {
            automation::stalled_order_alerts(state).await?;
            Ok(Outcome::Done)
        }
        kinds::FETCH_FX_RATES => {
            let p: FxPayload = payload(job)?;
            automation::fetch_fx_rates(state, p.from, p.to).await?;
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

    if time >= NaiveTime::from_hms_opt(7, 0, 0).expect("valid time") {
        once(
            state,
            &format!("{}:{today}", kinds::STALLED_ORDERS),
            kinds::STALLED_ORDERS,
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
