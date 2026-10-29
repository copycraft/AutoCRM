//! VAT checks over the API against a stand-in for VIES: verdicts are logged and cached,
//! `force` asks again, outages are never cached or called invalid, and a bad format is a
//! 422 that never reaches VIES.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use autocrm::AppState;
use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// Answers like the VIES test service: number 100 valid, 200 invalid, 301 member state
/// down. Counts the calls it gets.
async fn stub_vies() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = axum::Router::new().route(
        "/check-vat-number",
        post(move |axum::Json(body): axum::Json<Value>| {
            counter.fetch_add(1, Ordering::SeqCst);
            async move {
                let answer = match body["vatNumber"].as_str().unwrap() {
                    "100" => json!({ "valid": true, "name": "ACME GMBH", "address": "---", "requestDate": "2026-10-28T08:55:12.345Z" }),
                    "200" => json!({ "valid": false }),
                    _ => json!({ "actionSucceed": false, "errorWrappers": [{ "error": "MS_UNAVAILABLE" }] }),
                };
                axum::Json(answer)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), calls)
}

struct Ctx {
    state: AppState,
    token: String,
    calls: Arc<AtomicUsize>,
}

async fn ctx(pool: &PgPool) -> Ctx {
    let (url, calls) = stub_vies().await;
    let mut state = common::state(pool.clone());
    let mut config = (*state.config).clone();
    config.vies.base_url = url;
    config.vies.backoff_base = std::time::Duration::ZERO;
    state.config = Arc::new(config);

    let user = common::user(pool, Role::Viewer).await;
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
    Ctx { state, token, calls }
}

async fn call(c: &Ctx, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {}", c.token));
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let response = api::router(c.state.clone())
        .oneshot(req.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[sqlx::test(migrations = "./migrations")]
async fn a_verdict_is_logged_cached_and_forced(pool: PgPool) {
    let c = ctx(&pool).await;
    let partner = common::partner(&pool, "ACME GmbH", None).await;
    let body = json!({ "input": "de 100", "subject": { "type": "partner", "id": partner } });

    let (status, first) = call(&c, "POST", "/api/vat/check", Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["status"], "VALID");
    assert_eq!(first["formatted"], "DE100");
    assert_eq!(first["name"], "ACME GMBH");
    assert_eq!(first["address"], Value::Null);
    assert_eq!(first["cached"], false);
    assert_eq!(c.calls.load(Ordering::SeqCst), 1);

    let (_, second) = call(&c, "POST", "/api/vat/check", Some(body.clone())).await;
    assert_eq!(second["cached"], true);
    assert_eq!(second["check_id"], first["check_id"]);
    assert_eq!(c.calls.load(Ordering::SeqCst), 1);

    let mut forced = body.clone();
    forced["force"] = json!(true);
    let (_, third) = call(&c, "POST", "/api/vat/check", Some(forced)).await;
    assert_eq!(third["cached"], false);
    assert_ne!(third["check_id"], first["check_id"]);
    assert_eq!(c.calls.load(Ordering::SeqCst), 2);

    let (status, history) = call(
        &c,
        "GET",
        &format!("/api/vat/checks?subject_type=partner&subject_id={partner}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = history["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], third["check_id"]);

    let (vat_status, last): (Option<String>, Option<i64>) =
        sqlx::query_as("SELECT vat_status, vat_last_check_id FROM partners WHERE id = $1")
            .bind(partner)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(vat_status.as_deref(), Some("VALID"));
    assert_eq!(last, third["check_id"].as_i64());
}

#[sqlx::test(migrations = "./migrations")]
async fn invalid_is_a_200_and_is_cached(pool: PgPool) {
    let c = ctx(&pool).await;
    let (status, r) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "DE200" }))).await;
    assert_eq!((status, r["status"].as_str()), (StatusCode::OK, Some("INVALID")));
    let (_, r) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "DE200" }))).await;
    assert_eq!(r["cached"], true);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_outage_is_unavailable_and_never_cached(pool: PgPool) {
    let c = ctx(&pool).await;
    let (status, r) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "DE301" }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["status"], "UNAVAILABLE");
    assert_eq!(r["error_code"], "MS_UNAVAILABLE");
    let after_first = c.calls.load(Ordering::SeqCst);
    assert!(after_first >= 1);

    let (_, r) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "DE301" }))).await;
    assert_eq!(r["cached"], false);
    assert!(c.calls.load(Ordering::SeqCst) > after_first);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_bad_format_is_422_and_never_reaches_vies(pool: PgPool) {
    let c = ctx(&pool).await;
    let (status, r) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "HU1234567" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r["status"], "FORMAT_INVALID");
    assert_eq!(r["error_code"], "FORMAT_INVALID");
    let (status, _) = call(&c, "POST", "/api/vat/check", Some(json!({ "input": "12345678" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(c.calls.load(Ordering::SeqCst), 0);
    let logged: i64 = sqlx::query_scalar("SELECT count(*) FROM vat_checks").fetch_one(&pool).await.unwrap();
    assert_eq!(logged, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_audit_log_is_append_only(pool: PgPool) {
    let c = ctx(&pool).await;
    call(&c, "POST", "/api/vat/check", Some(json!({ "input": "DE100" }))).await;
    assert!(sqlx::query("UPDATE vat_checks SET status = 'INVALID'").execute(&pool).await.is_err());
    assert!(sqlx::query("DELETE FROM vat_checks").execute(&pool).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn an_unknown_partner_is_404(pool: PgPool) {
    let c = ctx(&pool).await;
    let body = json!({ "input": "DE100", "subject": { "type": "partner", "id": 999_999 } });
    let (status, _) = call(&c, "POST", "/api/vat/check", Some(body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
