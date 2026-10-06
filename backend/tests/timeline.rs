//! A record's history in one list: changes with names instead of ids, stage moves,
//! tasks, and the HR gate on employees.

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

async fn token(pool: &PgPool, role: Role) -> (i64, String) {
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
    (user.user_id, token)
}

#[sqlx::test(migrations = "./migrations")]
async fn a_lead_history_shows_changes_stages_and_tasks(pool: PgPool) {
    let (me, office) = token(&pool, Role::Office).await;
    let (_, lead) = call(&pool, "POST", "/api/leads", &office, Some(json!({ "title": "Hűtős Sprinter" }))).await;
    let id = lead["id"].as_i64().unwrap();
    let (status, _) = call(&pool, "PATCH", &format!("/api/leads/{id}"), &office,
        Some(json!({ "assigned_to": me, "contact_name": "Faragó Aurél" }))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&pool, "POST", &format!("/api/leads/{id}/stage"), &office, Some(json!({ "stage": "contacted" }))).await;
    assert_eq!(status, StatusCode::OK);
    sqlx::query("INSERT INTO tasks (entity_type, entity_id, title, created_by, done_at) VALUES ('lead', $1, 'Önéletrajz feltöltve', $2, now())")
        .bind(id)
        .bind(me)
        .execute(&pool)
        .await
        .unwrap();

    let (status, history) = call(&pool, "GET", &format!("/api/timeline/lead/{id}"), &office, None).await;
    assert_eq!(status, StatusCode::OK, "{history}");
    let items = history["items"].as_array().unwrap();
    let kinds: Vec<(&str, &str)> = items.iter().map(|e| (e["kind"].as_str().unwrap(), e["action"].as_str().unwrap())).collect();
    for expected in [("create", "create"), ("change", "update"), ("stage", "new"), ("stage", "contacted"), ("task", "created"), ("task", "done")] {
        assert!(kinds.contains(&expected), "missing {expected:?} in {kinds:?}");
    }
    // The stage change is shown once, from the stage history, not again from the audit log.
    assert_eq!(kinds.iter().filter(|k| k.1 == "stage_change").count(), 0);
    let change = items.iter().find(|e| e["kind"] == "change").unwrap();
    // The person, not their id.
    let assigned = &change["changes"]["assigned_to"];
    assert!(assigned[1].is_string(), "{assigned}");
    assert_eq!(change["changes"]["contact_name"][1], "Faragó Aurél");
    let stage = items.iter().find(|e| e["action"] == "contacted").unwrap();
    assert!(stage["text"].is_string());

    // Unknown kinds are refused; employee history needs HR access.
    let (status, _) = call(&pool, "GET", "/api/timeline/spaceship/1", &office, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(&pool, "GET", "/api/timeline/employee/1", &office, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_incoming_invoice_history_starts_with_its_file(pool: PgPool) {
    let (me, office) = token(&pool, Role::Office).await;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO incoming_invoices (file_key, file_name, file_size, created_by)
         VALUES ('incoming-invoices/x.pdf', 'szamla.pdf', 4000, $1) RETURNING id",
    )
    .bind(me)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, _) = call(&pool, "PATCH", &format!("/api/incoming-invoices/{id}"), &office, Some(json!({ "supplier_name": "Hűtőgép Kft." }))).await;
    assert_eq!(status, StatusCode::OK);
    let (_, history) = call(&pool, "GET", &format!("/api/timeline/incoming_invoice/{id}"), &office, None).await;
    let items = history["items"].as_array().unwrap();
    assert_eq!(items[0]["kind"], "change");
    let file = items.iter().find(|e| e["kind"] == "file").unwrap();
    assert_eq!(file["file_name"], "szamla.pdf");
    assert_eq!(file["incoming_invoice_id"], id);
}
