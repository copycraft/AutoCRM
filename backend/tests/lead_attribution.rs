//! Source tracking: what the website sends about where a visitor came from is stored with
//! the lead, sorted into a channel, shown on the lead and counted in the report.

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

fn state(pool: PgPool) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.leads_api_key = Some(KEY.into());
    config.leads_notify_to = None;
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
    builder = match bearer {
        Some(t) => builder.header("authorization", format!("Bearer {t}")),
        None => builder.header("x-leads-key", KEY),
    };
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

async fn admin(pool: &PgPool) -> String {
    let user = common::user(pool, Role::Admin).await;
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

async fn website_lead(pool: &PgPool, extra: Value) -> i64 {
    let mut body = json!({ "name": "Teszt Látogató", "email": "latogato@example.hu" });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    let (status, _) = call(pool, "POST", "/api/leads/website", None, Some(body)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    sqlx::query_scalar("SELECT max(id) FROM leads")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn the_tags_are_stored_classified_and_shown_on_the_lead(pool: PgPool) {
    let admin = admin(&pool).await;
    let id = website_lead(
        &pool,
        json!({
            "utm_source": "google",
            "utm_medium": "cpc",
            "utm_campaign": "tavasz-hutokamra",
            "referrer": "https://www.google.com/",
            "landing_page": "/szolgaltatasok/hutokamra"
        }),
    )
    .await;
    let (status, detail) = call(
        &pool,
        "GET",
        &format!("/api/leads/{id}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let a = &detail["attribution"];
    assert_eq!(a["channel"], "paid");
    assert_eq!(a["utm_campaign"], "tavasz-hutokamra");
    assert_eq!(a["landing_page"], "/szolgaltatasok/hutokamra");

    // A visitor who sent nothing is direct; one from a search engine is organic.
    let direct = website_lead(&pool, json!({})).await;
    let (_, d) = call(
        &pool,
        "GET",
        &format!("/api/leads/{direct}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(d["attribution"]["channel"], "direct");
    let organic = website_lead(
        &pool,
        json!({ "referrer": "https://www.bing.com/search?q=hutos" }),
    )
    .await;
    let (_, o) = call(
        &pool,
        "GET",
        &format!("/api/leads/{organic}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(o["attribution"]["channel"], "organic");

    // A lead entered by hand has no source information at all.
    let (status, manual) = call(
        &pool,
        "POST",
        "/api/leads",
        Some(&admin),
        Some(json!({ "title": "Telefonos érdeklődés" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, m) = call(
        &pool,
        "GET",
        &format!("/api/leads/{}", manual["id"]),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(m["attribution"], Value::Null);
}

#[sqlx::test(migrations = "./migrations")]
async fn overlong_tags_are_refused_and_nothing_is_filed(pool: PgPool) {
    let (status, _) = call(
        &pool,
        "POST",
        "/api/leads/website",
        None,
        Some(json!({ "name": "X", "email": "x@example.hu", "referrer": "https://example.com/".to_string() + &"a".repeat(600) })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM leads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_report_counts_leads_and_wins_by_channel_campaign_and_page(pool: PgPool) {
    let admin = admin(&pool).await;
    let paid1 = website_lead(
        &pool,
        json!({ "utm_source": "google", "utm_medium": "cpc", "utm_campaign": "tavasz", "landing_page": "/hutokamra" }),
    )
    .await;
    website_lead(
        &pool,
        json!({ "utm_source": "google", "utm_medium": "cpc", "utm_campaign": "tavasz", "landing_page": "/hutokamra" }),
    )
    .await;
    website_lead(
        &pool,
        json!({ "referrer": "https://www.google.com/", "landing_page": "/kapcsolat" }),
    )
    .await;
    website_lead(&pool, json!({})).await;

    // One of the paid leads is won.
    sqlx::query("INSERT INTO lead_stages (lead_id, stage_key) VALUES ($1, 'won')")
        .bind(paid1)
        .execute(&pool)
        .await
        .unwrap();

    let (status, r) = call(
        &pool,
        "GET",
        "/api/reports/lead-sources",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(r["total"], 4);
    assert_eq!(r["won"], 1);

    let channels = r["by_channel"].as_array().unwrap();
    let row = |c: &str| channels.iter().find(|x| x["channel"] == c).unwrap();
    assert_eq!(
        (row("paid")["leads"].as_i64(), row("paid")["won"].as_i64()),
        (Some(2), Some(1))
    );
    assert_eq!(row("organic")["leads"], 1);
    assert_eq!(row("direct")["leads"], 1);
    assert_eq!(
        channels[0]["channel"], "paid",
        "the biggest channel comes first"
    );

    // Only tagged traffic has campaigns.
    let campaigns = r["by_campaign"].as_array().unwrap();
    assert_eq!(campaigns.len(), 1);
    assert_eq!(campaigns[0]["utm_campaign"], "tavasz");
    assert_eq!(campaigns[0]["leads"], 2);

    let pages = r["by_page"].as_array().unwrap();
    assert_eq!(pages[0]["landing_page"], "/hutokamra");
    assert_eq!(pages[0]["leads"], 2);
    assert_eq!(pages.len(), 2);

    // A period with nothing in it is empty, not an error.
    let (status, empty) = call(
        &pool,
        "GET",
        "/api/reports/lead-sources?from=2020-01-01&to=2020-12-31",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["total"], 0);
}
