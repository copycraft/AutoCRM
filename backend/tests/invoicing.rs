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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    // The request only queues the work: NAV is asynchronous, and so is this.
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    assert_eq!(queued.status, InvoiceStatus::Submitting);
    assert_eq!(queued.kind, InvoiceKind::Invoice);
    assert!(
        queued.number.starts_with("AT"),
        "number was {}",
        queued.number
    );
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
    assert_ne!(
        storno.number, invoice.number,
        "a storno is its own document"
    );

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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
        false,
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
        invoices::list_for_order(&pool, order.id)
            .await
            .unwrap()
            .is_empty(),
        "a proforma is not an invoice and must not create one"
    );

    let letters = emails_for(&pool, order.id).await;
    let proforma_letter = letters
        .iter()
        .find(|e| e.template_key.as_deref() == Some("proforma_created"))
        .expect("the proforma is sent to the customer");
    let full = emails::find(&pool, proforma_letter.id)
        .await
        .unwrap()
        .unwrap();
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
    let bytes = state
        .storage
        .get_bytes(&document.storage_key)
        .await
        .unwrap();
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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
    let state =
        common::state_with_nav(pool.clone(), &sidecar.url, common::MOCK_SUPPLIER_TAX_NUMBER);
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

// ── INV-L8: reconciling a duplicate-number answer ────────────────────────────
// The `nav_rejected` path cannot be reached through the mock sidecar (it settles
// everything it is given), so these tests stub the sidecar's HTTP surface directly:
// the CREATE is answered INVOICE_NUMBER_ALREADY_EXISTS — the first attempt timed out
// after NAV stored the report — while the read-back serves a document the test
// controls. No object store is needed: a missing PDF never fails a stored invoice.

use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone)]
struct StubDoc {
    number: String,
    issue_date: String,
    currency: String,
    net: String,
    vat: String,
    gross: String,
}

struct StubSidecar {
    url: String,
    doc: Arc<Mutex<Option<StubDoc>>>,
    /// Raw CREATE bodies, in arrival order: what the backend actually reported.
    created: Arc<Mutex<Vec<String>>>,
}

