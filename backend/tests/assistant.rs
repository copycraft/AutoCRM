//! The assistant's tool loop against a scripted stand-in for the model: tool calls run,
//! names resolve to ids, filters come back, nothing is written, and an absent model is
//! a clear refusal.

mod common;

use std::sync::Arc;

use autocrm::AppState;
use autocrm::api;
use autocrm::config::AiConfig;
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

/// A fake chat endpoint: the first answer is `first` (a tool call), and once the
/// conversation holds a tool result it answers with plain text naming what it saw.
async fn stub_model(first: Value) -> String {
    let app = axum::Router::new().route(
        "/v1/chat/completions",
        post(move |axum::Json(body): axum::Json<Value>| {
            let first = first.clone();
            async move {
                let messages = body["messages"].as_array().unwrap();
                let tool_results: Vec<&str> = messages
                    .iter()
                    .filter(|m| m["role"] == "tool")
                    .filter_map(|m| m["content"].as_str())
                    .collect();
                let message = if tool_results.is_empty() {
                    first
                } else {
                    json!({ "role": "assistant", "content": format!("Kész. {}", tool_results.join(" | ")) })
                };
                axum::Json(json!({ "choices": [{ "message": message }] }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn state(pool: PgPool, url: Option<String>) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.ai = url.map(|url| AiConfig { url, model: "stub".into(), timeout_secs: 10 });
    state.config = Arc::new(config);
    state
}

async fn ask(pool: &PgPool, url: Option<String>, question: &str) -> (StatusCode, Value) {
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
    let body = json!({ "messages": [{ "role": "user", "content": question }] });
    let response = api::router(state(pool.clone(), url))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/assistant/chat")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// A model address nothing listens on: a routed question must not need it.
const NO_MODEL: &str = "http://127.0.0.1:9";

#[sqlx::test(migrations = "./migrations")]
async fn a_plain_filter_request_is_routed_and_grounded_without_the_model(pool: PgPool) {
    let leads_before: i64 = sqlx::query_scalar("SELECT count(*) FROM leads").fetch_one(&pool).await.unwrap();
    let (status, reply) = ask(&pool, Some(NO_MODEL.into()), "Szűrő: nyitott JEGELVE leadek Megkeresve fázisban").await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let filter = &reply["filters"][0];
    assert_eq!(filter["list"], "leads");
    assert_eq!(filter["params"]["stage"], "contacted");
    assert_eq!(filter["params"]["open"], "1");
    // JEGELVE exists in every market; with no market named and no leads on any, the
    // Hungarian list wins.
    let tag: i64 = filter["params"]["tag"].as_str().unwrap().parse().unwrap();
    let (label, market): (String, String) = sqlx::query_as("SELECT label, market FROM lead_tags WHERE id = $1")
        .bind(tag).fetch_one(&pool).await.unwrap();
    assert_eq!((label.as_str(), market.as_str()), ("JEGELVE", "hu"));
    assert!(filter["path"].as_str().unwrap().starts_with("/leads?"));
    assert_eq!(reply["steps"][0]["tool"], "create_filter");
    assert!(reply["reply"].as_str().unwrap().starts_with("Elkészítettem a szűrőt"));
    // Read-only.
    let leads_after: i64 = sqlx::query_scalar("SELECT count(*) FROM leads").fetch_one(&pool).await.unwrap();
    assert_eq!(leads_before, leads_after);

    // A market in the question picks that market's tag.
    let (_, reply) = ask(&pool, Some(NO_MODEL.into()), "Szűrő: német JEGELVE leadek").await;
    let tag: i64 = reply["filters"][0]["params"]["tag"].as_str().unwrap().parse().unwrap();
    let market: String = sqlx::query_scalar("SELECT market FROM lead_tags WHERE id = $1").bind(tag).fetch_one(&pool).await.unwrap();
    assert_eq!(market, "de");
}

#[sqlx::test(migrations = "./migrations")]
async fn greetings_overviews_and_record_numbers_need_no_model(pool: PgPool) {
    let (status, reply) = ask(&pool, Some(NO_MODEL.into()), "Szia!").await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert!(reply["reply"].as_str().unwrap().starts_with("Szia!"));

    let (_, reply) = ask(&pool, Some(NO_MODEL.into()), "Összesítő").await;
    assert!(reply["reply"].as_str().unwrap().contains("Leadek fázisonként"));

    let (_, reply) = ask(&pool, Some(NO_MODEL.into()), "Mi van a 99999-es leaddel?").await;
    assert_eq!(reply["steps"][0]["tool"], "get_record");
    assert_eq!(reply["steps"][0]["arguments"]["id"], 99999);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_model_picks_the_tool_and_the_question_grounds_its_arguments(pool: PgPool) {
    // The model names the newsletter list but also invents a status nobody asked for.
    let url = stub_model(json!({
        "role": "assistant",
        "content": null,
        "tool_calls": [{
            "id": "c1", "type": "function",
            "function": { "name": "create_filter", "arguments": "{\"list\": \"subscribers\", \"tag\": \"pekseg\", \"status\": \"unsubscribed\"}" }
        }]
    }))
    .await;
    let (status, reply) = ask(&pool, Some(url), "Pékségek").await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    let filter = &reply["filters"][0];
    assert_eq!(filter["list"], "subscribers");
    assert!(filter["params"]["tag"].is_string());
    assert!(filter["params"].get("status").is_none(), "an invented status is dropped: {filter}");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_refused_call_is_retried_once_then_explained(pool: PgPool) {
    // Written as text, the way small models often answer; points at a lead that is not there.
    let url = stub_model(json!({
        "role": "assistant",
        "content": "<tool_call>\n{\"name\": \"get_record\", \"arguments\": {\"kind\": \"lead\", \"id\": 424242}}\n</tool_call>"
    }))
    .await;
    let (status, reply) = ask(&pool, Some(url), "valami egészen más").await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(reply["steps"][0]["tool"], "get_record");
    assert_eq!(reply["steps"][0]["ok"], false);
    assert!(reply["reply"].as_str().unwrap().contains("nincs ilyen lead"), "{reply}");
}

#[sqlx::test(migrations = "./migrations")]
async fn without_a_model_the_assistant_is_off(pool: PgPool) {
    let (status, body) = ask(&pool, None, "Szia").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "assistant_unavailable");
    // A question that needs a model which is not running is the same refusal, not a 500.
    let (status, body) = ask(&pool, Some(NO_MODEL.into()), "Pékségek").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
}
