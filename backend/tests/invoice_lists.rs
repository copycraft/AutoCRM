//! The invoice lists worked out from the data: outgoing (Kiállítandó ... Sztornó, with
//! paid marked by the office or by a cash payment), and incoming supplier invoices.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn send(pool: &PgPool, req: Request<Body>) -> (StatusCode, Value) {
    let response = api::router(common::state(pool.clone())).oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn call(pool: &PgPool, method: &str, uri: &str, bearer: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {bearer}"));
    let body = match body {
        Some(b) => {
            builder = builder.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    send(pool, builder.body(body).unwrap()).await
}

async fn token(pool: &PgPool, role: Role) -> String {
    let user = common::user(pool, role).await;
    let token = auth::generate_token();
    sessions::insert(
        pool,
        NewSession {
            user_id: user.user_id,
            token_hash: &auth::token_hash(&token),
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

/// An invoice row as NAV left it, without going through the sidecar.
async fn invoice(pool: &PgPool, order_id: i64, number: &str, kind: &str, status: &str, method: &str, original: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO invoices (order_id, number, kind, status, original_invoice_id, currency,
                               issue_date, delivery_date, net_amount, vat_amount, gross_amount,
                               payment_method, issued_at)
         VALUES ($1, $2, $3::invoice_kind, $4::invoice_status, $5, 'HUF', current_date, current_date,
                 100, 27, 127, $6, CASE WHEN $4 <> 'submitting' THEN now() END)
         RETURNING id",
    )
    .bind(order_id)
    .bind(number)
    .bind(kind)
    .bind(status)
    .bind(original)
    .bind(method)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn bucket_of(pool: &PgPool, id: i64) -> String {
    sqlx::query_scalar("SELECT bucket FROM invoice_buckets WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn outgoing_invoices_land_on_their_list_by_themselves(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::invoiceable_order(&pool, &user, "HUF", vec![]).await.id;

    let submitting = invoice(&pool, order, "T-1", "invoice", "submitting", "TRANSFER", None).await;
    let transfer = invoice(&pool, order, "T-2", "invoice", "issued", "TRANSFER", None).await;
    let cash = invoice(&pool, order, "T-3", "invoice", "issued", "CASH", None).await;
    let annulled = invoice(&pool, order, "T-4", "invoice", "annulled", "TRANSFER", None).await;
    let cancelled = invoice(&pool, order, "T-5", "invoice", "stornoed", "TRANSFER", None).await;
    let storno = invoice(&pool, order, "T-6", "storno", "issued", "TRANSFER", Some(cancelled)).await;

    assert_eq!(bucket_of(&pool, submitting).await, "to_issue");
    assert_eq!(bucket_of(&pool, transfer).await, "issued");
    // Cash is paid when it is issued.
    assert_eq!(bucket_of(&pool, cash).await, "paid");
    assert_eq!(bucket_of(&pool, annulled).await, "archived");
    assert_eq!(bucket_of(&pool, cancelled).await, "stornoed");
    assert_eq!(bucket_of(&pool, storno).await, "storno");

    // A rejected attempt waits to be issued again, until a later one replaces it.
    let other = common::invoiceable_order(&pool, &user, "HUF", vec![]).await.id;
    let rejected = invoice(&pool, other, "R-1", "invoice", "rejected", "TRANSFER", None).await;
    assert_eq!(bucket_of(&pool, rejected).await, "to_issue");
    invoice(&pool, other, "R-2", "invoice", "issued", "TRANSFER", None).await;
    assert_eq!(bucket_of(&pool, rejected).await, "archived");

    // The office marks a transfer paid, and back.
    let (status, _) = call(&pool, "POST", &format!("/api/invoices/{transfer}/paid"), &office, Some(json!({ "paid": true }))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(bucket_of(&pool, transfer).await, "paid");
    let (_, paid) = call(&pool, "GET", "/api/invoices?bucket=paid", &office, None).await;
    let numbers: Vec<&str> = paid["items"].as_array().unwrap().iter().map(|i| i["number"].as_str().unwrap()).collect();
    assert_eq!(numbers, vec!["T-3", "T-2"]);
    assert!(paid["items"][0]["paid_at"].is_string());
    let (status, _) = call(&pool, "POST", &format!("/api/invoices/{transfer}/paid"), &office, Some(json!({ "paid": false }))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(bucket_of(&pool, transfer).await, "issued");

    // Only an issued invoice can be paid.
    let (status, body) = call(&pool, "POST", &format!("/api/invoices/{storno}/paid"), &office, Some(json!({ "paid": true }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let (_, counts) = call(&pool, "GET", "/api/invoices/buckets", &office, None).await;
    let count = |b: &str| {
        counts["items"].as_array().unwrap().iter().find(|c| c["bucket"] == b).map(|c| c["count"].as_i64().unwrap()).unwrap_or(0)
    };
    assert_eq!(count("issued"), 2); // T-2 and R-2
    assert_eq!(count("archived"), 2);

    let (status, _) = call(&pool, "GET", "/api/invoices?bucket=nope", &office, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

async fn incoming(pool: &PgPool) -> i64 {
    sqlx::query_scalar("INSERT INTO incoming_invoices (file_name) VALUES ('szamla.pdf') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn incoming_invoices_move_between_lists_as_they_are_filled_in(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let id = incoming(&pool).await;
    let patch = |body: Value| {
        let pool = pool.clone();
        let office = office.clone();
        async move { call(&pool, "PATCH", &format!("/api/incoming-invoices/{id}"), &office, Some(body)).await }
    };

    let (_, row) = call(&pool, "GET", &format!("/api/incoming-invoices/{id}"), &office, None).await;
    assert_eq!(row["bucket"], "open_invoice");

    let (status, row) = patch(json!({
        "supplier_name": "Hűtőgép Kft.", "invoice_number": "HG-2026/118",
        "gross_amount": 12_700_000, "net_amount": 10_000_000, "vat_amount": 2_700_000,
        "issue_date": "2026-10-01", "due_date": "2026-10-09"
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{row}");
    assert_eq!(row["bucket"], "open_invoice");

    let (_, row) = patch(json!({ "paid_amount": 5_000_000 })).await;
    assert_eq!(row["bucket"], "partial");
    let (_, row) = patch(json!({ "paid_amount": 12_700_000, "paid_on": "2026-10-05" })).await;
    assert_eq!(row["bucket"], "transferred");
    let (_, row) = patch(json!({ "payment_method": "CASH" })).await;
    assert_eq!(row["bucket"], "cash");
    let (_, row) = patch(json!({ "booking_only": true })).await;
    assert_eq!(row["bucket"], "booking_only");

    let proforma = incoming(&pool).await;
    let (_, row) = call(&pool, "PATCH", &format!("/api/incoming-invoices/{proforma}"), &office, Some(json!({ "kind": "proforma" }))).await;
    assert_eq!(row["bucket"], "open_proforma");
    let receipt = incoming(&pool).await;
    let (_, row) = call(&pool, "PATCH", &format!("/api/incoming-invoices/{receipt}"), &office, Some(json!({ "kind": "receipt" }))).await;
    assert_eq!(row["bucket"], "cash_receipt");

    // One supplier's number is recorded once.
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/incoming-invoices/{proforma}"),
        &office,
        Some(json!({ "supplier_name": "hűtőgép kft.", "invoice_number": "hg-2026/118" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Bad values are refused.
    for bad in [json!({ "kind": "bill" }), json!({ "currency": "USD" }), json!({ "paid_amount": -1 })] {
        let (status, _) = call(&pool, "PATCH", &format!("/api/incoming-invoices/{receipt}"), &office, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // Lists, counts, search, suppliers.
    let (_, open) = call(&pool, "GET", "/api/incoming-invoices?bucket=open_proforma", &office, None).await;
    assert_eq!(open["items"].as_array().unwrap().len(), 1);
    let (_, found) = call(&pool, "GET", "/api/incoming-invoices?q=h%C5%B1t%C5%91g%C3%A9p", &office, None).await;
    assert_eq!(found["items"][0]["id"], id);
    let (_, counts) = call(&pool, "GET", "/api/incoming-invoices/buckets", &office, None).await;
    assert_eq!(counts["items"].as_array().unwrap().len(), 3);
    let (_, suppliers) = call(&pool, "GET", "/api/incoming-invoices/suppliers", &office, None).await;
    assert_eq!(suppliers["items"][0]["supplier_name"], "Hűtőgép Kft.");
    assert_eq!(suppliers["items"][0]["payment_method"], "CASH");

    // Removing keeps the record but takes it off the lists.
    let (status, _) = call(&pool, "DELETE", &format!("/api/incoming-invoices/{receipt}"), &office, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&pool, "GET", &format!("/api/incoming-invoices/{receipt}"), &office, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // A viewer reads, never writes.
    let viewer = token(&pool, Role::Viewer).await;
    let (status, _) = call(&pool, "GET", "/api/incoming-invoices", &viewer, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&pool, "PATCH", &format!("/api/incoming-invoices/{id}"), &viewer, Some(json!({ "notes": "x" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_uploaded_file_becomes_an_open_invoice_once(pool: PgPool) {
    let state = common::state(pool.clone());
    let office = token(&pool, Role::Office).await;
    let upload = |bytes: &'static [u8]| {
        Request::builder()
            .method("POST")
            .uri("/api/incoming-invoices/upload?filename=Sz%C3%A1mla%20118.pdf")
            .header("authorization", format!("Bearer {office}"))
            .header("content-type", "application/pdf")
            .body(Body::from(bytes))
            .unwrap()
    };
    // Not a document type we take: refused before anything is stored.
    let (status, _) = send(&pool, upload(b"MZ\x90\x00 not a pdf")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    if !common::storage_available(&state).await {
        return;
    }
    let (status, row) = send(&pool, upload(b"%PDF-1.7 a supplier invoice")).await;
    assert_eq!(status, StatusCode::CREATED, "{row}");
    assert_eq!(row["bucket"], "open_invoice");
    assert_eq!(row["file_name"], "Számla 118.pdf");
    assert_eq!(row["file_type"], "application/pdf");
    let (status, again) = send(&pool, upload(b"%PDF-1.7 a supplier invoice")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{again}");
    let id = row["id"].as_i64().unwrap();
    let (status, url) = call(&pool, "GET", &format!("/api/incoming-invoices/{id}/file"), &office, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(url["url"].as_str().unwrap().contains("incoming-invoices"));
}