impl StubSidecar {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let doc: Arc<Mutex<Option<StubDoc>>> = Arc::new(Mutex::new(None));
        let created: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let served = doc.clone();
        let recorded = created.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let served = served.clone();
                let recorded = recorded.clone();
                tokio::spawn(async move {
                    // Read through the end of the headers, then any body: the CREATE
                    // body is what the reported document claims.
                    let mut head = Vec::new();
                    let mut byte = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") && head.len() < 65536 {
                        match socket.read(&mut byte).await {
                            Ok(0) | Err(_) => return,
                            Ok(_) => head.extend_from_slice(&byte),
                        }
                    }
                    let head = String::from_utf8_lossy(&head);
                    let mut lines = head.lines();
                    let mut parts = lines.next().unwrap_or("").split_whitespace();
                    let method = parts.next().unwrap_or("");
                    let path = parts.next().unwrap_or("");
                    let content_length = lines
                        .filter_map(|line| line.strip_prefix("content-length:"))
                        .filter_map(|v| v.trim().parse::<usize>().ok())
                        .next()
                        .unwrap_or(0);
                    // Header names arrive lowercase from reqwest; tolerate either.
                    let content_length = if content_length == 0 {
                        head.lines()
                            .filter_map(|line| line.strip_prefix("Content-Length:"))
                            .filter_map(|v| v.trim().parse::<usize>().ok())
                            .next()
                            .unwrap_or(0)
                    } else {
                        content_length
                    };
                    let mut body_bytes = vec![0u8; content_length.min(65536)];
                    let mut read = 0;
                    while read < body_bytes.len() {
                        match socket.read(&mut body_bytes[read..]).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => read += n,
                        }
                    }
                    let request_body = String::from_utf8_lossy(&body_bytes[..read]).into_owned();
                    let is_storno = method == "POST" && path.ends_with("/storno");
                    let (status, body) = if method == "POST" && path.ends_with("/annul") {
                        // NAV refusing a second filing of the same annulment.
                        (
                            "422 Unprocessable Entity",
                            r#"{"error":{"kind":"nav_error","message":"NAV rejected the request: ANNULMENT_IN_PROGRESS","navErrorCode":"ANNULMENT_IN_PROGRESS"}}"#
                                .to_string(),
                        )
                    } else if (method == "POST" && path == "/invoices") || is_storno {
                        recorded.lock().unwrap().push(request_body);
                        (
                            "422 Unprocessable Entity",
                            r#"{"error":{"kind":"nav_rejected","message":"NAV rejected the invoice: INVOICE_NUMBER_ALREADY_EXISTS: Invoice number already exists","messages":[{"source":"business","level":"ERROR","code":"INVOICE_NUMBER_ALREADY_EXISTS","message":"Invoice number already exists","path":"/InvoiceData/invoiceNumber"}]}}"#
                                .to_string(),
                        )
                    } else if method == "GET" && path.starts_with("/invoices/") {
                        match served.lock().unwrap().clone() {
                            Some(d) => (
                                "200 OK",
                                format!(
                                    r#"{{"invoiceNumber":"{}","issueDate":"{}","totals":{{"currency":"{}","net":"{}","vat":"{}","gross":"{}"}}}}"#,
                                    d.number, d.issue_date, d.currency, d.net, d.vat, d.gross
                                ),
                            ),
                            None => (
                                "404 Not Found",
                                r#"{"error":{"kind":"not_found","message":"NAV holds nothing under this number"}}"#
                                    .to_string(),
                            ),
                        }
                    } else {
                        (
                            "404 Not Found",
                            r#"{"error":{"kind":"not_found","message":"unknown stub path"}}"#
                                .to_string(),
                        )
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        StubSidecar { url, doc, created }
    }

    fn created_bodies(&self) -> Vec<String> {
        self.created.lock().unwrap().clone()
    }

    fn minor_to_decimal(minor: i64) -> String {
        format!("{}.{:02}", minor / 100, minor % 100)
    }

