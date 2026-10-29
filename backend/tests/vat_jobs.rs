//! The VAT sweeps against a stand-in for VIES: the import backfill stays inside the call
//! caps and never turns throttling into INVALID, UNAVAILABLE answers are asked again and
//! then flagged after a day, and the monthly re-check raises a notification when a valid
//! number goes invalid.

mod common;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use autocrm::config::Config;
use autocrm::domain::role::Role;
use autocrm::service::vat::{self, SweepMode, SweepPayload};
use axum::routing::post;
use serde_json::{Value, json};
use sqlx::PgPool;

const SPACING: Duration = Duration::from_millis(40);

#[derive(Default)]
struct Stub {
    calls: AtomicUsize,
    in_flight: AtomicUsize,
    max_in_flight: AtomicUsize,
    started: Mutex<Vec<Instant>>,
    /// Numbers that already got their one throttling answer.
    throttled: Mutex<HashSet<String>>,
    /// Every answer is "not valid" from now on.
    invalid_now: AtomicBool,
    /// Every answer is "member state down".
    down: AtomicBool,
}

/// VIES as a bulk job meets it: every number's first call is a concurrency refusal, the
/// retry gets the answer.
async fn stub_vies() -> (String, Arc<Stub>) {
    let stub = Arc::new(Stub::default());
    let s = stub.clone();
    let app = axum::Router::new().route(
        "/check-vat-number",
        post(move |axum::Json(body): axum::Json<Value>| {
            let s = s.clone();
            async move {
                s.calls.fetch_add(1, Ordering::SeqCst);
                s.started.lock().unwrap().push(Instant::now());
                let now = s.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                s.max_in_flight.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                let number = body["vatNumber"].as_str().unwrap().to_string();
                let first = s.throttled.lock().unwrap().insert(number);
                let answer = if s.down.load(Ordering::SeqCst) {
                    json!({ "actionSucceed": false, "errorWrappers": [{ "error": "MS_UNAVAILABLE" }] })
                } else if first {
                    json!({ "actionSucceed": false, "errorWrappers": [{ "error": "MS_MAX_CONCURRENT_REQ" }] })
                } else {
                    json!({ "valid": !s.invalid_now.load(Ordering::SeqCst), "name": "ACME" })
                };
                s.in_flight.fetch_sub(1, Ordering::SeqCst);
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), stub)
}

fn config(url: String) -> Config {
    let mut c = common::config();
    c.vies.base_url = url;
    c.vies.backoff_base = Duration::ZERO;
    c.vies.bulk_interval = SPACING;
    c
}

/// Runs a sweep to the end, batch after batch, as the worker would.
async fn sweep_all(pool: &PgPool, c: &Config, mode: SweepMode) -> (usize, usize, usize) {
    let mut p = SweepPayload { mode, after_id: 0, checked_before: Some(chrono::Utc::now()) };
    let (mut checked, mut flagged, mut invalidated) = (0, 0, 0);
    loop {
        let r = vat::sweep(pool, c, &p).await.unwrap();
        checked += r.checked;
        flagged += r.flagged;
        invalidated += r.invalidated;
        match r.next {
            Some(next) => p = next,
            None => return (checked, flagged, invalidated),
        }
    }
}

async fn partner_with_vat(pool: &PgPool, n: usize) -> i64 {
    let id = common::partner(pool, &format!("Kunde {n}"), None).await;
    sqlx::query("UPDATE partners SET eu_tax_number = $2, country = 'DE' WHERE id = $1")
        .bind(id)
        .bind(format!("DE{}", 100_000_000 + n))
        .execute(pool)
        .await
        .unwrap();
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn the_import_backfill_respects_the_caps_and_never_says_invalid(pool: PgPool) {
    let (url, stub) = stub_vies().await;
    let c = config(url);
    for n in 0..50 {
        partner_with_vat(&pool, n).await;
    }
    // A malformed number is skipped, not retried forever.
    let bad = common::partner(&pool, "Rossz szám", None).await;
    sqlx::query("UPDATE partners SET eu_tax_number = 'US123' WHERE id = $1")
        .bind(bad)
        .execute(&pool)
        .await
        .unwrap();

    let (checked, _, _) = sweep_all(&pool, &c, SweepMode::Backfill).await;
    assert_eq!(checked, 50);

    let statuses: Vec<(String, i64)> =
        sqlx::query_as("SELECT status, count(*) FROM vat_checks GROUP BY status")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(statuses, vec![("VALID".to_string(), 50)], "throttling must never read as INVALID");
    let by: Vec<String> = sqlx::query_scalar("SELECT DISTINCT created_by FROM vat_checks")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(by, vec!["migration"]);

    // Every number was refused once, then answered: 100 calls, never two at a time, and
    // never closer together than the bulk spacing.
    assert_eq!(stub.calls.load(Ordering::SeqCst), 100);
    assert_eq!(stub.max_in_flight.load(Ordering::SeqCst), 1);
    let started = stub.started.lock().unwrap();
    let tightest = started.windows(2).map(|w| w[1] - w[0]).min().unwrap();
    assert!(tightest >= SPACING - Duration::from_millis(5), "calls {tightest:?} apart");

    // Run again: nothing left to do.
    assert_eq!(sweep_all(&pool, &c, SweepMode::Backfill).await.0, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn unavailable_is_asked_again_then_flagged_after_a_day(pool: PgPool) {
    let (url, stub) = stub_vies().await;
    let c = config(url);
    let recovers = partner_with_vat(&pool, 1).await;
    let stays_down = partner_with_vat(&pool, 2).await;

    stub.down.store(true, Ordering::SeqCst);
    sweep_all(&pool, &c, SweepMode::Backfill).await;
    let status: Vec<String> = sqlx::query_scalar("SELECT vat_status FROM partners WHERE id = ANY($1) ORDER BY id")
        .bind(vec![recovers, stays_down])
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(status, vec!["UNAVAILABLE", "UNAVAILABLE"]);

    // A day has passed for one of them.
    sqlx::query("UPDATE partners SET vat_unavailable_since = now() - interval '25 hours' WHERE id = $1")
        .bind(stays_down)
        .execute(&pool)
        .await
        .unwrap();
    stub.down.store(false, Ordering::SeqCst);
    let (checked, flagged, _) = sweep_all(&pool, &c, SweepMode::Unavailable).await;
    assert_eq!((checked, flagged), (1, 1));

    let rows: Vec<(Option<String>, bool, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT vat_status, vat_needs_manual_check, vat_unavailable_since FROM partners WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![recovers, stays_down])
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows[0].0.as_deref(), Some("VALID"));
    assert!(!rows[0].1 && rows[0].2.is_none());
    assert_eq!(rows[1].0.as_deref(), Some("UNAVAILABLE"));
    assert!(rows[1].1, "a day of UNAVAILABLE needs a person");

    // Flagged partners are left alone by later sweeps.
    assert_eq!(sweep_all(&pool, &c, SweepMode::Unavailable).await, (0, 0, 0));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_valid_number_going_invalid_raises_a_notification(pool: PgPool) {
    let (url, stub) = stub_vies().await;
    let c = config(url);
    let office = common::user(&pool, Role::Office).await;
    let active = common::order(&pool, &office, "HUF", vec![]).await.partner_id;
    let idle = common::partner(&pool, "Régi ügyfél", None).await;
    for id in [active, idle] {
        sqlx::query("UPDATE partners SET eu_tax_number = $2, country = 'DE' WHERE id = $1")
            .bind(id)
            .bind(format!("DE{}", 200_000_000 + id))
            .execute(&pool)
            .await
            .unwrap();
    }
    sweep_all(&pool, &c, SweepMode::Backfill).await;
    // Both were checked a while ago.
    sqlx::query("UPDATE partners SET vat_checked_at = now() - interval '40 days' WHERE id = ANY($1)")
        .bind(vec![active, idle])
        .execute(&pool)
        .await
        .unwrap();

    stub.invalid_now.store(true, Ordering::SeqCst);
    let (checked, _, invalidated) = sweep_all(&pool, &c, SweepMode::Revalidate).await;
    // Only the partner with recent activity is re-checked.
    assert_eq!((checked, invalidated), (1, 1));

    let status: String = sqlx::query_scalar("SELECT vat_status FROM partners WHERE id = $1")
        .bind(active)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "INVALID");
    let link: String = sqlx::query_scalar(
        "SELECT link FROM notifications WHERE user_id = $1 AND kind = 'vat_invalid'",
    )
    .bind(office.user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(link, format!("/partners/{active}"));
}
