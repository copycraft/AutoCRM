//! Customer replies read from the mailbox, automatic payment reminders, lost reasons,
//! quote expiry alerts and tracked newsletter sends.

mod common;

use autocrm::domain::role::Role;
use autocrm::repo::leads::LeadInput;
use autocrm::repo::timeline::{self, TimelineEntity};
use autocrm::repo::{followups, invoices, lost_reasons, newsletter};
use autocrm::service::auth::AuthUser;
use autocrm::service::email::{self, QuotationRequest};
use autocrm::service::{mailbox, newsletter as sends, reminders};
use sqlx::PgPool;

fn lead(email: Option<&str>) -> LeadInput {
    LeadInput {
        title: "Hűtős Sprinter".into(),
        partner_id: None,
        contact_id: None,
        contact_name: Some("Kiss Péter".into()),
        contact_email: email.map(String::from),
        contact_phone: None,
        source: None,
        source_detail: None,
        description: None,
        assigned_to: None,
        quoted_value_minor: Some(50_000_000),
        currency: Some("HUF".into()),
        quote_valid_until: None,
    }
}

async fn setup(pool: &PgPool) -> (autocrm::AppState, AuthUser) {
    let state = common::state(pool.clone());
    let user = common::user(pool, Role::Office).await;
    sqlx::query("UPDATE settings SET automatic_email_enabled = true")
        .execute(pool)
        .await
        .unwrap();
    (state, user)
}

