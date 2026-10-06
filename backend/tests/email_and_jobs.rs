//! The email safety rails and the job queue, against a real database.

mod common;

use chrono::{NaiveDate, NaiveTime};
use serde_json::json;
use sqlx::PgPool;

use autocrm::domain::email::EmailStatus;
use autocrm::domain::role::Role;
use autocrm::integrations::email::Mailer;
use autocrm::repo::blockers::{self, BlockerInput};
use autocrm::repo::config::{self, SettingsUpdate};
use autocrm::repo::emails::{self, NewEmail};
use autocrm::repo::jobs;
use autocrm::service::automation;
use autocrm::service::email::{
    About, AutomaticEmail, Delivery, deliver, queue_automatic, triggers,
};

async fn enable_automatic_email(pool: &PgPool, user_id: i64) {
    config::update_settings(
        pool,
        &SettingsUpdate {
            automatic_email_enabled: true,
            max_auto_emails_per_recipient_day: 3,
            send_window_start: NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            send_window_end: NaiveTime::from_hms_opt(23, 59, 59).unwrap(),
            send_window_weekdays_only: false,
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

async fn manual_email(pool: &PgPool, user_id: i64, status: EmailStatus) -> i64 {
    let id = emails::insert(
        pool,
        &NewEmail {
            order_id: None,
            lead_id: None,
            partner_id: None,
            blocker_id: None,
            template_key: None,
            trigger: triggers::MANUAL,
            sent_by: Some(user_id),
            idempotency_key: None,
            to_address: "customer@example.hu",
            cc: &[],
            bcc: &[],
            from_address: "Teszt <noreply@autotherm.hu>",
            reply_to: Some("teszt@autotherm.hu"),
            subject: "Teszt",
            body_html: "<p>Teszt</p>",
            body_text: "Teszt",
            attachments: json!([]),
            send_after: None,
        },
    )
    .await
    .unwrap()
    .unwrap();
    emails::set_status(pool, id, status, None).await.unwrap();
    id
}

#[sqlx::test(migrations = "./migrations")]
async fn job_dedupe_and_claiming(pool: PgPool) {
    let first = jobs::enqueue(&pool, "noop", json!({}), None, Some("k1"))
        .await
        .unwrap();
    let dup = jobs::enqueue(&pool, "noop", json!({}), None, Some("k1"))
        .await
        .unwrap();
    assert!(first.is_some());
    assert_eq!(
        dup, None,
        "a second unfinished job with the same key must not be created"
    );

    let claimed = jobs::claim(&pool, "w1", 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].attempts, 1);
    assert!(
        jobs::claim(&pool, "w2", 10).await.unwrap().is_empty(),
        "locked jobs are not claimable"
    );

    jobs::complete(&pool, claimed[0].id).await.unwrap();
    assert!(jobs::key_used(&pool, "k1").await.unwrap());
    assert!(
        jobs::enqueue(&pool, "noop", json!({}), None, Some("k1"))
            .await
            .unwrap()
            .is_some(),
        "finished keys can be reused"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn failing_jobs_back_off_then_dead_letter(pool: PgPool) {
    let id = jobs::enqueue(&pool, "noop", json!({}), None, None)
        .await
        .unwrap()
        .unwrap();
    let job = jobs::claim(&pool, "w", 1).await.unwrap().remove(0);
    jobs::fail(
        &pool,
        job.id,
        "boom",
        Some(chrono::Utc::now() + chrono::TimeDelta::hours(1)),
    )
    .await
    .unwrap();
    assert!(
        jobs::claim(&pool, "w", 1).await.unwrap().is_empty(),
        "retry is scheduled in the future"
    );

    jobs::fail(&pool, id, "boom again", None).await.unwrap();
    let failed = jobs::list_failed(&pool, 10).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert!(jobs::retry_failed(&pool, id).await.unwrap());
    assert_eq!(jobs::claim(&pool, "w", 1).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn dry_run_delivers_manual_mail(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let id = manual_email(&pool, user.user_id, EmailStatus::Queued).await;

    assert!(matches!(
        deliver(&state, &Mailer::DryRun, id).await.unwrap(),
        Delivery::Done
    ));
    let email = emails::find(&pool, id).await.unwrap().unwrap();
    assert_eq!(email.status, EmailStatus::Sent);
    assert!(email.provider_id.unwrap().ends_with("@autotherm.test>"));

    // Delivering again is a no-op: sent mail is never re-sent.
    deliver(&state, &Mailer::DryRun, id).await.unwrap();
    assert_eq!(emails::find(&pool, id).await.unwrap().unwrap().attempts, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn interrupted_send_goes_to_review_not_resend(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let id = manual_email(&pool, user.user_id, EmailStatus::Sending).await;
    deliver(&state, &Mailer::DryRun, id).await.unwrap();
    assert_eq!(
        emails::find(&pool, id).await.unwrap().unwrap().status,
        EmailStatus::NeedsReview
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn automatic_mail_is_idempotent_and_respects_kill_switch(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    let mut conn = pool.acquire().await.unwrap();
    let email = |key: &str| AutomaticEmail {
        template_key: "order_stage_changed",
        about: About {
            order_id: Some(order.id),
            ..Default::default()
        },
        to: "customer@example.hu",
        trigger: triggers::STAGE_CHANGED,
        idempotency_key: key.to_string(),
        attachments: Vec::new(),
        extra_values: Default::default(),
        sender: None,
    };
    let first = queue_automatic(&mut conn, &state.config, email("stage:1"))
        .await
        .unwrap();
    let again = queue_automatic(&mut conn, &state.config, email("stage:1"))
        .await
        .unwrap();
    assert!(first.is_some());
    assert_eq!(again, None);
    drop(conn);

    // Kill switch is off by default: the queued automatic mail is cancelled, not sent.
    let id = first.unwrap();
    deliver(&state, &Mailer::DryRun, id).await.unwrap();
    let cancelled = emails::find(&pool, id).await.unwrap().unwrap();
    assert_eq!(cancelled.status, EmailStatus::Cancelled);
    assert!(cancelled.subject.contains(&order.number));
}

#[sqlx::test(migrations = "./migrations")]
async fn suppressed_recipients_never_get_automatic_mail(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Admin).await;
    enable_automatic_email(&pool, user.user_id).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    emails::add_suppression(
        &pool,
        "Customer@Example.hu",
        Some("asked to stop"),
        user.user_id,
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let id = queue_automatic(
        &mut conn,
        &state.config,
        AutomaticEmail {
            template_key: "order_stage_changed",
            about: About {
                order_id: Some(order.id),
                ..Default::default()
            },
            to: "customer@example.hu",
            trigger: triggers::STAGE_CHANGED,
            idempotency_key: "stage:x".into(),
            attachments: Vec::new(),
            extra_values: Default::default(),
            sender: None,
        },
    )
    .await
    .unwrap()
    .unwrap();
    drop(conn);
    deliver(&state, &Mailer::DryRun, id).await.unwrap();
    assert_eq!(
        emails::find(&pool, id).await.unwrap().unwrap().status,
        EmailStatus::Cancelled
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn overdue_blockers_are_nudged_once_per_interval(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Admin).await;
    enable_automatic_email(&pool, user.user_id).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let supplier =
        common::partner(&pool, "Michael Supplies", Some("michael@supplier.example")).await;
    let blocker_id = blockers::insert(
        &pool,
        order.id,
        &BlockerInput {
            what: "ATP tanúsítvány".into(),
            responsible_partner_id: Some(supplier),
            responsible_email: None,
            due_date: NaiveDate::from_ymd_opt(2020, 1, 1),
            notes: None,
            nudge_enabled: true,
        },
        user.user_id,
    )
    .await
    .unwrap();

    assert_eq!(automation::nudge_blockers(&state).await.unwrap(), 1);
    assert_eq!(
        automation::nudge_blockers(&state).await.unwrap(),
        0,
        "interval not elapsed"
    );

    let blocker = blockers::find(
        &pool,
        blocker_id,
        autocrm::service::business_today(state.config.business_tz),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(blocker.nudge_count, 1);

    let log = emails::list(
        &pool,
        &emails::EmailFilter {
            order_id: Some(order.id),
            ..Default::default()
        },
        10,
        0,
    )
    .await
    .unwrap();
    assert_eq!(log.len(), 1, "nudge appears in the order's correspondence");
    assert_eq!(log[0].blocker_id, Some(blocker_id));
    assert_eq!(log[0].to_address, "michael@supplier.example");

    let full = emails::find(&pool, log[0].id).await.unwrap().unwrap();
    assert!(full.body_text.contains("ATP tanúsítvány"));
    assert!(!full.body_text.contains("{{MISSING:"), "{}", full.body_text);

    deliver(&state, &Mailer::DryRun, log[0].id).await.unwrap();
    assert_eq!(
        emails::find(&pool, log[0].id)
            .await
            .unwrap()
            .unwrap()
            .status,
        EmailStatus::Sent
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn the_inbox_search_runs_on_the_server(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    manual_email(&pool, user.user_id, EmailStatus::Sent).await;

    let found = emails::list(
        &pool,
        &emails::EmailFilter {
            q: Some("teszt".into()),
            ..Default::default()
        },
        10,
        0,
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 1);

    let missing = emails::list(
        &pool,
        &emails::EmailFilter {
            q: Some("nincs-ilyen-targy".into()),
            ..Default::default()
        },
        10,
        0,
    )
    .await
    .unwrap();
    assert!(missing.is_empty());

    // Wildcards in the query are literal: searching for 50% matches 50%, not everything.
    let literal = emails::list(
        &pool,
        &emails::EmailFilter {
            q: Some("%".into()),
            ..Default::default()
        },
        10,
        0,
    )
    .await
    .unwrap();
    assert!(literal.is_empty());
}

// ── Logic audit round 2 (MAIL-L4, MAIL-L2) ──

#[sqlx::test(migrations = "./migrations")]
async fn a_dead_lettered_send_job_makes_the_email_failed_and_retryable(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let queued = manual_email(&pool, user.user_id, EmailStatus::Queued).await;
    let sending = manual_email(&pool, user.user_id, EmailStatus::Sending).await;

    autocrm::service::email::job_dead_lettered(&state, queued, "storage unreachable")
        .await
        .unwrap();
    autocrm::service::email::job_dead_lettered(&state, sending, "db down after SMTP")
        .await
        .unwrap();

    let failed = emails::find(&pool, queued).await.unwrap().unwrap();
    assert_eq!(failed.status, EmailStatus::Failed);
    assert!(failed.error.unwrap().contains("storage unreachable"));
    // The outcome of a row already handed to SMTP is unknown: never call it failed.
    assert_eq!(
        emails::find(&pool, sending).await.unwrap().unwrap().status,
        EmailStatus::NeedsReview
    );
    // Visible where failed mail is shown, and retryable through the normal path.
    let attention = emails::list(
        &pool,
        &emails::EmailFilter {
            needs_attention: true,
            ..Default::default()
        },
        10,
        0,
    )
    .await
    .unwrap();
    assert!(attention.iter().any(|m| m.id == queued));
    autocrm::service::email::retry(&state, queued)
        .await
        .unwrap();
    assert_eq!(
        emails::find(&pool, queued).await.unwrap().unwrap().status,
        EmailStatus::Queued
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_document_attached_and_embedded_is_not_reported_deleted(pool: PgPool) {
    use autocrm::domain::media::DocumentKind;
    use autocrm::repo::documents::{self, NewDocument, Owner};
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let mut conn = pool.acquire().await.unwrap();
    let (doc, _) = documents::insert(
        &mut conn,
        &NewDocument {
            owner: Owner::Order(order.id),
            vehicle_id: None,
            kind: DocumentKind::Design,
            filename: "kep.png",
            content_type: "image/png",
            storage_key: "test/kep.png",
            content_hash: &[9u8; 32],
            byte_size: 10,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    drop(conn);
    let id = manual_email(&pool, user.user_id, EmailStatus::Queued).await;
    sqlx::query("UPDATE email_messages SET attachments = $2 WHERE id = $1")
        .bind(id)
        .bind(json!([
            { "document_id": doc.id },
            { "document_id": doc.id, "content_id": format!("doc-{}", doc.id) }
        ]))
        .execute(&pool)
        .await
        .unwrap();

    // Storage is unreachable in tests, so delivery errors out after the lookup and the row
    // stays queued for the job to retry. What it must not do is claim the file was deleted.
    let _ = deliver(&state, &Mailer::DryRun, id).await;
    let email = emails::find(&pool, id).await.unwrap().unwrap();
    assert_ne!(
        email.error.as_deref(),
        Some("an attached document was deleted before sending")
    );
    assert_ne!(email.status, EmailStatus::Failed);
}
