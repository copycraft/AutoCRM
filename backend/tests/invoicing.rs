//! Invoicing end to end against the NAV sidecar running in MOCK_MODE.
//!
//! The sidecar's mock is not a stub: it speaks the real XML, verifies the request
//! signature the way NAV does, and decides an invoice's fate with the same validator NAV's
//! rules are written from. So these tests exercise the whole path — snapshot, number,
//! report, verdict, PDF, letter — without a technical user and without touching the tax
//! authority.
//!
//! Tests that need the sidecar skip loudly when it is not built; the ones that need an
//! object store skip when MinIO is not up. See `common::start_sidecar`.

mod common;

use rust_decimal::Decimal;
use sqlx::PgPool;

use autocrm::domain::invoice::{InvoiceKind, InvoiceStatus};
use autocrm::domain::media::DocumentKind;
use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::emails::{self, EmailFilter};
use autocrm::repo::{documents, invoices};
use autocrm::service::invoicing::{
    self, AnnulRequest, IssueRequest, ProformaRequest, StornoRequest, SubmitPayload,
};
use autocrm::service::orders::NewItem;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

/// One line: 1 × 1 000 000.00 HUF net, which is 270 000.00 VAT at the default rate.
fn items() -> Vec<NewItem> {
    vec![NewItem {
        description: "Hűtőfelépítmény beépítése".into(),
        quantity: dec("1"),
        unit_price: 100_000_000,
    }]
}

