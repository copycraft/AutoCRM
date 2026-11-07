//! What each role may see (0061–0063): the grid starts as the code's defaults, a grid
//! change applies on the next request, and the shop floor is sent no money, no customer
//! contacts and no sales data — checked on the wire, where the phone would cache it.

mod common;

use autocrm::api;
use autocrm::domain::role::{Capability, Role};
use autocrm::repo::leads::LeadInput;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use autocrm::service::orders::NewItem;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get(pool: &PgPool, uri: &str, bearer: &str) -> (StatusCode, Value) {
    let response = api::router(common::state(pool.clone()))
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn bearer(pool: &PgPool, role: Role) -> (String, i64) {
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
    (token, user.user_id)
}

/// Every non-null value under a money key, anywhere in the document.
fn money_in(v: &Value, path: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                let here = format!("{path}.{k}");
                if autocrm::service::visibility::is_money_key(k) && !x.is_null() {
                    out.push(format!("{here} = {x}"));
                }
                money_in(x, &here, out);
            }
        }
        Value::Array(items) => {
            for (i, x) in items.iter().enumerate() {
                money_in(x, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

/// Every value of the customer's email or phone, anywhere in the document.
fn contacts_in(v: &Value, needles: &[&str], path: &str, out: &mut Vec<String>) {
    match v {
        Value::String(s) if needles.iter().any(|n| s.contains(n)) => {
            out.push(format!("{path} = {s}"))
        }
        Value::Object(map) => map
            .iter()
            .for_each(|(k, x)| contacts_in(x, needles, &format!("{path}.{k}"), out)),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .for_each(|(i, x)| contacts_in(x, needles, &format!("{path}[{i}]"), out)),
        _ => {}
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn the_grid_starts_as_the_code_defaults(pool: PgPool) {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT role::text, capability FROM role_permissions")
            .fetch_all(&pool)
            .await
            .unwrap();
    for role in Role::ALL.into_iter().filter(|r| *r != Role::Admin) {
        for cap in Capability::ALL.into_iter().filter(|c| c.grantable()) {
            let seeded = rows.contains(&(role.key().to_string(), cap.key()));
            assert_eq!(
                seeded,
                role.can(cap),
                "{role:?} / {cap:?}: the seed and Role::can disagree"
            );
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_grid_change_applies_on_the_next_request(pool: PgPool) {
    let (token, _) = bearer(&pool, Role::Designer).await;
    assert_eq!(
        get(&pool, "/api/leads", &token).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query(
        "INSERT INTO role_permissions (role, capability) VALUES ('designer', 'view_sales')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(get(&pool, "/api/leads", &token).await.0, StatusCode::OK);
    let (_, me) = get(&pool, "/api/auth/me", &token).await;
    assert!(
        me["user"]["capabilities"]
            .as_array()
            .or(me["capabilities"].as_array())
            .unwrap()
            .contains(&Value::String("view_sales".into())),
        "{me}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn the_shop_floor_is_sent_no_money_contacts_or_sales(pool: PgPool) {
    // A job with priced items, for a customer with an email and a phone, and an enquiry.
    let office = common::user(&pool, Role::Office).await;
    let order = common::order(
        &pool,
        &office,
        "HUF",
        vec![NewItem {
            description: "Hűtőgép beépítés".into(),
            quantity: "1".parse().unwrap(),
            unit_price: 98_765_400,
        }],
    )
    .await;
    sqlx::query("UPDATE partners SET phone = '+36 30 555 1234', city = 'Kecskemét' WHERE id = $1")
        .bind(order.partner_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO contacts (partner_id, name, email, phone) VALUES ($1, 'Kiss Anna', 'anna@vevo.hu', '+36 20 444 9876')",
    )
    .bind(order.partner_id)
    .execute(&pool)
    .await
    .unwrap();
    autocrm::service::leads::create(
        &pool,
        &office,
        LeadInput {
            title: "Hűtős Sprinter ajánlat".into(),
            partner_id: Some(order.partner_id),
            contact_id: None,
            contact_name: Some("Kiss Anna".into()),
            contact_email: Some("anna@vevo.hu".into()),
            contact_phone: None,
            source: None,
            source_detail: None,
            description: None,
            assigned_to: None,
            quoted_value_minor: Some(1_234_500),
            currency: Some("HUF".into()),
            quote_valid_until: None,
        },
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();

    let (workshop, _) = bearer(&pool, Role::Designer).await;
    let (worker, _) = bearer(&pool, Role::Worker).await;
    let id = order.id;
    let p = order.partner_id;
    let open = [
        "/api/orders".to_string(),
        format!("/api/orders/{id}"),
        format!("/api/orders/{id}/items"),
        format!("/api/orders/{id}/audit"),
        format!("/api/timeline/order/{id}"),
        format!("/api/orders/{id}/documents"),
        format!("/api/orders/{id}/modules"),
        "/api/partners".to_string(),
        format!("/api/partners/{p}"),
        format!("/api/partners/{p}/contacts"),
        format!("/api/timeline/partner/{p}"),
        "/api/search?q=Kiss".to_string(),
        "/api/search?q=555".to_string(),
        "/api/incidents".to_string(),
        "/api/reports/stage-durations".to_string(),
        "/api/reports/workload".to_string(),
    ];
    for token in [&workshop, &worker] {
        for uri in &open {
            let (status, body) = get(&pool, uri, token).await;
            assert_eq!(status, StatusCode::OK, "{uri}: {body}");
            let mut found = Vec::new();
            money_in(&body, uri, &mut found);
            contacts_in(
                &body,
                &["anna@vevo.hu", "444 9876", "555 1234"],
                uri,
                &mut found,
            );
            contacts_in(&body, &["Hűtős Sprinter ajánlat"], uri, &mut found);
            assert!(found.is_empty(), "sent to the shop floor: {found:#?}");
        }
        for uri in [
            "/api/leads".to_string(),
            "/api/leads/quotes/expiring".to_string(),
            "/api/invoices".to_string(),
            "/api/proformas".to_string(),
            format!("/api/orders/{id}/invoices"),
            format!("/api/partners/{p}/statement"),
            "/api/incoming-invoices".to_string(),
            "/api/emails".to_string(),
            "/api/reports/volume".to_string(),
            "/api/reports/sales-funnel".to_string(),
            "/api/reports/revenue-by-country".to_string(),
        ] {
            assert_eq!(
                get(&pool, &uri, token).await.0,
                StatusCode::FORBIDDEN,
                "{uri}"
            );
        }
    }

    // The office still gets all of it.
    let (office_token, _) = bearer(&pool, Role::Office).await;
    let (_, detail) = get(&pool, &format!("/api/orders/{id}"), &office_token).await;
    assert_eq!(detail["items"][0]["unit_price"], 98_765_400);
    assert!(detail["value"].is_object(), "{detail}");
    let (_, partner) = get(&pool, &format!("/api/partners/{p}"), &office_token).await;
    assert_eq!(partner["partner"]["phone"], "+36 30 555 1234");
    assert_eq!(partner["leads"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_purchaser_hands_over_and_the_worker_does_not(pool: PgPool) {
    let purchaser = common::user(&pool, Role::Purchaser).await;
    let worker = common::user(&pool, Role::Worker).await;
    assert!(purchaser.can(Capability::HandleHandovers));
    assert!(!worker.can(Capability::HandleHandovers));
    let (token, _) = bearer(&pool, Role::Purchaser).await;
    assert_eq!(
        get(&pool, "/api/incoming-invoices", &token).await.0,
        StatusCode::OK
    );
    assert_eq!(
        get(&pool, "/api/invoices", &token).await.0,
        StatusCode::FORBIDDEN
    );
}
