//! Quote follow-ups: a quotation schedules the default sequence, due letters go out as
//! automatic mail, a decided lead gets no more, and the office can add or stop them.

mod common;

use autocrm::domain::role::Role;
use autocrm::repo::leads::LeadInput;
use autocrm::repo::{emails, followups};
use autocrm::service::auth::AuthUser;
use autocrm::service::email::triggers;
use autocrm::service::followups::send_due;
use autocrm::service::leads::{QuotationRecord, record_quotation};
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
        quoted_value_minor: None,
        currency: None,
        quote_valid_until: None,
    }
}

/// The quotation PDF the sales manager uploaded: a document row without bytes, which
/// recording never reads.
async fn quote_pdf(pool: &PgPool, lead_id: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO documents (lead_id, kind, filename, content_type, storage_key, content_hash, byte_size)
         VALUES ($1, 'other', 'ajanlat.pdf', 'application/pdf', 'test/fake',
                 digest($1::text, 'sha256'), 10)
         RETURNING id",
    )
    .bind(lead_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn quotation(document_id: i64, steps: Option<Vec<i64>>) -> QuotationRecord {
    QuotationRecord {
        document_id,
        quoted_value_minor: Some(127_000_000),
        currency: Some(autocrm::domain::money::Currency::HUF),
        quote_valid_until: None,
        sent_on: None,
        followup_step_ids: steps,
    }
}

async fn setup(pool: &PgPool, email: Option<&str>) -> (autocrm::AppState, AuthUser, i64) {
    let state = common::state(pool.clone());
    let user = common::user(pool, Role::Office).await;
    sqlx::query("UPDATE settings SET automatic_email_enabled = true")
        .execute(pool)
        .await
        .unwrap();
    let lead = autocrm::service::leads::create(
        &state.db,
        &user,
        lead(email),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();
    (state, user, lead.id)
}

/// Makes the lead's first scheduled follow-up due now.
async fn make_first_due(pool: &PgPool, lead_id: i64) -> i64 {
    sqlx::query_scalar(
        "UPDATE lead_followups SET due_at = now() - interval '1 minute'
         WHERE id = (SELECT id FROM lead_followups WHERE lead_id = $1 AND status = 'scheduled'
                     ORDER BY due_at LIMIT 1)
         RETURNING id",
    )
    .bind(lead_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_quotation_schedules_the_sequence_and_due_letters_go_out(pool: PgPool) {
    let (state, user, lead_id) = setup(&pool, Some("vevo@example.hu")).await;
    let pdf = quote_pdf(&pool, lead_id).await;
    record_quotation(&state, &user, lead_id, &quotation(pdf, None))
        .await
        .unwrap();

    let rows = followups::for_lead(&pool, lead_id).await.unwrap();
    let labels: Vec<&str> = rows.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(labels, vec!["1 hét", "2 hét", "1 hónap"]);
    let days = |f: &followups::Followup| (f.due_at - chrono::Utc::now()).num_days();
    assert!((6..=7).contains(&days(&rows[0])));
    assert!((29..=30).contains(&days(&rows[2])));

    // Nothing is due yet.
    assert_eq!(send_due(&state).await.unwrap(), 0);

    let first = make_first_due(&pool, lead_id).await;
    assert_eq!(send_due(&state).await.unwrap(), 1);
    let row = followups::find(&pool, first).await.unwrap().unwrap();
    assert_eq!(row.status, "sent");
    let mail = emails::find(&pool, row.email_id.unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(mail.trigger, triggers::QUOTE_FOLLOWUP);
    assert_eq!(mail.to_address, "vevo@example.hu");
    assert!(mail.subject.contains("Hűtős Sprinter"), "{}", mail.subject);
    assert!(!mail.body_text.contains("MISSING"), "{}", mail.body_text);
    // Sending twice does not send twice.
    assert_eq!(send_due(&state).await.unwrap(), 0);

    // A new quotation replaces what is still waiting, here with only the 2-week step.
    let two_weeks: i64 = sqlx::query_scalar("SELECT id FROM followup_steps WHERE delay_days = 14")
        .fetch_one(&pool)
        .await
        .unwrap();
    record_quotation(
        &state,
        &user,
        lead_id,
        &quotation(pdf, Some(vec![two_weeks])),
    )
    .await
    .unwrap();
    let rows = followups::for_lead(&pool, lead_id).await.unwrap();
    let scheduled: Vec<&str> = rows
        .iter()
        .filter(|f| f.status == "scheduled")
        .map(|f| f.label.as_str())
        .collect();
    assert_eq!(scheduled, vec!["2 hét"]);
    assert_eq!(rows.iter().filter(|f| f.status == "cancelled").count(), 2);
    assert_eq!(rows.iter().filter(|f| f.status == "sent").count(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_decided_lead_gets_no_more_letters(pool: PgPool) {
    let (state, user, lead_id) = setup(&pool, Some("vevo@example.hu")).await;
    let pdf = quote_pdf(&pool, lead_id).await;
    record_quotation(&state, &user, lead_id, &quotation(pdf, None))
        .await
        .unwrap();

    // Lost: whatever is waiting is cancelled on the spot.
    autocrm::service::stages::change_lead_stage(
        &state.db,
        &user,
        lead_id,
        "lost",
        Some("Mástól vásárolt"),
    )
    .await
    .unwrap();
    let rows = followups::for_lead(&pool, lead_id).await.unwrap();
    assert!(rows.iter().all(|f| f.status == "cancelled"), "{rows:?}");
    assert_eq!(rows[0].note.as_deref(), Some("a lead lezárult"));

    // And a letter scheduled behind the stage's back is skipped when due, not sent.
    sqlx::query(
        "UPDATE lead_followups SET status = 'scheduled', due_at = now() WHERE lead_id = $1",
    )
    .bind(lead_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(send_due(&state).await.unwrap(), 0);
    let rows = followups::for_lead(&pool, lead_id).await.unwrap();
    assert!(rows.iter().all(|f| f.status == "skipped"), "{rows:?}");
}

#[sqlx::test(migrations = "./migrations")]
async fn no_address_is_a_skip_with_a_reason_and_the_kill_switch_holds_everything(pool: PgPool) {
    let (state, user, lead_id) = setup(&pool, None).await;
    // Recording a quotation with follow-ups needs an address; schedule by hand instead.
    let _ = user;
    followups::schedule(
        &pool,
        lead_id,
        "1 hét",
        "quote_followup_1",
        chrono::Utc::now(),
        None,
    )
    .await
    .unwrap();

    sqlx::query("UPDATE settings SET automatic_email_enabled = false")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(send_due(&state).await.unwrap(), 0);
    assert_eq!(
        followups::for_lead(&pool, lead_id).await.unwrap()[0].status,
        "scheduled"
    );

    sqlx::query("UPDATE settings SET automatic_email_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(send_due(&state).await.unwrap(), 0);
    let row = &followups::for_lead(&pool, lead_id).await.unwrap()[0];
    assert_eq!(row.status, "skipped");
    assert_eq!(row.note.as_deref(), Some("nincs e-mail cím a leadhez"));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_recorded_quotation_files_the_pdf_sets_the_price_and_sends_nothing(pool: PgPool) {
    let (state, user, lead_id) = setup(&pool, Some("vevo@example.hu")).await;
    let pdf = quote_pdf(&pool, lead_id).await;
    let lead = record_quotation(&state, &user, lead_id, &quotation(pdf, Some(vec![])))
        .await
        .unwrap();
    assert_eq!(lead.quoted_value_minor, Some(127_000_000));
    assert_eq!(lead.currency.as_deref(), Some("HUF"));

    let kind: String = sqlx::query_scalar("SELECT kind::text FROM documents WHERE id = $1")
        .bind(pdf)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kind, "quote");
    // The customer got the quotation from a person's own mailbox: nothing is queued here.
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM email_messages WHERE lead_id = $1")
        .bind(lead_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
    assert!(
        followups::for_lead(&pool, lead_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_quotation_from_another_lead_or_without_an_address_is_refused(pool: PgPool) {
    let (state, user, lead_id) = setup(&pool, None).await;
    let pdf = quote_pdf(&pool, lead_id).await;

    // Follow-ups with nowhere to go are refused up front, naming the lead.
    let err = record_quotation(&state, &user, lead_id, &quotation(pdf, None))
        .await
        .unwrap_err();
    assert!(
        err.to_string()
            .contains(&format!("lead #{lead_id} has no email")),
        "{err}"
    );
    // Without follow-ups the same quotation is fine.
    record_quotation(&state, &user, lead_id, &quotation(pdf, Some(vec![])))
        .await
        .unwrap();

    let (_, _, other) = setup(&pool, Some("masik@example.hu")).await;
    let err = record_quotation(&state, &user, other, &quotation(pdf, None))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("document of this lead"), "{err}");

    let mut future = quotation(pdf, Some(vec![]));
    future.sent_on = Some(chrono::Utc::now().date_naive() + chrono::TimeDelta::days(3));
    let err = record_quotation(&state, &user, lead_id, &future)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("future"), "{err}");
}
