//! Lead tags: per-market lists the office edits, and website domains that tag a lead on
//! arrival.

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

async fn tag_labels(pool: &PgPool, bearer: &str, lead_id: i64) -> Vec<(String, Value)> {
    let (status, detail) = call(
        pool,
        "GET",
        &format!("/api/leads/{lead_id}"),
        Some(bearer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    detail["tags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["label"].as_str().unwrap().to_string(),
                t["matched_domain"].clone(),
            )
        })
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn the_market_lists_come_seeded(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let (status, list) = call(&pool, "GET", "/api/lead-tags", Some(&office), None).await;
    assert_eq!(status, StatusCode::OK);
    let items = list["items"].as_array().unwrap();
    for market in ["hu", "ro", "de", "it"] {
        assert!(
            items.iter().any(|t| t["market"] == market),
            "no {market} tags"
        );
    }
    let it = items
        .iter()
        .find(|t| t["label"] == "furgonifunebri.it")
        .unwrap();
    assert_eq!(it["domains"], json!(["furgonifunebri.it"]));
}

#[sqlx::test(migrations = "./migrations")]
async fn a_lead_from_a_claimed_domain_is_tagged_on_arrival(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let (status, tag) = call(
        &pool,
        "POST",
        "/api/lead-tags",
        Some(&office),
        Some(json!({
            "market": "HU",
            "label": "Hűtőautók weboldal",
            "color": "#3D6FD6",
            "domains": ["https://www.HutoAutok.hu/", "  "]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{tag}");
    assert_eq!(tag["market"], "hu");
    assert_eq!(tag["color"], "#3d6fd6");
    assert_eq!(tag["domains"], json!(["hutoautok.hu"]));

    // The site field, a subdomain of it.
    let a = website_lead(&pool, json!({ "site": "ajanlat.hutoautok.hu" })).await;
    assert_eq!(
        tag_labels(&pool, &office, a).await,
        vec![("Hűtőautók weboldal".to_string(), json!("hutoautok.hu"))]
    );
    // A full landing URL works as well as the site field.
    let b = website_lead(
        &pool,
        json!({ "landing_page": "https://hutoautok.hu/hutokamra" }),
    )
    .await;
    assert_eq!(tag_labels(&pool, &office, b).await.len(), 1);
    // A lookalike domain and a bare path do not.
    let c = website_lead(
        &pool,
        json!({ "site": "nothutoautok.hu", "page": "/hutoautok.hu" }),
    )
    .await;
    assert!(tag_labels(&pool, &office, c).await.is_empty());

    // The list shows the tags and filters on one.
    let id = tag["id"].as_i64().unwrap();
    let (status, list) = call(
        &pool,
        "GET",
        &format!("/api/leads?tag={id}"),
        Some(&office),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = list["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["tags"][0]["label"], "Hűtőautók weboldal");
    assert!(
        rows[0]["title"].is_string(),
        "the summary fields are flattened into the row"
    );

    // A typed source names the domain too.
    let (status, lead) = call(
        &pool,
        "POST",
        "/api/leads",
        Some(&office),
        Some(json!({ "title": "Telefonos érdeklődő", "source": "hutoautok.hu" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let manual = lead["id"].as_i64().unwrap();
    assert_eq!(tag_labels(&pool, &office, manual).await.len(), 1);

    // The counts on the settings list.
    let (_, list) = call(&pool, "GET", "/api/lead-tags", Some(&office), None).await;
    let counted = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap()
        .clone();
    assert_eq!(counted["total_leads"], 3);
    assert_eq!(counted["open_leads"], 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn one_domain_belongs_to_one_live_tag(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let (status, body) = call(
        &pool,
        "POST",
        "/api/lead-tags",
        Some(&office),
        Some(json!({ "market": "it", "label": "Másik", "domains": ["furgonifunebri.it"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "domain_taken");

    for (bad, why) in [
        (json!({ "market": "hun", "label": "x" }), "market"),
        (json!({ "market": "hu", "label": " " }), "label"),
        (
            json!({ "market": "hu", "label": "x", "color": "red" }),
            "color",
        ),
        (
            json!({ "market": "hu", "label": "x", "domains": ["nem domain"] }),
            "domain",
        ),
    ] {
        let (status, _) = call(&pool, "POST", "/api/lead-tags", Some(&office), Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{why}");
    }

    // The same label twice in one market is refused; in another market it is fine.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/lead-tags",
        Some(&office),
        Some(json!({ "market": "hu", "label": "jegelve" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Archiving frees the domain for another tag.
    let owner: i64 =
        sqlx::query_scalar("SELECT id FROM lead_tags WHERE label = 'furgonifunebri.it'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (status, archived) = call(
        &pool,
        "PATCH",
        &format!("/api/lead-tags/{owner}"),
        Some(&office),
        Some(json!({ "archived": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(archived["archived_at"].is_string());
    let (status, _) = call(
        &pool,
        "POST",
        "/api/lead-tags",
        Some(&office),
        Some(json!({ "market": "it", "label": "Másik", "domains": ["furgonifunebri.it"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_office_sets_a_leads_tags(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let lead = website_lead(
        &pool,
        json!({ "site": "https://www.bestattungswagen.at/kontakt" }),
    )
    .await;
    let ids: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, label FROM lead_tags WHERE market = 'de' AND label IN ('bestattungswagen.at', 'Viszonteladók')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let id_of = |l: &str| ids.iter().find(|(_, label)| label == l).unwrap().0;
    let auto = id_of("bestattungswagen.at");
    let reseller = id_of("Viszonteladók");

    // Adding one keeps the recognised tag and how it got there.
    let (status, body) = call(
        &pool,
        "PUT",
        &format!("/api/leads/{lead}/tags"),
        Some(&office),
        Some(json!({ "tag_ids": [auto, reseller, reseller] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let kept = items.iter().find(|t| t["id"] == auto).unwrap();
    assert_eq!(kept["matched_domain"], "bestattungswagen.at");
    let added = items.iter().find(|t| t["id"] == reseller).unwrap();
    assert!(added["matched_domain"].is_null());

    // Removing.
    let (status, body) = call(
        &pool,
        "PUT",
        &format!("/api/leads/{lead}/tags"),
        Some(&office),
        Some(json!({ "tag_ids": [reseller] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    // A tag that does not exist.
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/leads/{lead}/tags"),
        Some(&office),
        Some(json!({ "tag_ids": [999999] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Read-only roles cannot.
    let viewer = token(&pool, Role::Viewer).await;
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/leads/{lead}/tags"),
        Some(&viewer),
        Some(json!({ "tag_ids": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_market_list_is_reordered(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM lead_tags WHERE market = 'ro' ORDER BY position DESC")
            .fetch_all(&pool)
            .await
            .unwrap();
    let (status, _) = call(
        &pool,
        "PUT",
        "/api/lead-tags/order",
        Some(&office),
        Some(json!({ "market": "ro", "ids": ids })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let now: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM lead_tags WHERE market = 'ro' ORDER BY position")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(now, ids);
}
