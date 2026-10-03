//! `GET /config/lookups` as the clients see it: one document with every
//! enumeration the phone and the web render, readable by any signed-in user,
//! and in agreement with what the API accepts.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
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

fn keys(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|e| e["key"].as_str().unwrap().to_string())
        .collect()
}

fn labels(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|e| e["label_hu"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn any_signed_in_user_reads_the_whole_document(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;

    let (status, doc) = get(&pool, "/api/config/lookups", Some(&viewer)).await;
    assert_eq!(status, StatusCode::OK);

    // The handover damage record, as the phone's form and the web history render it.
    assert_eq!(
        keys(&doc["damage_types"]),
        [
            "scratch", "dent", "crack", "chip", "broken", "missing", "stain", "tear", "other"
        ]
    );
    assert!(
        labels(&doc["damage_types"])
            .iter()
            .all(|l| !l.trim().is_empty())
    );
    assert_eq!(keys(&doc["severities"]), ["minor", "moderate", "severe"]);
    assert_eq!(keys(&doc["verdicts"]), ["preexisting", "new", "dismissed"]);
    assert_eq!(keys(&doc["walkaround_kinds"]), ["checkout", "checkin"]);
    assert_eq!(doc["walkaround_kinds"][0]["label_hu"], "Átvétel");

    // Chips and pickers.
    assert_eq!(keys(&doc["fuel_levels"]), ["E", "1/4", "1/2", "3/4", "F"]);
    assert_eq!(
        keys(&doc["heating_fuels"]),
        ["diesel", "electric", "lpg", "engine_coolant"]
    );
    assert_eq!(doc["heating_fuels"][0]["label_hu"], "Dízel");
    assert_eq!(
        keys(&doc["defrost_modes"]),
        ["automatic", "manual", "hot_gas"]
    );
    assert_eq!(doc["defrost_modes"][2]["label_hu"], "Forrógázas");
    assert_eq!(
        keys(&doc["order_relations"]),
        ["warranty", "rework", "repeat"]
    );
    assert_eq!(doc["order_relations"][0]["label_hu"], "Garanciális");
    assert_eq!(
        keys(&doc["task_entity_types"]),
        ["order", "lead", "partner"]
    );
    assert_eq!(keys(&doc["currencies"]), ["HUF", "EUR"]);
    assert_eq!(keys(&doc["invoice_payment_methods"]), ["TRANSFER", "CASH"]);
    assert_eq!(doc["invoice_payment_methods"][0]["label_hu"], "Átutalás");
    assert_eq!(
        keys(&doc["annulment_codes"]),
        [
            "ERRATIC_DATA",
            "ERRATIC_INVOICE_NUMBER",
            "ERRATIC_INVOICE_ISSUE_DATE",
            "ERRATIC_ELECTRONIC_HASH_VALUE"
        ]
    );

    // Image categories carry the two rules the phone used to hard-code.
    let categories = doc["image_categories"].as_array().unwrap();
    assert_eq!(
        keys(&doc["image_categories"]),
        [
            "intake",
            "production",
            "completion",
            "marketing",
            "inspection"
        ]
    );
    for category in categories {
        let key = category["key"].as_str().unwrap();
        assert_eq!(category["immutable"], key == "intake", "{key}");
        assert_eq!(category["attachable"], key == "production", "{key}");
        assert!(!category["label_hu"].as_str().unwrap().trim().is_empty());
    }

    // The composer starters, content included.
    let themes = doc["email_themes"].as_array().unwrap();
    assert_eq!(keys(&doc["email_themes"]), ["quotation", "promo"]);
    assert_eq!(themes[0]["subject"], "Árajánlatunk");
    assert!(
        themes[0]["body"]
            .as_str()
            .unwrap()
            .contains("Tisztelt Címzett!")
    );

    // The error catalog travels with the document: a reworded message reaches
    // the clients with the next fetch, no app update.
    let texts = doc["error_texts"].as_array().unwrap();
    assert!(texts.len() > 30);
    let checkout = texts.iter().find(|e| e["code"] == "checkout_open").unwrap();
    assert!(checkout["text_hu"].as_str().unwrap().contains("átvételi"));
}

#[sqlx::test(migrations = "./migrations")]
async fn lookups_need_a_session(pool: PgPool) {
    let (status, _) = get(&pool, "/api/config/lookups", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
