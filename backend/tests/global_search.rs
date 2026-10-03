//! Global search: every word must match, accents and punctuation are ignored, better matches
//! rank first, and the staff directory is only searched for users with HR access.

mod common;

use autocrm::api;
use autocrm::domain::partner::PartnerKind;
use autocrm::domain::role::Role;
use autocrm::repo::contacts::{self, ContactInput};
use autocrm::repo::emails::{self, NewEmail};
use autocrm::repo::partners::PartnerInput;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::repo::{partners, search};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn partner(pool: &PgPool, name: &str, city: Option<&str>, phone: Option<&str>) -> i64 {
    partners::insert(
        pool,
        &PartnerInput {
            kind: PartnerKind::Business,
            name: name.into(),
            tax_number: None,
            eu_tax_number: None,
            country: "HU".into(),
            default_currency: "HUF".into(),
            email: None,
            phone: phone.map(String::from),
            website: None,
            postal_code: None,
            city: city.map(String::from),
            address_line: None,
            notes: None,
            role: None,
        },
    )
    .await
    .unwrap()
    .id
}

fn names(hits: &[search::PartnerHit]) -> Vec<&str> {
    hits.iter().map(|h| h.name.as_str()).collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn every_word_must_match_in_any_order_ignoring_accents(pool: PgPool) {
    partner(&pool, "Kovács Fuvarozó Kft.", Some("Győr"), None).await;
    partner(&pool, "Kovács Építő Zrt.", Some("Pécs"), None).await;

    // Accents and case do not matter, the order of words does not matter.
    for q in ["kovacs gyor", "gyor KOVÁCS", "fuvarozo kft", "Kovács Győr"] {
        let hits = search::partners(&pool, q, 8).await.unwrap();
        assert_eq!(names(&hits), ["Kovács Fuvarozó Kft."], "query {q:?}");
    }
    // One word that matches nothing rules the record out.
    assert!(
        search::partners(&pool, "kovacs budapest", 8)
            .await
            .unwrap()
            .is_empty()
    );
    // One word matches both.
    assert_eq!(search::partners(&pool, "kovacs", 8).await.unwrap().len(), 2);
    // Nothing to search.
    assert!(search::partners(&pool, "   ", 8).await.unwrap().is_empty());
    // A wildcard typed by the user is just a character.
    assert!(search::partners(&pool, "%", 8).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn better_matches_rank_first_whatever_their_age(pool: PgPool) {
    // Inserted newest-last, so plain recency would put the exact match at the bottom.
    partner(&pool, "Kovács", None, None).await;
    partner(&pool, "Kovács és Társa", None, None).await;
    partner(&pool, "Nagy Kovács Kft.", None, None).await;
    partner(&pool, "Újabb Kovács", None, None).await;

    let hits = search::partners(&pool, "kovács", 8).await.unwrap();
    assert_eq!(
        names(&hits),
        [
            "Kovács",
            "Kovács és Társa",
            "Újabb Kovács",
            "Nagy Kovács Kft."
        ],
        "exact, then prefix, then anywhere (newest first within a rank)"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn phone_numbers_find_partners_and_contacts_in_any_spelling(pool: PgPool) {
    let id = partner(&pool, "Telefon Kft.", None, Some("06-30-123-4567")).await;
    contacts::insert(
        &pool,
        id,
        &ContactInput {
            name: "Szabó Ilona".into(),
            email: Some("ilona@telefon.hu".into()),
            phone: Some("+36 20 111 2222".into()),
            position: Some("Flottafelelős".into()),
            notes: None,
        },
    )
    .await
    .unwrap();

    for q in ["+36 30 123 4567", "06301234567", "0036301234567", "30 123"] {
        assert_eq!(
            search::partners(&pool, q, 8).await.unwrap().len(),
            1,
            "partner {q:?}"
        );
    }
    for q in [
        "+36 20 111 2222",
        "06 20 111 2222",
        "szabo ilona",
        "ilona telefon",
        "flottafelelos",
    ] {
        let hits = search::contacts(&pool, q, 8).await.unwrap();
        assert_eq!(hits.len(), 1, "contact {q:?}");
        assert_eq!(hits[0].partner_id, id);
        assert_eq!(hits[0].partner_name, "Telefon Kft.");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn emails_are_found_by_subject_or_recipient(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    emails::insert(
        &pool,
        &NewEmail {
            order_id: None,
            lead_id: None,
            partner_id: None,
            blocker_id: None,
            template_key: None,
            trigger: "manual",
            sent_by: Some(user.user_id),
            idempotency_key: None,
            to_address: "beszallito@festek.example",
            cc: &[],
            bcc: &[],
            from_address: "Teszt <noreply@autotherm.hu>",
            reply_to: None,
            subject: "Sürgős: festék szállítás",
            body_html: "<p>x</p>",
            body_text: "x",
            attachments: json!([]),
            send_after: None,
        },
    )
    .await
    .unwrap();
    for q in ["festek szallitas", "surgos", "beszallito festek"] {
        assert_eq!(search::emails(&pool, q, 8).await.unwrap().len(), 1, "{q:?}");
    }
    assert!(
        search::emails(&pool, "festek ajanlat", 8)
            .await
            .unwrap()
            .is_empty()
    );
}

async fn token(pool: &PgPool, user_id: i64) -> String {
    let token = auth::generate_token();
    sessions::insert(
        pool,
        NewSession {
            user_id,
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

async fn get(pool: &PgPool, uri: &str, token: &str) -> (StatusCode, Value) {
    let response = api::router(common::state(pool.clone()))
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
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

#[sqlx::test(migrations = "./migrations")]
async fn the_staff_directory_is_searched_only_for_hr_users(pool: PgPool) {
    let admin = common::user(&pool, Role::Admin).await;
    let office = common::user(&pool, Role::Office).await;
    let admin_t = token(&pool, admin.user_id).await;
    let office_t = token(&pool, office.user_id).await;

    sqlx::query("INSERT INTO employees (full_name, email, company_phone, personal_phone) VALUES ($1, $2, $3, $4)")
        .bind("Kiss Péter")
        .bind("peter@autotherm.hu")
        .bind("+36 30 111 2222")
        .bind("+36 20 333 4444")
        .execute(&pool)
        .await
        .unwrap();

    // Admins find them, by name without accents and by the personal number.
    for q in ["kiss peter", "06 20 333 4444"] {
        let (status, body) = get(
            &pool,
            &format!("/api/search?q={}", q.replace(' ', "%20")),
            &admin_t,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["employees"].as_array().unwrap().len(), 1, "{q:?}");
        assert_eq!(body["employees"][0]["full_name"], "Kiss Péter");
    }
    // An office user without the flag gets an empty group, not an error and not the data.
    let (status, body) = get(&pool, "/api/search?q=kiss%20peter", &office_t).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["employees"].as_array().unwrap().is_empty());
    // Granting HR access opens the group; the other groups are there for everyone.
    sqlx::query("UPDATE users SET hr_access = true WHERE id = $1")
        .bind(office.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (_, body) = get(&pool, "/api/search?q=kiss%20peter", &office_t).await;
    assert_eq!(body["employees"].as_array().unwrap().len(), 1);
    for group in ["orders", "partners", "leads", "contacts", "emails"] {
        assert!(body[group].is_array(), "{group}");
    }
}
