//! HR status lists: new employees start active, a status that ends employment archives
//! them and archiving puts them on it, and HR edits the lists.

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
    let response = api::router(common::state(pool.clone()))
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
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

async fn status_id(pool: &PgPool, label: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM employee_statuses WHERE label = $1")
        .bind(label)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn statuses_follow_the_employee_in_and_out(pool: PgPool) {
    let admin = token(&pool, Role::Admin).await;
    let active = status_id(&pool, "Aktív munkavállaló").await;
    let ended = status_id(&pool, "Megszűnt jogviszony").await;
    let papers = status_id(&pool, "Személyi adatokra vár").await;

    let (status, emp) = call(&pool, "POST", "/api/hr/employees", &admin, Some(json!({ "full_name": "Nagy Anna" }))).await;
    assert_eq!(status, StatusCode::CREATED, "{emp}");
    assert_eq!(emp["status_id"], active);
    let id = emp["id"].as_i64().unwrap();

    let (status, emp) = call(&pool, "PUT", &format!("/api/hr/employees/{id}/status"), &admin, Some(json!({ "status_id": papers }))).await;
    assert_eq!(status, StatusCode::OK, "{emp}");
    assert_eq!(emp["status_id"], papers);
    assert!(emp["archived_at"].is_null());

    // Ending the employment archives; archiving and coming back move the status.
    let (_, emp) = call(&pool, "PUT", &format!("/api/hr/employees/{id}/status"), &admin, Some(json!({ "status_id": ended }))).await;
    assert!(emp["archived_at"].is_string());
    let (_, emp) = call(&pool, "POST", &format!("/api/hr/employees/{id}/unarchive"), &admin, None).await;
    assert_eq!(emp["status_id"], active);
    assert!(emp["archived_at"].is_null());
    let (_, emp) = call(&pool, "POST", &format!("/api/hr/employees/{id}/archive"), &admin, None).await;
    assert_eq!(emp["status_id"], ended);
    // Any other status brings them back.
    let (_, emp) = call(&pool, "PUT", &format!("/api/hr/employees/{id}/status"), &admin, Some(json!({ "status_id": papers }))).await;
    assert!(emp["archived_at"].is_null());

    // The list filters by status and counts.
    let (_, list) = call(&pool, "GET", &format!("/api/hr/employees?status={papers}"), &admin, None).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let (_, statuses) = call(&pool, "GET", "/api/hr/statuses", &admin, None).await;
    let s = statuses["items"].as_array().unwrap().iter().find(|s| s["id"] == papers).unwrap().clone();
    assert_eq!(s["employees"], 1);
    assert_eq!(s["section"], "Aktív munkavállaló");
}

#[sqlx::test(migrations = "./migrations")]
async fn hr_edits_the_lists(pool: PgPool) {
    let admin = token(&pool, Role::Admin).await;
    let (status, created) = call(
        &pool,
        "POST",
        "/api/hr/statuses",
        &admin,
        Some(json!({ "section": "Távollévő munkavállaló", "label": "Katonai szolgálat", "color": "#4F8A52" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["color"], "#4f8a52");
    let id = created["id"].as_i64().unwrap();

    let (status, _) = call(&pool, "PATCH", &format!("/api/hr/statuses/{id}"), &admin, Some(json!({ "archived": true }))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&pool, "PUT", "/api/hr/employees/1/status", &admin, Some(json!({ "status_id": id }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The start and end statuses stay.
    let active = status_id(&pool, "Aktív munkavállaló").await;
    let (status, body) = call(&pool, "PATCH", &format!("/api/hr/statuses/{active}"), &admin, Some(json!({ "archived": true }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "status_required");

    // Without HR access, nothing.
    let office = token(&pool, Role::Office).await;
    let (status, _) = call(&pool, "GET", "/api/hr/statuses", &office, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
