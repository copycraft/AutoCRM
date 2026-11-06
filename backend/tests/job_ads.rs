//! Job ads (0064): each ad's tracked link, the visits and applications it brings, and the
//! rest sorted by channel. Our own numbers; nothing leaves for the ad platforms.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::recruitment::{self, ApplicationInput, ApplicationSource, PostingInput};
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    let body = match body {
        Some(v) => {
            b = b.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let response = api::router(common::state(pool.clone()))
        .oneshot(b.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn hr_bearer(pool: &PgPool) -> (String, i64) {
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
    (token, user.user_id)
}

fn applicant(email: &str, source: ApplicationSource) -> ApplicationInput {
    ApplicationInput {
        full_name: "Nagy Péter".into(),
        email: email.into(),
        phone: "+36 30 123 4567".into(),
        age: 30,
        city: None,
        message: None,
        resume_key: "test/cv.pdf".into(),
        resume_filename: "cv.pdf".into(),
        resume_content_type: "application/pdf".into(),
        source,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn each_ad_counts_its_own_visits_and_applicants(pool: PgPool) {
    let (token, user_id) = hr_bearer(&pool).await;
    let posting = recruitment::insert_posting(
        &pool,
        &PostingInput {
            title: "Lakatos".into(),
            description: None,
            location: Some("Szeged".into()),
        },
        "lakatos-x1",
        user_id,
    )
    .await
    .unwrap();
    recruitment::set_posting_status(&pool, posting, "published")
        .await
        .unwrap();

    let (status, ad) = call(
        &pool,
        "POST",
        &format!("/api/hr/jobs/{posting}/ads"),
        Some(&token),
        Some(json!({ "platform": "facebook", "name": "Lakatos – Szeged ősz", "spend_minor": 3_000_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ad}");
    let ad_id = ad["id"].as_i64().unwrap();
    let link = ad["link"].as_str().unwrap();
    assert!(link.contains("/hu/jobs/lakatos-x1?"), "{link}");
    assert!(
        link.contains("utm_source=facebook") && link.contains("utm_medium=paid_social"),
        "{link}"
    );
    assert!(
        link.contains(&format!("ad={ad_id}")) && link.contains("utm_campaign=lakatos-szeged-osz"),
        "{link}"
    );

    // Two visits through the ad, one from a Google search; no login, nothing about the visitor.
    for body in [
        json!({ "ad": ad_id, "utm_source": "facebook", "utm_medium": "paid_social" }),
        json!({ "ad": ad_id, "utm_source": "facebook", "utm_medium": "paid_social" }),
        json!({ "referrer": "https://www.google.com/" }),
        // Another listing's ad id counts as no ad.
        json!({ "ad": ad_id + 999 }),
    ] {
        let (status, _) = call(
            &pool,
            "POST",
            "/api/public/jobs/lakatos-x1/visit",
            None,
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    recruitment::insert_application(
        &pool,
        posting,
        &applicant(
            "a@example.hu",
            ApplicationSource {
                ad_id: Some(ad_id),
                channel: Some("paid".into()),
                ..Default::default()
            },
        ),
    )
    .await
    .unwrap();
    recruitment::insert_application(
        &pool,
        posting,
        &applicant(
            "b@example.hu",
            ApplicationSource {
                channel: Some("organic".into()),
                ..Default::default()
            },
        ),
    )
    .await
    .unwrap();

    let (status, report) = call(
        &pool,
        "GET",
        &format!("/api/hr/jobs/{posting}/ads"),
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    let ad = &report["ads"][0];
    assert_eq!(ad["visits"], 2);
    assert_eq!(ad["applications"], 1);
    assert_eq!(ad["cost_per_application_minor"], 3_000_000);
    let other = report["other"].as_array().unwrap();
    let organic = other.iter().find(|c| c["channel"] == "organic").unwrap();
    assert_eq!(
        (organic["visits"].as_i64(), organic["applications"].as_i64()),
        (Some(1), Some(1))
    );
    assert!(
        other
            .iter()
            .any(|c| c["channel"] == "direct" && c["visits"] == 1),
        "{report}"
    );

    // The applicant list says where each one came from.
    let (_, apps) = call(
        &pool,
        "GET",
        &format!("/api/hr/jobs/{posting}/applications"),
        Some(&token),
        None,
    )
    .await;
    let apps = apps["items"].as_array().unwrap();
    assert!(
        apps.iter()
            .any(|a| a["ad_id"] == ad_id && a["channel"] == "paid"),
        "{apps:?}"
    );

    // Archived, the ad keeps its numbers; visits to a draft listing are refused.
    let (status, archived) = call(
        &pool,
        "PATCH",
        &format!("/api/hr/job-ads/{ad_id}"),
        Some(&token),
        Some(json!({ "platform": "facebook", "name": "Lakatos – Szeged ősz", "archived": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        archived["archived_at"].is_string() && archived["visits"] == 2,
        "{archived}"
    );
    recruitment::set_posting_status(&pool, posting, "draft")
        .await
        .unwrap();
    let (status, _) = call(
        &pool,
        "POST",
        "/api/public/jobs/lakatos-x1/visit",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
