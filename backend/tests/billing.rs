//! The Számlázó page as the office sees it: every invoice and storno plus every
//! díjbekérő in one place, across orders, with the order and partner each row
//! belongs to. Issuing still happens per order (behind `IssueInvoices`); these
//! reads are open to any signed-in user.

mod common;

use autocrm::api;
use autocrm::domain::invoice::InvoiceKind;
use autocrm::domain::media::DocumentKind;
use autocrm::domain::role::Role;
use autocrm::repo::documents::{self, NewDocument, Owner};
use autocrm::repo::invoices::{self, NewInvoice, NewProforma};
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

async fn token_for(pool: &PgPool, role: Role) -> String {
    let user = common::user(pool, role).await;
    let token = auth::generate_token();
    let hash = auth::token_hash(&token);
    sessions::insert(
        pool,
        NewSession {
            user_id: user.user_id,
            token_hash: &hash,
            kind: SessionKind::Mobile,
            device_label: None,
            user_agent: None,
            ip: None,
            expires_at: chrono::Utc::now() + chrono::TimeDelta::days(1),
        },
    )
    .await
    .unwrap();
    token
}

async fn get(pool: &PgPool, uri: &str, token: Option<&str>) -> (StatusCode, Value) {
    let app = api::router(common::state(pool.clone()));
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn day() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 10).unwrap()
}

/// An order with one issued invoice, its storno, and one proforma.
async fn billed_order(pool: &PgPool) -> (i64, String) {
    let user = common::user(pool, Role::Office).await;
    let order = common::order(pool, &user, "HUF", vec![]).await;
    let mut tx = pool.begin().await.unwrap();
    let invoice = invoices::insert(
        &mut tx,
        &NewInvoice {
            order_id: order.id,
            number: "AT2026-0001",
            kind: InvoiceKind::Invoice,
            original_invoice_id: None,
            currency: "HUF",
            issue_date: day(),
            delivery_date: day(),
            payment_date: None,
            payment_method: "TRANSFER",
            net_amount: 100_000,
            vat_amount: 27_000,
            gross_amount: 127_000,
            created_by: Some(user.user_id),
        },
    )
    .await
    .unwrap();
    sqlx::query("UPDATE invoices SET status = 'issued', issued_at = now() WHERE id = $1")
        .bind(invoice.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    invoices::insert(
        &mut tx,
        &NewInvoice {
            order_id: order.id,
            number: "AT2026-0002",
            kind: InvoiceKind::Storno,
            original_invoice_id: Some(invoice.id),
            currency: "HUF",
            issue_date: day(),
            delivery_date: day(),
            payment_date: None,
            payment_method: "TRANSFER",
            net_amount: -100_000,
            vat_amount: -27_000,
            gross_amount: -127_000,
            created_by: Some(user.user_id),
        },
    )
    .await
    .unwrap();
    let (document, _) = documents::insert(
        &mut tx,
        &NewDocument {
            owner: Owner::Order(order.id),
            vehicle_id: None,
            kind: DocumentKind::Other,
            filename: "DB2026-0001.pdf",
            content_type: "application/pdf",
            storage_key: "test/proforma.pdf",
            content_hash: &[7u8; 32],
            byte_size: 12,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    invoices::insert_proforma(
        &mut tx,
        &NewProforma {
            order_id: order.id,
            number: "DB2026-0001",
            currency: "HUF",
            issue_date: day(),
            payment_date: None,
            net_amount: 100_000,
            vat_amount: 27_000,
            gross_amount: 127_000,
            document_id: document.id,
            created_by: Some(user.user_id),
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let number: String = sqlx::query_scalar("SELECT number FROM orders WHERE id = $1")
        .bind(order.id)
        .fetch_one(pool)
        .await
        .unwrap();
    (order.id, number)
}

#[sqlx::test(migrations = "./migrations")]
async fn the_billing_page_lists_everything_newest_first(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;
    let (_, number) = billed_order(&pool).await;

    let (status, invoices) = get(&pool, "/api/invoices", Some(&viewer)).await;
    assert_eq!(status, StatusCode::OK);
    let items = invoices["items"].as_array().unwrap();
    // The storno first (newest), then the invoice it cancels. No second query.
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["kind"], "storno");
    assert_eq!(items[0]["original_invoice_id"], items[1]["id"]);
    assert!(items[0]["gross_amount"].as_i64().unwrap() < 0);
    for item in items {
        assert_eq!(item["order_number"], number);
        assert!(!item["partner_name"].as_str().unwrap().is_empty());
    }

    let (status, proformas) = get(&pool, "/api/proformas", Some(&viewer)).await;
    assert_eq!(status, StatusCode::OK);
    let items = proformas["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["number"], "DB2026-0001");
    assert_eq!(items[0]["order_number"], number);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_billing_lists_filter_by_status_and_kind(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;
    billed_order(&pool).await;

    let (_, issued) = get(&pool, "/api/invoices?status=issued", Some(&viewer)).await;
    assert_eq!(issued["items"].as_array().unwrap().len(), 1);
    assert_eq!(issued["items"][0]["number"], "AT2026-0001");

    let (_, stornos) = get(&pool, "/api/invoices?kind=storno", Some(&viewer)).await;
    assert_eq!(stornos["items"].as_array().unwrap().len(), 1);
    assert_eq!(stornos["items"][0]["kind"], "storno");

    let (status, _) = get(&pool, "/api/invoices?status=printed", Some(&viewer)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = get(&pool, "/api/invoices?kind=receipt", Some(&viewer)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_billing_lists_need_a_session(pool: PgPool) {
    for uri in ["/api/invoices", "/api/proformas"] {
        let (status, _) = get(&pool, uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn the_billing_list_takes_a_limit(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;
    billed_order(&pool).await;

    let (_, one) = get(&pool, "/api/invoices?limit=1", Some(&viewer)).await;
    assert_eq!(one["items"].as_array().unwrap().len(), 1);
    // The storno is newest, so a limit of one is the storno.
    assert_eq!(one["items"][0]["kind"], "storno");
}