    fn serve(
        &self,
        number: &str,
        issue_date: &str,
        currency: &str,
        net: i64,
        vat: i64,
        gross: i64,
    ) {
        *self.doc.lock().unwrap() = Some(StubDoc {
            number: number.to_string(),
            issue_date: issue_date.to_string(),
            currency: currency.to_string(),
            net: Self::minor_to_decimal(net),
            vat: Self::minor_to_decimal(vat),
            gross: Self::minor_to_decimal(gross),
        });
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_retry_answered_duplicate_number_adopts_our_stored_report(pool: PgPool) {
    let stub = StubSidecar::start().await;
    let state = common::state_with_nav(pool.clone(), &stub.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    // NAV holds exactly what we sent: the timed-out first attempt stored it.
    stub.serve(
        &queued.number,
        &queued.issue_date.to_string(),
        &queued.currency,
        queued.net_amount,
        queued.vat_amount,
        queued.gross_amount,
    );

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let adopted = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(
        adopted.status,
        InvoiceStatus::Issued,
        "our stored report must be adopted, not rejected; message was {:?}",
        adopted.nav_message
    );
    assert_eq!(adopted.nav_status.as_deref(), Some("DONE"));
    assert!(
        adopted.nav_transaction_id.is_none(),
        "the timed-out attempt's transaction id is unknowable"
    );
    assert!(
        adopted
            .nav_message
            .as_deref()
            .unwrap_or_default()
            .contains("reconciled"),
        "the row must say why there is no transaction id"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_retry_is_rejected_when_nav_holds_a_different_document(pool: PgPool) {
    let stub = StubSidecar::start().await;
    let state = common::state_with_nav(pool.clone(), &stub.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    // Same number, someone else's totals: adopting it would bless a foreign document.
    stub.serve(
        &queued.number,
        &queued.issue_date.to_string(),
        &queued.currency,
        queued.net_amount,
        queued.vat_amount,
        queued.gross_amount + 1,
    );

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let refused = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(refused.status, InvoiceStatus::Rejected);
    let message = refused.nav_message.unwrap_or_default();
    assert!(
        message.contains("INVOICE_NUMBER_ALREADY_EXISTS"),
        "the duplicate answer stays on the row, was: {message}"
    );
    assert!(
        message.contains("not adopted"),
        "the row must say why it was not adopted, was: {message}"
    );
}

/// An issued invoice without a live sidecar: create it, then mark it reported by hand.
async fn issued_invoice(
    pool: &PgPool,
    state: &autocrm::AppState,
) -> autocrm::repo::invoices::Invoice {
    let user = common::user(pool, Role::Office).await;
    let order = common::invoiceable_order(pool, &user, "HUF", items()).await;
    let queued = invoicing::create_invoice(state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    sqlx::query("UPDATE invoices SET status = 'issued', issued_at = now() WHERE id = $1")
        .bind(queued.id)
        .execute(pool)
        .await
        .unwrap();
    invoices::find(pool, queued.id).await.unwrap().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_retried_storno_answered_duplicate_number_is_adopted(pool: PgPool) {
    let stub = StubSidecar::start().await;
    let state = common::state_with_nav(pool.clone(), &stub.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let user = common::user(&pool, Role::Office).await;
    let original = issued_invoice(&pool, &state).await;
    let storno = invoicing::create_storno(
        &state,
        &user,
        original.id,
        &StornoRequest {
            issue_date: None,
            send_email: false,
        },
    )
    .await
    .unwrap();
    // The timed-out first attempt stored the reversal: NAV holds it, negative totals.
    stub.serve(
        &storno.number,
        &storno.issue_date.to_string(),
        &storno.currency,
        storno.net_amount,
        storno.vat_amount,
        storno.gross_amount,
    );

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: storno.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let adopted = invoices::find(&pool, storno.id).await.unwrap().unwrap();
    assert_eq!(
        adopted.status,
        InvoiceStatus::Issued,
        "NAV holds our storno: adopt it, do not reject it; message was {:?}",
        adopted.nav_message
    );
    let original = invoices::find(&pool, original.id).await.unwrap().unwrap();
    assert_eq!(
        original.status,
        InvoiceStatus::Stornoed,
        "the original must read as reversed, as it is at NAV, or it could be stornoed twice"
    );
    let again = invoicing::create_storno(
        &state,
        &user,
        original.id,
        &StornoRequest {
            issue_date: None,
            send_email: false,
        },
    )
    .await;
    assert!(
        again.is_err(),
        "a second storno of a reversed invoice must be refused"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_refused_annulment_retry_says_the_first_may_have_landed(pool: PgPool) {
    let stub = StubSidecar::start().await;
    let state = common::state_with_nav(pool.clone(), &stub.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let invoice = issued_invoice(&pool, &state).await;
    let payload = invoicing::AnnulPayload {
        invoice_id: invoice.id,
        code: "ERRATIC_DATA".into(),
        reason: "Hibás vevőadat".into(),
        send_email: false,
    };

    // First attempt: a refusal is just a refusal.
    invoicing::annul_job(&state, &payload, false).await.unwrap();
    let first = invoices::find(&pool, invoice.id).await.unwrap().unwrap();
    assert_eq!(first.status, InvoiceStatus::Issued);
    assert!(!first.nav_message.unwrap_or_default().contains("portal"));

    // A retry after a timeout: the refusal may be NAV seeing our first filing.
    invoicing::annul_job(&state, &payload, true).await.unwrap();
    let retried = invoices::find(&pool, invoice.id).await.unwrap().unwrap();
    assert_eq!(retried.status, InvoiceStatus::Issued);
    let message = retried.nav_message.unwrap_or_default();
    assert!(
        message.contains("may already have been filed") && message.contains("Online Számla"),
        "was: {message}"
    );
}

// ── INV-L10: an unbuildable submit reaches a terminal state ──────────────────
// No sidecar is needed: breaking the partner's address fails the request before any
// HTTP happens, which is exactly the stuck case (the sidecar never sees it).

#[sqlx::test(migrations = "./migrations")]
async fn an_unbuildable_submit_is_rejected_when_the_queue_gives_up(pool: PgPool) {
    // No HTTP ever happens (the request cannot be built), so the sidecar address is
    // never called; any URL configures invoicing on.
    let state = common::state_with_nav(pool.clone(), "http://127.0.0.1:9/", "12345678");
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    // The partner is edited while the invoice is in flight: the address the report
    // needs is gone.
    sqlx::query("UPDATE partners SET postal_code = NULL, city = NULL, address_line = NULL")
        .execute(&pool)
        .await
        .unwrap();

    let error = invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap_err();
    assert!(
        format!("{error:#}").starts_with("invoice_unbuildable: "),
        "the dead-letter branch keys on this marker, got: {error:#}"
    );
    let stuck = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(stuck.status, InvoiceStatus::Submitting);

    // The queue gives up: the row must become terminal, with the reason on it.
    invoicing::job_dead_lettered_unbuildable(&state, queued.id, &format!("{error:#}"))
        .await
        .unwrap();
    let terminal = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(terminal.status, InvoiceStatus::Rejected);
    assert_eq!(
        terminal.nav_error_code.as_deref(),
        Some("unbuildable"),
        "a local code, not NAV's: NAV never saw this invoice"
    );
    let message = terminal.nav_message.unwrap_or_default();
    assert!(
        message.contains("nothing was sent to NAV"),
        "the accountant must see the number died unreported, was: {message}"
    );

    // And the order is free again: a fixed partner can be invoiced anew.
    sqlx::query(
        "UPDATE partners SET postal_code = '1117', city = 'Budapest', address_line = 'Kossuth utca 12' WHERE id = (SELECT partner_id FROM orders WHERE id = $1)",
    )
    .bind(order.id)
    .execute(&pool)
    .await
    .unwrap();
    let retry = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();
    assert_eq!(retry.status, InvoiceStatus::Submitting);
}

// ── INV-L9: fetching the missing PDF again ───────────────────────────────────
// The guards are deterministic without a sidecar: no HTTP happens before them.

#[sqlx::test(migrations = "./migrations")]
async fn refetching_a_pdf_needs_an_issued_invoice_missing_its_file(pool: PgPool) {
    use autocrm::error::AppError;
    // The guards run before any HTTP, so the sidecar address is never called.
    let state = common::state_with_nav(pool.clone(), "http://127.0.0.1:9/", "12345678");
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;
    let queued = invoicing::create_invoice(&state, &user, order.id, &IssueRequest::default())
        .await
        .unwrap();

    // Still being reported: there is no report to render yet.
    match invoicing::refetch_pdf(&state, queued.id).await.unwrap_err() {
        AppError::Rule { code, .. } => assert_eq!(code, "not_issued"),
        other => panic!("expected not_issued, got {other:?}"),
    }

    // Filed already: nothing missing.
    sqlx::query("UPDATE invoices SET status = 'issued'")
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let (document, _) = documents::insert(
        &mut conn,
        &documents::NewDocument {
            owner: documents::Owner::Order(order.id),
            vehicle_id: None,
            kind: DocumentKind::Design,
            filename: "AT2026-x.pdf",
            content_type: "application/pdf",
            storage_key: "test/AT2026-x.pdf",
            content_hash: &[7u8; 32],
            byte_size: 10,
            uploaded_by: Some(user.user_id),
            source_ref: None,
            previous_version_id: None,
            version: 1,
        },
    )
    .await
    .unwrap();
    drop(conn);
    sqlx::query("UPDATE invoices SET document_id = $1")
        .bind(document.id)
        .execute(&pool)
        .await
        .unwrap();
    match invoicing::refetch_pdf(&state, queued.id).await.unwrap_err() {
        AppError::Conflict { .. } => {}
        other => panic!("expected a conflict, got {other:?}"),
    }

    // Unknown invoice.
    assert!(matches!(
        invoicing::refetch_pdf(&state, 999_999_999)
            .await
            .unwrap_err(),
        AppError::NotFound(_)
    ));
}

// ── Cash vs transfer: one series, a stored method ────────────────────────────

fn issue_with_method(method: Option<&str>) -> IssueRequest {
    IssueRequest {
        payment_method: method.map(str::to_string),
        ..IssueRequest::default()
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn cash_and_transfer_invoices_share_one_series_with_a_stored_method(pool: PgPool) {
    // No HTTP happens at issue time, so any sidecar address configures invoicing on.
    let state = common::state_with_nav(pool.clone(), "http://127.0.0.1:9/", "12345678");
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;

    // Absent means a bank transfer.
    let transfer = invoicing::create_invoice(&state, &user, order.id, &issue_with_method(None))
        .await
        .unwrap();
    assert_eq!(transfer.payment_method, "TRANSFER");

    // Cash is stored as chosen — and on the same consecutive series.
    sqlx::query("UPDATE invoices SET status = 'rejected' WHERE id = $1")
        .bind(transfer.id)
        .execute(&pool)
        .await
        .unwrap();
    let cash = invoicing::create_invoice(&state, &user, order.id, &issue_with_method(Some("cash")))
        .await
        .unwrap();
    assert_eq!(cash.payment_method, "CASH");
    let cash_no: i32 = cash.number.rsplit('-').next().unwrap().parse().unwrap();
    let transfer_no: i32 = transfer.number.rsplit('-').next().unwrap().parse().unwrap();
    assert_eq!(cash_no, transfer_no + 1, "one series, not one per method");

    // Anything outside the closed set is refused before a number is drawn.
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM invoices")
        .fetch_one(&pool)
        .await
        .unwrap();
    let error =
        invoicing::create_invoice(&state, &user, order.id, &issue_with_method(Some("CHEQUE")))
            .await
            .unwrap_err();
    assert!(
        matches!(&error, AppError::Validation(m) if m.contains("payment method must be TRANSFER or CASH")),
        "got {error:?}",
    );
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM invoices")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after, "a refused method must not spend a number");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_cash_invoice_is_reported_as_cash(pool: PgPool) {
    let stub = StubSidecar::start().await;
    // The stub answers the CREATE with a duplicate number, so submit reconciles —
    // but only after sending the report it was asked to send.
    let state = common::state_with_nav(pool.clone(), &stub.url, common::MOCK_SUPPLIER_TAX_NUMBER);
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", items()).await;
    let queued =
        invoicing::create_invoice(&state, &user, order.id, &issue_with_method(Some("CASH")))
            .await
            .unwrap();
    // NAV holds exactly this cash document (its totals are the stored ones).
    stub.serve(
        &queued.number,
        &queued.issue_date.to_string(),
        &queued.currency,
        queued.net_amount,
        queued.vat_amount,
        queued.gross_amount,
    );

    invoicing::submit_invoice(
        &state,
        &SubmitPayload {
            invoice_id: queued.id,
            send_email: false,
            payment_method: None,
        },
    )
    .await
    .unwrap();

    let bodies = stub.created_bodies();
    assert_eq!(bodies.len(), 1, "one report, even on the reconcile path");
    assert!(
        bodies[0].contains(r#""paymentMethod":"CASH""#),
        "the report carries the stored method, got: {}",
        bodies[0]
    );
    let adopted = invoices::find(&pool, queued.id).await.unwrap().unwrap();
    assert_eq!(adopted.status, InvoiceStatus::Issued);
    assert_eq!(adopted.payment_method, "CASH");
}
