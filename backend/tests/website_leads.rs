//! `POST /api/leads/website`: the autotherm.hu contact form files a lead.

mod common;

use std::sync::Arc;

use autocrm::AppState;
use autocrm::api;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const KEY: &str = "test-leads-key";

fn state(pool: PgPool, key: Option<&str>) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.leads_api_key = key.map(String::from);
    state.config = Arc::new(config);
    state
}

async fn post(state: AppState, key: Option<&str>, body: Value) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/leads/website")
        .header("content-type", "application/json");
    if let Some(key) = key {
        builder = builder.header("x-leads-key", key);
    }
    api::router(state)
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_form_with_the_key_becomes_an_unassigned_lead_in_the_first_stage(pool: PgPool) {
    let status = post(
        state(pool.clone(), Some(KEY)),
        Some(KEY),
        json!({
            "name": "  Kiss Péter ",
            "email": "Peter@Example.HU",
            "phone": "+36 30 123 4567",
            "message": "Hűtős kisteherautó átalakítás",
            "subject": "Hűtőkamra",
            "vehicle": "Sprinter 316",
            "page": "/szolgaltatasok"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        contact_name: Option<String>,
        contact_email: Option<String>,
        source: Option<String>,
        description: Option<String>,
        created_by: Option<i64>,
        assigned_to: Option<i64>,
        stage: Option<String>,
    }
    let lead: Row = sqlx::query_as(
        "SELECT l.title, l.contact_name, l.contact_email, l.source, l.description,
                l.created_by, l.assigned_to,
                (SELECT stage_key FROM lead_stages WHERE lead_id = l.id
                 ORDER BY entered_at DESC, id DESC LIMIT 1) AS stage
         FROM leads l",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lead.title, "Weboldal: Hűtőkamra");
    assert_eq!(lead.contact_name.as_deref(), Some("Kiss Péter"));
    assert_eq!(lead.contact_email.as_deref(), Some("peter@example.hu"));
    assert_eq!(lead.source.as_deref(), Some("website"));
    assert!(lead.created_by.is_none() && lead.assigned_to.is_none());
    let description = lead.description.unwrap();
    assert!(description.contains("Hűtős kisteherautó"));
    assert!(description.contains("Jármű: Sprinter 316"));
    assert!(lead.stage.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn without_the_right_key_nothing_is_filed(pool: PgPool) {
    let form = json!({ "name": "Teszt", "email": "a@example.hu" });
    // Wrong key, missing key, and the endpoint switched off all look the same.
    assert_eq!(
        post(state(pool.clone(), Some(KEY)), Some("nope"), form.clone()).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(state(pool.clone(), Some(KEY)), None, form.clone()).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(state(pool.clone(), None), Some(""), form).await,
        StatusCode::FORBIDDEN
    );
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn bad_forms_are_refused_and_the_honeypot_is_dropped_quietly(pool: PgPool) {
    let s = || state(pool.clone(), Some(KEY));
    for body in [
        json!({ "name": "  ", "email": "a@example.hu" }),
        json!({ "name": "Teszt" }),
        json!({ "name": "Teszt", "email": "not-an-address" }),
        json!({ "name": "Teszt", "email": "a@example.hu", "message": "x".repeat(5001) }),
    ] {
        assert_eq!(post(s(), Some(KEY), body).await, StatusCode::BAD_REQUEST);
    }
    // A bot that fills the hidden field is told it worked, and nothing is stored.
    let status = post(
        s(),
        Some(KEY),
        json!({ "name": "Bot", "email": "bot@example.hu", "company": "Spam Kft" }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

/// What the alert row looks like: (to, subject, body, trigger, status).
async fn alerts(pool: &PgPool) -> Vec<(String, String, String, String, String)> {
    sqlx::query_as(
        "SELECT to_address, subject, body_text, trigger, status::text FROM email_messages
         ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

fn state_with_alert(pool: PgPool, to: Option<&str>) -> AppState {
    let mut state = state(pool, Some(KEY));
    let mut config = (*state.config).clone();
    config.leads_notify_to = to.map(String::from);
    state.config = Arc::new(config);
    state
}

#[sqlx::test(migrations = "./migrations")]
async fn each_website_lead_queues_one_alert_to_the_office(pool: PgPool) {
    let status = post(
        state_with_alert(pool.clone(), Some("vastag.peter@autotherm.hu")),
        Some(KEY),
        json!({
            "name": "Kiss Péter",
            "email": "peter@example.hu",
            "phone": "+36 30 123 4567",
            "message": "Árajánlatot kérek {{MISSING:x}}",
            "subject": "Hűtőkamra"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let rows = alerts(&pool).await;
    assert_eq!(rows.len(), 1);
    let (to, subject, body, trigger, status) = &rows[0];
    assert_eq!(to, "vastag.peter@autotherm.hu");
    assert_eq!(subject, "Új weboldali érdeklődés: Weboldal: Hűtőkamra");
    assert_eq!(trigger, "website_lead_alert");
    assert_eq!(status, "queued");
    for needle in [
        "Kiss Péter",
        "peter@example.hu",
        "+36 30 123 4567",
        "Árajánlatot kérek",
        "/hu/leads/",
    ] {
        assert!(body.contains(needle), "alert body lacks {needle:?}: {body}");
    }
    // Visitor text must not be able to pass for a template marker and fail the alert.
    assert!(!body.contains("{{MISSING:"), "{body}");

    // A second lead is a second alert; a bot's honeypot submission is none.
    post(
        state_with_alert(pool.clone(), Some("vastag.peter@autotherm.hu")),
        Some(KEY),
        json!({ "name": "Nagy Anna", "phone": "+36 1 555 0000" }),
    )
    .await;
    post(
        state_with_alert(pool.clone(), Some("vastag.peter@autotherm.hu")),
        Some(KEY),
        json!({ "name": "Bot", "email": "bot@example.hu", "company": "Spam" }),
    )
    .await;
    assert_eq!(alerts(&pool).await.len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn without_a_recipient_no_alert_is_queued_but_the_lead_still_is(pool: PgPool) {
    let status = post(
        state_with_alert(pool.clone(), None),
        Some(KEY),
        json!({ "name": "Kiss Péter", "email": "peter@example.hu" }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(alerts(&pool).await.is_empty());
    let leads = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(leads, 1);
}

async fn set_rails(pool: &PgPool, enabled: bool, user_id: i64) {
    use autocrm::repo::config::{self, SettingsUpdate};
    use chrono::NaiveTime;
    config::update_settings(
        pool,
        &SettingsUpdate {
            automatic_email_enabled: enabled,
            // The strictest rails there are: one automatic mail per recipient per day, and
            // a window of one minute at 03:00 on weekdays (closed at almost any moment).
            max_auto_emails_per_recipient_day: 1,
            send_window_start: NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
            send_window_end: NaiveTime::from_hms_opt(3, 1, 0).unwrap(),
            send_window_weekdays_only: true,
            nudge_interval_days: 3,
            nudge_escalate_after: 2,
            stage_change_notifications: false,
            stalled_alert_recipients: vec![],
            email_mode: None,
            smtp_host: None,
            smtp_port: None,
            smtp_security: None,
            smtp_username: None,
            smtp_password: None,
            smtp_helo_name: None,
            smtp_force_ipv4: None,
            redirect_to: None,
        },
        user_id,
    )
    .await
    .unwrap();
}

async fn deliver_all(pool: &PgPool, state: &AppState) {
    use autocrm::integrations::email::Mailer;
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM email_messages ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap();
    for id in ids {
        autocrm::service::email::deliver(state, &Mailer::DryRun, id)
            .await
            .unwrap();
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn alerts_ignore_the_send_window_and_the_daily_cap_but_not_the_kill_switch(pool: PgPool) {
    let user = common::user(&pool, autocrm::domain::role::Role::Admin).await;
    set_rails(&pool, true, user.user_id).await;
    let state = state_with_alert(pool.clone(), Some("vastag.peter@autotherm.hu"));
    for name in ["Egy", "Kettő", "Három"] {
        let status = post(
            state.clone(),
            Some(KEY),
            json!({ "name": name, "email": "x@example.hu" }),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
    }
    deliver_all(&pool, &state).await;
    let statuses: Vec<String> = alerts(&pool).await.into_iter().map(|r| r.4).collect();
    assert_eq!(statuses, ["sent", "sent", "sent"]);

    // With automatic email switched off, nothing goes out, alerts included.
    set_rails(&pool, false, user.user_id).await;
    post(
        state.clone(),
        Some(KEY),
        json!({ "name": "Négy", "email": "y@example.hu" }),
    )
    .await;
    deliver_all(&pool, &state).await;
    let last = alerts(&pool).await.pop().unwrap();
    assert_eq!(last.4, "cancelled");
}