fn raw(from: &str, in_reply_to: Option<&str>, message_id: &str) -> Vec<u8> {
    let mut m = format!(
        "From: Kiss Péter <{from}>\r\nTo: sales@autotherm.hu\r\nSubject: Re: Árajánlat\r\n\
         Message-ID: <{message_id}>\r\nDate: Tue, 6 Oct 2026 10:00:00 +0200\r\n"
    );
    if let Some(id) = in_reply_to {
        m.push_str(&format!("In-Reply-To: <{id}>\r\nReferences: <{id}>\r\n"));
    }
    m.push_str("Content-Type: text/plain; charset=utf-8\r\n\r\nKöszönöm, megrendelném.\r\n");
    m.into_bytes()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_reply_to_our_quotation_stops_its_follow_ups_and_lands_on_the_history(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    let lead = autocrm::service::leads::create(
        &state.db,
        &user,
        lead(Some("vevo@example.hu")),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();
    email::send_quotation(
        &state,
        &user,
        lead.id,
        &QuotationRequest {
            subject: None,
            body: None,
            hero: None,
            body_markdown: false,
            attachment_document_ids: vec![],
            followup_step_ids: None,
        },
    )
    .await
    .unwrap();
    // The quotation as the worker would have left it once sent.
    let email_id: i64 = sqlx::query_scalar(
        "UPDATE email_messages SET status = 'sent', provider_id = '<email-77.abc@autotherm.hu>'
          WHERE lead_id = $1 RETURNING id",
    )
    .bind(lead.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(followups::for_lead(&pool, lead.id).await.unwrap().len(), 3);

    let raw = raw(
        "vevo@example.hu",
        Some("email-77.abc@autotherm.hu"),
        "r1@example.hu",
    );
    let stored = mailbox::ingest(&pool, &raw, "sales@autotherm.hu")
        .await
        .unwrap();
    assert!(stored.is_some());
    let (lead_id, reply_to): (Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT lead_id, reply_to_email_id FROM inbound_emails")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((lead_id, reply_to), (Some(lead.id), Some(email_id)));

    let rows = followups::for_lead(&pool, lead.id).await.unwrap();
    assert!(rows.iter().all(|f| f.status == "cancelled"), "{rows:?}");
    assert!(
        rows.iter()
            .all(|f| f.note.as_deref() == Some("az ügyfél válaszolt"))
    );

    let history = timeline::list(&pool, TimelineEntity::Lead, lead.id, 50)
        .await
        .unwrap();
    let received = history
        .iter()
        .find(|e| e.kind == "email" && e.action == "received")
        .expect("the reply is on the history");
    assert_eq!(received.text.as_deref(), Some("Re: Árajánlat"));
    assert!(received.detail.as_deref().unwrap().contains("megrendelném"));

    // The same message read again is stored once.
    assert!(
        mailbox::ingest(&pool, &raw, "sales@autotherm.hu")
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn replies_match_by_address_and_strangers_or_our_own_mail_are_left_alone(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    let lead = autocrm::service::leads::create(
        &state.db,
        &user,
        lead(Some("Vevo@Example.hu")),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();
    let by_address = raw("vevo@example.hu", None, "r2@example.hu");
    assert!(
        mailbox::ingest(&pool, &by_address, "sales@autotherm.hu")
            .await
            .unwrap()
            .is_some()
    );
    let matched: Option<i64> = sqlx::query_scalar("SELECT lead_id FROM inbound_emails")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(matched, Some(lead.id));

    let stranger = raw("spam@example.com", None, "r3@example.com");
    assert!(
        mailbox::ingest(&pool, &stranger, "sales@autotherm.hu")
            .await
            .unwrap()
            .is_none()
    );
    let own = raw("sales@autotherm.hu", None, "r4@autotherm.hu");
    assert!(
        mailbox::ingest(&pool, &own, "sales@autotherm.hu")
            .await
            .unwrap()
            .is_none()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM inbound_emails")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

async fn overdue_invoice(pool: &PgPool, user: &AuthUser, days_late: i32) -> i64 {
    let order = common::order(pool, user, "HUF", vec![]).await;
    sqlx::query_scalar(
        "INSERT INTO invoices (order_id, number, kind, status, currency, issue_date, delivery_date,
                               payment_date, net_amount, vat_amount, gross_amount, payment_method, issued_at)
         VALUES ($1, 'AT2026-' || $1, 'invoice', 'issued', 'HUF', current_date - 30, current_date - 30,
                 current_date - $2, 100000, 27000, 127000, 'TRANSFER', now())
         RETURNING id",
    )
    .bind(order.id)
    .bind(days_late)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn payment_reminders_go_once_per_step_and_only_the_latest_when_very_late(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    let two_weeks_late = overdue_invoice(&pool, &user, 20).await;
    let few_days_late = overdue_invoice(&pool, &user, 4).await;
    let not_yet = overdue_invoice(&pool, &user, 1).await;
    let switched_off = overdue_invoice(&pool, &user, 20).await;
    invoices::set_reminders_off(&pool, switched_off, true)
        .await
        .unwrap();

    let sent = reminders::send_payment_reminders(&state).await.unwrap();
    assert_eq!(sent, 2);
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT r.invoice_id, s.template_key, r.status FROM invoice_reminders r
           JOIN followup_steps s ON s.id = r.step_id ORDER BY r.invoice_id, s.delay_days",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (two_weeks_late, "invoice_overdue_1".into(), "skipped".into()),
            (two_weeks_late, "invoice_overdue_2".into(), "sent".into()),
            (few_days_late, "invoice_overdue_1".into(), "sent".into()),
        ]
    );
    let letters: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM email_messages WHERE trigger = 'invoice_reminder' AND to_address = 'partner@example.hu'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(letters, 2);

    // A rerun sends nothing new; a paid invoice is never chased.
    assert_eq!(reminders::send_payment_reminders(&state).await.unwrap(), 0);
    invoices::set_paid(&pool, not_yet, true).await.unwrap();
    sqlx::query("UPDATE invoices SET payment_date = current_date - 30 WHERE id = $1")
        .bind(not_yet)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(reminders::send_payment_reminders(&state).await.unwrap(), 0);

    let overdue = invoices::overdue(&pool, chrono::Utc::now().date_naive(), None, 50)
        .await
        .unwrap();
    assert!(
        overdue
            .iter()
            .any(|i| i.id == two_weeks_late && i.reminders_sent == 1)
    );
    assert!(!overdue.iter().any(|i| i.id == not_yet));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_quote_about_to_expire_notifies_its_owner_once(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    let mut input = lead(Some("vevo@example.hu"));
    input.assigned_to = Some(user.user_id);
    input.quote_valid_until = Some(
        autocrm::service::business_today(state.config.business_tz) + chrono::TimeDelta::days(3),
    );
    let lead = autocrm::service::leads::create(
        &state.db,
        &user,
        input,
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(reminders::quote_expiry_alerts(&state).await.unwrap(), 1);
    assert_eq!(reminders::quote_expiry_alerts(&state).await.unwrap(), 0);
    let (title, link): (String, Option<String>) = sqlx::query_as(
        "SELECT title, link FROM notifications WHERE user_id = $1 AND kind = 'quote_expiry'",
    )
    .bind(user.user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(title, "3 nap múlva lejár egy árajánlat");
    assert_eq!(link, Some(format!("/leads/{}", lead.id)));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_lost_reason_is_kept_on_the_lead_and_counted(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    let lead = autocrm::service::leads::create(
        &state.db,
        &user,
        lead(None),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();
    let reasons = lost_reasons::list(&pool, false).await.unwrap();
    let pricey = reasons.iter().find(|r| r.label == "Drága volt").unwrap();
    autocrm::service::stages::change_lead_stage(&state.db, &user, lead.id, "lost", None)
        .await
        .unwrap();
    lost_reasons::set_for_lead(&pool, lead.id, Some(pricey.id))
        .await
        .unwrap();
    assert_eq!(
        lost_reasons::for_lead(&pool, lead.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        pricey.id
    );
    let now = chrono::Utc::now();
    let breakdown = lost_reasons::breakdown(
        &pool,
        now - chrono::TimeDelta::days(1),
        now + chrono::TimeDelta::days(1),
    )
    .await
    .unwrap();
    assert_eq!(breakdown.len(), 1);
    assert_eq!(
        (breakdown[0].reason.as_str(), breakdown[0].leads),
        ("Drága volt", 1)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_tracked_newsletter_is_one_letter_per_reader_with_its_own_links(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    for addr in ["a@example.hu", "b@example.hu"] {
        newsletter::subscribe(&pool, addr, "", "office")
            .await
            .unwrap();
    }
    let compose = sends::Compose {
        subject: "Őszi akció".into(),
        body: "Nézze meg: https://autotherm.hu/akcio?x=1&y=2".into(),
        body_markdown: true,
        hero: None,
        tag_ids: vec![],
        attachment_document_ids: vec![],
        embed_document_ids: vec![],
        send_at: chrono::Utc::now() - chrono::TimeDelta::seconds(1),
        variants: vec![],
    };
    let (send_id, recipients) = sends::schedule(&state, &user, &compose).await.unwrap();
    assert_eq!(recipients, 2);
    assert_eq!(sends::dispatch(&state, send_id).await.unwrap(), 2);
    // Idempotent: a second run writes nothing.
    assert_eq!(sends::dispatch(&state, send_id).await.unwrap(), 0);

    let rows: Vec<(String, String, Vec<String>, String)> = sqlx::query_as(
        "SELECT to_address, tracking_token, bcc, body_html FROM email_messages
          WHERE newsletter_send_id = $1 ORDER BY to_address",
    )
    .bind(send_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].1, rows[1].1, "each reader has their own token");
    for (_, token, bcc, html) in &rows {
        assert!(bcc.is_empty());
        assert!(html.contains(&format!("track/open?token={token}")));
        assert!(html.contains(&format!("track/click?token={token}&url=")));
        assert!(html.contains("unsubscribe?token="));
    }

    // An open and a signed click are counted; a forged click records nothing.
    let token = &rows[0].1;
    assert!(newsletter::record_open(&pool, token).await.unwrap());
    let url = "https://autotherm.hu/akcio?x=1&y=2";
    assert!(sends::link_signature_ok(
        &state.config.upload_signing_key,
        token,
        url,
        &sends::link_signature(&state.config.upload_signing_key, token, url)
    ));
    newsletter::record_click(&pool, token, url).await.unwrap();
    let stats = newsletter::send_stats(&pool, send_id).await.unwrap();
    assert_eq!((stats.opened, stats.clicked), (1, 1));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_scheduled_newsletter_waits_and_a_cancelled_one_never_goes(pool: PgPool) {
    let (state, user) = setup(&pool).await;
    newsletter::subscribe(&pool, "a@example.hu", "", "office")
        .await
        .unwrap();
    let mut compose = sends::Compose {
        subject: "Később".into(),
        body: "Szöveg".into(),
        body_markdown: false,
        hero: None,
        tag_ids: vec![],
        attachment_document_ids: vec![],
        embed_document_ids: vec![],
        send_at: chrono::Utc::now() + chrono::TimeDelta::hours(2),
        variants: vec![],
    };
    let (later, _) = sends::schedule(&state, &user, &compose).await.unwrap();
    assert_eq!(
        sends::dispatch(&state, later).await.unwrap(),
        0,
        "not before its time"
    );

    compose.send_at = chrono::Utc::now() - chrono::TimeDelta::seconds(1);
    let (cancelled, _) = sends::schedule(&state, &user, &compose).await.unwrap();
    assert!(newsletter::cancel_send(&pool, cancelled).await.unwrap());
    assert_eq!(sends::dispatch(&state, cancelled).await.unwrap(), 0);
}
