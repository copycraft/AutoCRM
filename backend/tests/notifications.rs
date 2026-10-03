//! The notification feed: a website lead notifies the people who work leads, each reads
//! and marks only their own.

mod common;

use std::sync::Arc;

use autocrm::AppState;
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

const KEY: &str = "test-leads-key";

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

fn state(pool: PgPool) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.leads_api_key = Some(KEY.into());
    state.config = Arc::new(config);
    state
}

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        builder = builder.header("authorization", format!("Bearer {t}"));
    } else {
        builder = builder.header("x-leads-key", KEY);
    }
    let body = match body {
        Some(b) => {
            builder = builder.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let response = api::router(state(pool.clone()))
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn website_lead(pool: &PgPool, name: &str) {
    let (status, _) = call(
        pool,
        "POST",
        "/api/leads/website",
        None,
        Some(
            json!({ "name": name, "email": "x@example.hu", "message": "Hűtős átalakítást kérek" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_website_lead_notifies_admins_and_office_only(pool: PgPool) {
    let admin = token(&pool, Role::Admin).await;
    let office = token(&pool, Role::Office).await;
    let designer = token(&pool, Role::Designer).await;
    let viewer = token(&pool, Role::Viewer).await;

    website_lead(&pool, "Kiss Péter").await;

    for t in [&admin, &office] {
        let (status, list) = call(&pool, "GET", "/api/notifications", Some(t), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["unread"], 1);
        let item = &list["items"][0];
        assert_eq!(item["kind"], "lead");
        assert_eq!(item["title"], "Új érdeklődés a weboldalról");
        assert!(item["body"].as_str().unwrap().contains("Kiss Péter"));
        assert!(item["link"].as_str().unwrap().starts_with("/leads/"));
        assert_eq!(item["read_at"], Value::Null);
    }
    for t in [&designer, &viewer] {
        let (_, list) = call(&pool, "GET", "/api/notifications", Some(t), None).await;
        assert_eq!(list["unread"], 0);
        assert!(list["items"].as_array().unwrap().is_empty());
    }
    // Not signed in: no feed.
    let (status, _) = call(&pool, "GET", "/api/notifications", Some("nope"), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "./migrations")]
async fn clients_ask_for_what_is_new_and_mark_their_own_read(pool: PgPool) {
    let admin = token(&pool, Role::Admin).await;
    let office = token(&pool, Role::Office).await;
    website_lead(&pool, "Első").await;
    website_lead(&pool, "Második").await;

    let (_, all) = call(&pool, "GET", "/api/notifications", Some(&admin), None).await;
    let items = all["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    // Newest first.
    assert!(items[0]["body"].as_str().unwrap().contains("Második"));
    let (oldest, newest) = (
        items[1]["id"].as_i64().unwrap(),
        items[0]["id"].as_i64().unwrap(),
    );

    // after_id is the polling cursor.
    let (_, fresh) = call(
        &pool,
        "GET",
        &format!("/api/notifications?after_id={oldest}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(fresh["items"].as_array().unwrap().len(), 1);
    assert_eq!(fresh["items"][0]["id"], newest);
    let (_, none) = call(
        &pool,
        "GET",
        &format!("/api/notifications?after_id={newest}"),
        Some(&admin),
        None,
    )
    .await;
    assert!(none["items"].as_array().unwrap().is_empty());

    // The admin marks one read; the office user's copy is untouched, even when the admin
    // names the office user's notification id.
    let (_, office_list) = call(&pool, "GET", "/api/notifications", Some(&office), None).await;
    let office_ids: Vec<i64> = office_list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_i64().unwrap())
        .collect();
    let (status, _) = call(
        &pool,
        "POST",
        "/api/notifications/read",
        Some(&admin),
        Some(json!({ "ids": [oldest] })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    call(
        &pool,
        "POST",
        "/api/notifications/read",
        Some(&admin),
        Some(json!({ "ids": office_ids })),
    )
    .await;
    let (_, admin_unread) = call(
        &pool,
        "GET",
        "/api/notifications?unread_only=true",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(admin_unread["unread"], 1);
    assert_eq!(admin_unread["items"].as_array().unwrap().len(), 1);
    let (_, office_after) = call(&pool, "GET", "/api/notifications", Some(&office), None).await;
    assert_eq!(office_after["unread"], 2);

    // Read-all.
    call(
        &pool,
        "POST",
        "/api/notifications/read-all",
        Some(&office),
        None,
    )
    .await;
    let (_, done) = call(&pool, "GET", "/api/notifications", Some(&office), None).await;
    assert_eq!(done["unread"], 0);
    assert_eq!(
        done["items"].as_array().unwrap().len(),
        2,
        "read ones stay in the list"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_bot_or_a_bad_form_notifies_nobody(pool: PgPool) {
    let admin = token(&pool, Role::Admin).await;
    call(
        &pool,
        "POST",
        "/api/leads/website",
        None,
        Some(json!({ "name": "Bot", "email": "b@example.hu", "company": "Spam" })),
    )
    .await;
    call(
        &pool,
        "POST",
        "/api/leads/website",
        None,
        Some(json!({ "name": "Nincs elérhetőség" })),
    )
    .await;
    let (_, list) = call(&pool, "GET", "/api/notifications", Some(&admin), None).await;
    assert_eq!(list["unread"], 0);
}
