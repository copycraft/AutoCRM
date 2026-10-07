//! Quote follow-ups: a quotation schedules the default sequence, due letters go out as
//! automatic mail, a decided lead gets no more, and the office can add or stop them.

mod common;

use autocrm::domain::role::Role;
use autocrm::repo::leads::LeadInput;
use autocrm::repo::{emails, followups};
use autocrm::service::auth::AuthUser;
use autocrm::service::email::{self, QuotationRequest, triggers};
use autocrm::service::followups::send_due;
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

fn quotation(steps: Option<Vec<i64>>) -> QuotationRequest {
    QuotationRequest {
        subject: None,
        body: None,
        hero: None,
        body_markdown: false,
        attachment_document_ids: vec![],
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
    email::send_quotation(&state, &user, lead_id, &quotation(None))
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
    email::send_quotation(&state, &user, lead_id, &quotation(Some(vec![two_weeks])))
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
    email::send_quotation(&state, &user, lead_id, &quotation(None))
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
    // The quotation itself needs an address; schedule by hand instead.
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
