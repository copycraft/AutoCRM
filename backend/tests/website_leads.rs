//! `POST /api/leads/website`: the autotherm.hu contact form files a lead.

mod common;

use std::sync::Arc;

use autocrm::AppState;
use autocrm::api;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const KEY: &str = "test-leads-key";

fn state(pool: PgPool, key: Option<&str>) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.leads_api_key = key.map(String::from);
    state.config = Arc::new(config);
    state
}

async fn post(state: AppState, key: Option<&str>, body: Value) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/leads/website")
        .header("content-type", "application/json");
    if let Some(key) = key {
        builder = builder.header("x-leads-key", key);
    }
    api::router(state)
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
        .status()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_form_with_the_key_becomes_an_unassigned_lead_in_the_first_stage(pool: PgPool) {
    let status = post(
        state(pool.clone(), Some(KEY)),
        Some(KEY),
        json!({
            "name": "  Kiss Péter ",
            "email": "Peter@Example.HU",
            "phone": "+36 30 123 4567",
            "message": "Hűtős kisteherautó átalakítás",
            "subject": "Hűtőkamra",
            "vehicle": "Sprinter 316",
            "page": "/szolgaltatasok"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    #[derive(sqlx::FromRow)]
    struct Row {
        title: String,
        contact_name: Option<String>,
        contact_email: Option<String>,
        source: Option<String>,
        description: Option<String>,
        created_by: Option<i64>,
        assigned_to: Option<i64>,
        stage: Option<String>,
    }
    let lead: Row = sqlx::query_as(
        "SELECT l.title, l.contact_name, l.contact_email, l.source, l.description,
                l.created_by, l.assigned_to,
                (SELECT stage_key FROM lead_stages WHERE lead_id = l.id
                 ORDER BY entered_at DESC, id DESC LIMIT 1) AS stage
         FROM leads l",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(lead.title, "Weboldal: Hűtőkamra");
    assert_eq!(lead.contact_name.as_deref(), Some("Kiss Péter"));
    assert_eq!(lead.contact_email.as_deref(), Some("peter@example.hu"));
    assert_eq!(lead.source.as_deref(), Some("website"));
    assert!(lead.created_by.is_none() && lead.assigned_to.is_none());
    let description = lead.description.unwrap();
    assert!(description.contains("Hűtős kisteherautó"));
    assert!(description.contains("Jármű: Sprinter 316"));
    assert!(lead.stage.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn without_the_right_key_nothing_is_filed(pool: PgPool) {
    let form = json!({ "name": "Teszt", "email": "a@example.hu" });
    // Wrong key, missing key, and the endpoint switched off all look the same.
    assert_eq!(
        post(state(pool.clone(), Some(KEY)), Some("nope"), form.clone()).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(state(pool.clone(), Some(KEY)), None, form.clone()).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        post(state(pool.clone(), None), Some(""), form).await,
        StatusCode::FORBIDDEN
    );
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn bad_forms_are_refused_and_the_honeypot_is_dropped_quietly(pool: PgPool) {
    let s = || state(pool.clone(), Some(KEY));
    for body in [
        json!({ "name": "  ", "email": "a@example.hu" }),
        json!({ "name": "Teszt" }),
        json!({ "name": "Teszt", "email": "not-an-address" }),
        json!({ "name": "Teszt", "email": "a@example.hu", "message": "x".repeat(5001) }),
    ] {
        assert_eq!(post(s(), Some(KEY), body).await, StatusCode::BAD_REQUEST);
    }
    // A bot that fills the hidden field is told it worked, and nothing is stored.
    let status = post(
        s(),
        Some(KEY),
        json!({ "name": "Bot", "email": "bot@example.hu", "company": "Spam Kft" }),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
