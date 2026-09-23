//! Newsletter and quotation mail, without touching the network.
//!
//! The blast inserts one row with everyone in BCC; the quotation inserts one row with the
//! hero band and the lead's PDF. Delivery itself is the worker's job and is covered by
//! the email tests — here the queueing, the audience math and the HTML are asserted.

mod common;

use autocrm::domain::role::Role;
use autocrm::repo::emails::{self, EmailFilter};
use autocrm::repo::leads::LeadInput;
use autocrm::repo::{leads, newsletter};
use autocrm::service::email::{
    self, ComposeRequest, NewsletterRequest, QuotationRequest, triggers,
};

async fn office(pool: &sqlx::PgPool) -> autocrm::service::auth::AuthUser {
    common::user(pool, Role::Office).await
}

fn lead_with_quote(contact_email: &str) -> LeadInput {
    LeadInput {
        title: "Harom Sprinter hutose".into(),
        partner_id: None,
        contact_id: None,
        contact_name: Some("Teszt Janos".into()),
        contact_email: Some(contact_email.into()),
        contact_phone: None,
        source: None,
        description: None,
        assigned_to: None,
        quoted_value_minor: Some(1_270_000_00),
        currency: Some("HUF".into()),
        quote_valid_until: Some(chrono::NaiveDate::from_ymd_opt(2026, 12, 31).unwrap()),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn subscribing_is_idempotent_and_case_insensitive(pool: sqlx::PgPool) {
    let state = common::state(pool);
    let a = newsletter::subscribe(&state.db, "Janos@Example.HU", "Janos", "website")
        .await
        .unwrap();
    assert_eq!(a.email, "janos@example.hu");
    let b = newsletter::subscribe(&state.db, "janos@example.hu", "Janos Uj", "website")
        .await
        .unwrap();
    assert_eq!(a.id, b.id);
    assert_eq!(b.name, "Janos Uj");
    assert_eq!(newsletter::count_active(&state.db).await.unwrap(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn unsubscribing_keeps_the_row(pool: sqlx::PgPool) {
    let state = common::state(pool);
    let sub = newsletter::subscribe(&state.db, "olvaso@example.hu", "", "website")
        .await
        .unwrap();
    let token: String = sqlx::query_scalar!(
        "SELECT unsubscribe_token FROM newsletter_subscriptions WHERE id = $1",
        sub.id
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    let gone = newsletter::unsubscribe_by_token(&state.db, &token)
        .await
        .unwrap()
        .unwrap();
    assert!(gone.unsubscribed_at.is_some());
    assert_eq!(newsletter::count_active(&state.db).await.unwrap(), 0);
    // Clicking the link twice changes nothing the second time.
    assert!(
        newsletter::unsubscribe_by_token(&state.db, &token)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_blast_is_one_row_with_everyone_in_bcc(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    newsletter::subscribe(&state.db, "a@example.hu", "", "website")
        .await
        .unwrap();
    newsletter::subscribe(&state.db, "b@example.hu", "", "office")
        .await
        .unwrap();

    let (id, count) = email::send_newsletter(
        &state,
        &user,
        &NewsletterRequest {
            subject: "Tavaszi akcio".into(),
            body: "# Ujdonsag\n\nJo hir mindenkinek.".into(),
            body_markdown: true,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(count, 2);

    let mail = emails::find(&state.db, id).await.unwrap().unwrap();
    assert_eq!(mail.trigger, triggers::NEWSLETTER);
    assert_eq!(mail.bcc.len(), 2);
    assert!(mail.bcc.contains(&"a@example.hu".to_string()));
    // Markdown rendered, branded layout kept, unsubscribe footer appended.
    assert!(mail.body_html.contains("<h1>Ujdonsag</h1>"), "{}", mail.body_html);
    assert!(mail.body_html.contains("AUTOTHERM"));
    assert!(mail.body_html.contains("/hu/newsletter/unsubscribe"));
    assert!(mail.body_text.contains("Leiratkoz"), "{}", mail.body_text);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_blast_to_nobody_is_refused(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    let err = email::send_newsletter(
        &state,
        &user,
        &NewsletterRequest {
            subject: "Semmi".into(),
            body: "Nincs kinek.".into(),
            body_markdown: false,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("nobody to send to"), "{err}");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_blast_with_variables_is_refused(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    newsletter::subscribe(&state.db, "a@example.hu", "", "website")
        .await
        .unwrap();
    let err = email::send_newsletter(
        &state,
        &user,
        &NewsletterRequest {
            subject: "Hello {{partner.name}}".into(),
            body: "x".into(),
            body_markdown: false,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("no recipient"), "{err}");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_quotation_shouts_and_quotes_the_lead(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    let lead = leads::insert(&state.db, &lead_with_quote("vevo@example.hu"), user.user_id)
        .await
        .unwrap();

    let id = email::send_quotation(
        &state,
        &user,
        lead.id,
        &QuotationRequest {
            subject: None,
            body: None,
            hero: None,
            body_markdown: false,
            attachment_document_ids: vec![],
        },
    )
    .await
    .unwrap();

    let mail = emails::find(&state.db, id).await.unwrap().unwrap();
    assert_eq!(mail.to_address, "vevo@example.hu");
    assert_eq!(mail.trigger, triggers::QUOTATION);
    assert_eq!(mail.subject, "Árajánlatunk: Harom Sprinter hutose");
    assert!(mail.body_html.contains("Megjött az Autotherm árajánlatod!"));
    assert!(mail.body_html.contains("background-color:#b91c1c"));
    assert!(mail.body_text.contains("1270000.00 HUF"), "{}", mail.body_text);
    assert!(mail.body_text.contains("2026.12.31."));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_quotation_without_an_address_names_the_lead(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    let mut input = lead_with_quote("x@y.zz");
    input.contact_email = None;
    let lead = leads::insert(&state.db, &input, user.user_id)
        .await
        .unwrap();

    let err = email::send_quotation(
        &state,
        &user,
        lead.id,
        &QuotationRequest {
            subject: None,
            body: None,
            hero: None,
            body_markdown: false,
            attachment_document_ids: vec![],
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains(&format!("lead #{} has no email", lead.id)), "{err}");
}

#[sqlx::test(migrations = "./migrations")]
async fn manual_markdown_compose_renders_html(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    let preview = email::preview(
        &state,
        &user,
        &ComposeRequest {
            order_id: None,
            lead_id: None,
            partner_id: None,
            to: "vevo@example.hu".into(),
            cc: vec![],
            template_key: None,
            subject: Some("Hir".into()),
            body: Some("# Cim\n\nSzoveg **felkover**.".into()),
            body_markdown: true,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap();
    assert!(preview.body_html.contains("<h1>Cim</h1>"), "{}", preview.body_html);
    assert!(preview.body_html.contains("<strong>felkover</strong>"));
    assert_eq!(preview.body_text, "# Cim\n\nSzoveg **felkover**.");
}

/// A document row without bytes: queueing and preview never touch storage, so the
/// rewrite is testable without MinIO. Filed against a lead, like a real quotation PDF.
async fn doc_row(pool: &sqlx::PgPool, user_id: i64, filename: &str, content_type: &str) -> i64 {
    let lead = leads::insert(pool, &lead_with_quote("doksi@example.hu"), user_id)
        .await
        .unwrap();
    sqlx::query_scalar!(
        "INSERT INTO documents (lead_id, kind, filename, content_type, storage_key, content_hash, byte_size)
         VALUES ($1, 'other', $2, $3, 'test/fake', digest($2, 'sha256'), 10) RETURNING id",
        lead.id,
        filename,
        content_type
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_blast_carries_attachments_hero_and_inline_images(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    newsletter::subscribe(&state.db, "a@example.hu", "", "website")
        .await
        .unwrap();
    let pdf = doc_row(&pool, user.user_id, "arlista.pdf", "application/pdf").await;
    let img = doc_row(&pool, user.user_id, "akcio.png", "image/png").await;

    let (id, _) = email::send_newsletter(
        &state,
        &user,
        &NewsletterRequest {
            subject: "Arajanlat mindenkinek".into(),
            body: format!("# Ujdonsag\n\nNezd meg: ![akcio](doc:{img})"),
            body_markdown: true,
            hero: Some("Arajánlatok mindenkinek!".into()),
            attachment_document_ids: vec![pdf],
            embed_document_ids: vec![img],
        },
    )
    .await
    .unwrap();

    let mail = emails::find(&state.db, id).await.unwrap().unwrap();
    assert!(mail.body_html.contains("Arajánlatok mindenkinek!"));
    assert!(mail.body_html.contains("background-color:#b91c1c"));
    assert!(
        mail.body_html.contains(&format!("src=\"cid:doc-{img}\"")),
        "{}",
        mail.body_html
    );
    assert!(!mail.body_html.contains("\"doc:"), "{}", mail.body_html);
    // The text part quotes the filename, not the pointer.
    assert!(mail.body_text.contains("akcio.png"), "{}", mail.body_text);
    assert!(!mail.body_text.contains("doc:"), "{}", mail.body_text);

    // Queue-time refs: the PDF waits for delivery, the image already knows its CID.
    // (Modes are stamped at delivery; here both are still empty.)
    let attached: Vec<(i64, Option<String>)> = mail
        .attachments
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["document_id"].as_i64().unwrap(),
                a.get("content_id").and_then(|c| c.as_str()).map(str::to_string),
            )
        })
        .collect();
    assert!(attached.contains(&(pdf, None)), "{attached:?}");
    assert!(
        attached.contains(&(img, Some(format!("doc-{img}")))),
        "{attached:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_dangling_embed_reference_refuses_the_blast(pool: sqlx::PgPool) {
    let state = common::state(pool.clone());
    let user = office(&pool).await;
    newsletter::subscribe(&state.db, "a@example.hu", "", "website")
        .await
        .unwrap();
    let err = email::send_newsletter(
        &state,
        &user,
        &NewsletterRequest {
            subject: "Hiba".into(),
            body: "Nezd: ![kep](doc:99999)".into(),
            body_markdown: true,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("doc:99999"), "{err}");
}