async fn emails_for(pool: &PgPool, order_id: i64) -> Vec<emails::EmailSummary> {
    emails::list(
        pool,
        &EmailFilter {
            order_id: Some(order_id),
            lead_id: None,
            partner_id: None,
            status: None,
            needs_attention: false,
            q: None,
        },
        50,
        0,
    )
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn an_invoice_is_reported_to_nav_and_sent_to_the_customer(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    // The request only queues the work: NAV is asynchronous, and so is this.
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    assert_eq!(queued.status, InvoiceStatus::Submitting);
    assert_eq!(queued.kind, InvoiceKind::Invoice);
    assert!(queued.number.starts_with("AT"), "number was {}", queued.number);
    assert_eq!(queued.net_amount, 100_000_000);
    assert_eq!(queued.vat_amount, 27_000_000);
    assert_eq!(queued.gross_amount, 127_000_000);
    assert!(queued.nav_transaction_id.is_none());

    // The line snapshot is the invoice's own, carrying the VAT rate the order has none of.
    let lines = invoices::lines_for(&pool, queued.id).await.unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].vat_rate, dec("0.2700"));
    assert_eq!(lines[0].net_amount, 100_000_000);

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: true,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let reported = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(reported.status, InvoiceStatus::Issued);
    assert_eq!(reported.nav_status.as_deref(), Some("DONE"));
    assert!(
        reported.nav_transaction_id.is_some(),
        "NAV's transaction id is the handle for any later support question"
    );
    assert!(reported.nav_error_code.is_none());

    let letters = emails_for(&pool, order.id).await;
    let invoice_letter = letters
        .iter()
        .find(|e| e.template_key.as_deref() == Some("invoice_issued"))
        .expect("the customer is sent the invoice");
    assert_eq!(invoice_letter.to_address, "vevo@example.hu");
    assert!(
        invoice_letter.subject.contains(&reported.number),
        "subject was {}",
        invoice_letter.subject
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_rejected_invoice_keeps_navs_own_fault_code(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    // Reporting as a taxpayer the technical user is not: NAV refuses a report filed on
    // someone else's behalf, and the mock refuses it for the same reason.
    let state = common::state_with_nav(pool.clone(), &sidecar.url, "12345678");
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    // A rejection is an answer, not a failure: the job completes.
    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: true,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let rejected = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(rejected.status, InvoiceStatus::Rejected);
    assert!(
        rejected.nav_error_code.is_some(),
        "the fault code must survive to the row; message was {:?}",
        rejected.nav_message
    );
    assert!(
        rejected.nav_message.as_deref().unwrap_or_default().len() > 10,
        "NAV's own words are kept, not replaced by 'failed'"
    );
    let messages = rejected.nav_messages.as_array().expect("messages array");
    assert!(!messages.is_empty(), "every finding is kept");

    // Nothing was sent: there is no invoice to send.
    let letters = emails_for(&pool, order.id).await;
    assert!(
        !letters
            .iter()
            .any(|e| e.template_key.as_deref() == Some("invoice_issued")),
        "a rejected invoice is not mailed to the customer"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_storno_reverses_the_invoice_and_tells_the_customer(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let invoice = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: invoice.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let storno = invoicing::create_storno(
        &state,
        &user,
        invoice.id,
        &StornoRequest {
            issue_date: None,
            send_email: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(storno.kind, InvoiceKind::Storno);
    assert_eq!(storno.original_invoice_id, Some(invoice.id));
    assert_eq!(storno.gross_amount, -invoice.gross_amount);
    assert_ne!(storno.number, invoice.number, "a storno is its own document");

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: storno.id,
            send_email: true,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let reported = invoices::find(&pool, storno.id).await.unwrap().unwrap();
    assert_eq!(reported.status, InvoiceStatus::Issued);
    assert!(reported.nav_transaction_id.is_some());

    let original = invoices::find(&pool, invoice.id).await.unwrap().unwrap();
    assert_eq!(
        original.status,
        InvoiceStatus::Stornoed,
        "the original stops being live once the cancellation is reported"
    );

    let letters = emails_for(&pool, order.id).await;
    let storno_letter = letters
        .iter()
        .find(|e| e.template_key.as_deref() == Some("invoice_stornoed"))
        .expect("the customer is told the invoice was cancelled");
    assert!(storno_letter.subject.contains(&storno.number));

    // And the order can be invoiced again: the live one is gone.
    invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .expect("a stornoed order can be re-invoiced");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_annulment_is_recorded_and_the_customer_is_notified(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let admin = common::user(&pool, Role::Admin).await;
    let order = common::invoiceable_order(&pool, &admin, "HUF", items()).await;

    let invoice = invoicing::create_invoice(&state, &admin, order.id, &IssueRequest::default())
        .await
        .unwrap();
    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: invoice.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let request = AnnulRequest {
        code: "ERRATIC_DATA".into(),
        reason: "A vevő adószáma hibásan került be".into(),
    };
    invoicing::annul_invoice(&state, &admin, invoice.id, &request)
        .await
        .unwrap();
    invoicing::annul_job(
        &state,
        &invoicing::AnnulPayload {
            invoice_id: invoice.id,
            code: request.code.clone(),
            reason: request.reason.clone(),
            send_email: true,
        },
    )
    .await
    .unwrap();

    let annulled = invoices::find(&pool, invoice.id).await.unwrap().unwrap();
    assert_eq!(annulled.status, InvoiceStatus::Annulled);
    assert!(annulled.annulment_transaction_id.is_some());
    assert_eq!(annulled.annulment_code.as_deref(), Some("ERRATIC_DATA"));
    assert!(annulled.annulled_at.is_some());

    let letters = emails_for(&pool, order.id).await;
    assert!(
        letters
            .iter()
            .any(|e| e.template_key.as_deref() == Some("invoice_annulled")),
        "an annulment is a short notice, and it is sent"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn an_annulment_needs_a_code_nav_knows(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let admin = common::user(&pool, Role::Admin).await;
    let order = common::invoiceable_order(&pool, &admin, "HUF", items()).await;
    let invoice = invoicing::create_invoice(&state, &admin, order.id, &IssueRequest::default())
        .await
        .unwrap();

    let error = invoicing::annul_invoice(
        &state,
        &admin,
        invoice.id,
        &AnnulRequest {
            code: "BECAUSE_I_SAY_SO".into(),
            reason: "nope".into(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(error, AppError::Validation(_)));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_proforma_is_rendered_and_never_reported(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    if !common::storage_available(&state).await {
        return;
    }
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let created = invoicing::create_proforma(
        &state,
        &user,
        order.id,
        &ProformaRequest {
            note: Some("Kérjük 8 napon belül átutalni.".into()),
            send_email: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(
        created.proforma.number.starts_with("DB"),
        "a proforma draws from its own series, never an invoice number: {}",
        created.proforma.number
    );
    assert_eq!(created.proforma.gross_amount, 127_000_000);

    // It is a document of the order, tagged as what it is.
    let document = documents::find(&pool, created.proforma.document_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(document.kind, DocumentKind::Proforma);
    assert_eq!(document.content_type, "application/pdf");
    assert!(document.byte_size > 1000, "a rendered PDF has substance");
    assert_eq!(document.order_id, Some(order.id));

    // And nothing about it is an invoice: no row, no status, no transaction.
    assert!(
        invoices::list_for_order(&pool, order.id).await.unwrap().is_empty(),
        "a proforma is not an invoice and must not create one"
    );

    let letters = emails_for(&pool, order.id).await;
    let proforma_letter = letters
        .iter()
        .find(|e| e.template_key.as_deref() == Some("proforma_created"))
        .expect("the proforma is sent to the customer");
    let full = emails::find(&pool, proforma_letter.id).await.unwrap().unwrap();
    assert!(
        full.body_text.contains("nem adóügyi bizonylat"),
        "the letter must say a díjbekérő is not a tax document"
    );
    let attachments = full.attachments.as_array().expect("attachments array");
    assert_eq!(attachments.len(), 1, "the PDF travels with the letter");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_invoice_pdf_is_fetched_from_nav_and_filed_with_the_order(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    if !common::storage_available(&state).await {
        return;
    }
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let invoice = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: invoice.id,
            send_email: true,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let reported = invoices::find(&pool, invoice.id).await.unwrap().unwrap();
    let document_id = reported.document_id.expect("the PDF is stored");
    let document = documents::find(&pool, document_id).await.unwrap().unwrap();
    assert_eq!(document.kind, DocumentKind::Invoice);
    assert_eq!(document.filename, format!("{}.pdf", reported.number));
    assert!(document.byte_size > 1000);

    // It really is the PDF the sidecar rendered.
    let bytes = state.storage.get_bytes(&document.storage_key).await.unwrap();
    assert_eq!(&bytes[..5], b"%PDF-");

    let letters = emails_for(&pool, order.id).await;
    let letter = letters
        .iter()
        .find(|e| e.template_key.as_deref() == Some("invoice_issued"))
        .unwrap();
    let full = emails::find(&pool, letter.id).await.unwrap().unwrap();
    let attachments = full.attachments.as_array().expect("attachments array");
    assert_eq!(attachments.len(), 1, "the invoice travels with the letter");
    assert_eq!(attachments[0]["document_id"], document_id);
}

#[sqlx::test(migrations = "./migrations")]
async fn one_live_invoice_per_order(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let first = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();

    // While it is still being reported, nothing else may be.
    let error = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict { code, .. } if code == "invoice_in_flight"));

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: first.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let error = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict { code, .. } if code == "invoice_exists"));
}

#[sqlx::test(migrations = "./migrations")]
async fn an_order_with_no_lines_has_nothing_to_invoice(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", vec![]).await;

    let error = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Rule { code, .. } if code == "no_items"));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_partner_without_an_invoiceable_address_is_refused_by_name(pool: PgPool) {
    let Some(sidecar) = common::start_sidecar() else {
        return;
    };
    let state = common::state_with_nav(
        pool.clone(),
        &sidecar.url,
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let user = common::user(&pool, Role::Office).await;
    // `common::order` builds a partner with no address at all.
    let order = common::order(&pool, &user, "HUF", items()).await;

    let error = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap_err();
    match error {
        AppError::Rule { code, message } => {
            assert_eq!(code, "invoice_data_missing");
            assert!(
                message.contains("postal code"),
                "the message must name the field to fix, was: {message}"
            );
        }
        other => panic!("expected a rule error naming the missing field, got {other:?}"),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn invoicing_says_so_when_it_is_not_configured(pool: PgPool) {
    // No sidecar needed: this is the state every deployment starts in.
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    let error = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap_err();
    match error {
        AppError::Rule { code, message } => {
            assert_eq!(code, "invoicing_not_configured");
            assert!(message.contains("NAV_SIDECAR_URL"));
        }
        other => panic!("expected a configuration rule error, got {other:?}"),
    }
}
