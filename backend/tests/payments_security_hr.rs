//! Migration 0049: two-factor sign-in and new-device alerts, partial payments and the
//! statement of account, threaded replies, the calendar feed, newsletter language variants,
//! HR checklists, the cooling unit's serial, ad conversions and the new reports.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::domain::totp;
use autocrm::error::AppError;
use autocrm::repo::leads::LeadInput;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::repo::users;
use autocrm::service::auth::{self, LoginRequest};
use autocrm::service::{mailbox, newsletter as sends};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery staple";

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn raw_get(pool: &PgPool, uri: &str) -> (StatusCode, String) {
    let response = api::router(common::state(pool.clone()))
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

async fn bearer_for(pool: &PgPool, user_id: i64) -> String {
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

async fn login(pool: &PgPool, role: Role) -> (String, i64) {
    let user = common::user(pool, role).await;
    (bearer_for(pool, user.user_id).await, user.user_id)
}

fn attempt(email: &str, ua: &str, code: Option<String>) -> LoginRequest {
    LoginRequest {
        email: email.into(),
        password: PASSWORD.into(),
        kind: SessionKind::Web,
        device_label: None,
        user_agent: Some(ua.into()),
        ip: Some("203.0.113.9".into()),
        totp_code: code,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn two_factor_asks_for_a_code_and_a_code_works_once(pool: PgPool) {
    let config = common::config();
    let email = format!("tf-{}@autotherm.test", common::rand_suffix());
    let hash = auth::hash_password(PASSWORD).unwrap();
    let user = users::insert(&pool, &email, "Kétlépcsős Kati", Role::Office, &hash, false)
        .await
        .unwrap();
    let bearer = bearer_for(&pool, user.id).await;

    // Setup needs the password, and hands out a secret the app can read.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/auth/two-factor/setup",
        &bearer,
        Some(json!({ "password": "wrong" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, setup) = call(
        &pool,
        "POST",
        "/api/auth/two-factor/setup",
        &bearer,
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{setup}");
    let secret = totp::base32_decode(setup["secret"].as_str().unwrap()).unwrap();
    assert!(
        setup["otpauth_url"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/AutoCRM:")
    );
    // Stored sealed, not as the base32 the person typed.
    let stored: String = sqlx::query_scalar("SELECT totp_secret FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored.starts_with("v1:"));

    // Pending: sign-in does not ask yet.
    auth::login(&pool, &config, attempt(&email, "ua-a", None))
        .await
        .unwrap();

    let now = chrono::Utc::now().timestamp();
    let (status, _) = call(
        &pool,
        "POST",
        "/api/auth/two-factor/enable",
        &bearer,
        Some(json!({ "code": "000000" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // The step before now: still in the window, and not the one enabling burns below.
    let enable_code = totp::code_at(&secret, now / 30 - 1);
    let (status, body) = call(
        &pool,
        "POST",
        "/api/auth/two-factor/enable",
        &bearer,
        Some(json!({ "code": enable_code })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let r = auth::login(&pool, &config, attempt(&email, "ua-a", None)).await;
    assert!(
        matches!(
            r,
            Err(AppError::Rule {
                code: "totp_required",
                ..
            })
        ),
        "{:?}",
        r.err()
    );

    let code = totp::code_at(&secret, now / 30);
    auth::login(&pool, &config, attempt(&email, "ua-a", Some(code.clone())))
        .await
        .unwrap();
    let again = auth::login(&pool, &config, attempt(&email, "ua-a", Some(code))).await;
    assert!(
        matches!(
            again,
            Err(AppError::Rule {
                code: "totp_invalid",
                ..
            })
        ),
        "a code is good once"
    );

    // Disabling needs the password; afterwards no code is asked.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/auth/two-factor/disable",
        &bearer,
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    auth::login(&pool, &config, attempt(&email, "ua-a", None))
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn a_new_device_is_noticed_and_mailed_about(pool: PgPool) {
    let config = common::config();
    let email = format!("nd-{}@autotherm.test", common::rand_suffix());
    let hash = auth::hash_password(PASSWORD).unwrap();
    users::insert(&pool, &email, "Új Eszköz", Role::Office, &hash, false)
        .await
        .unwrap();

    let first = auth::login(&pool, &config, attempt(&email, "Chrome/129 Windows", None))
        .await
        .unwrap();
    assert!(
        !first.new_device,
        "the very first sign-in has nothing to compare with"
    );
    let same = auth::login(&pool, &config, attempt(&email, "Chrome/129 Windows", None))
        .await
        .unwrap();
    assert!(!same.new_device);
    let other = auth::login(&pool, &config, attempt(&email, "Firefox/130 Linux", None))
        .await
        .unwrap();
    assert!(other.new_device);

    let state = common::state(pool.clone());
    autocrm::service::email::new_device_alert(
        &state,
        &other.user,
        other.session_id,
        "Firefox, Linux",
        Some("203.0.113.9"),
    )
    .await
    .unwrap();
    let (to, body): (String, String) = sqlx::query_as(
        "SELECT to_address, body_text FROM email_messages WHERE template_key = 'new_device_login'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(to, email);
    assert!(
        body.contains("Firefox, Linux") && body.contains("203.0.113.9"),
        "{body}"
    );
    assert!(!body.contains("MISSING"), "{body}");
}

async fn issued_invoice(
    pool: &PgPool,
    order_id: i64,
    gross: i64,
    currency: &str,
    due_days: i32,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO invoices (order_id, number, kind, status, currency, issue_date, delivery_date,
                               payment_date, net_amount, vat_amount, gross_amount, payment_method, issued_at)
         VALUES ($1, 'AT2026-P' || $1 || '-' || $2, 'invoice', 'issued', $3, current_date - 20, current_date - 20,
                 current_date + $4, $2, 0, $2, 'TRANSFER', now())
         RETURNING id",
    )
    .bind(order_id)
    .bind(gross)
    .bind(currency)
    .bind(due_days)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn payments_add_up_to_paid_and_the_statement_shows_what_is_open(pool: PgPool) {
    let (office, user_id) = login(&pool, Role::Office).await;
    let user = autocrm::service::auth::AuthUser {
        user_id,
        session_id: 0,
        session_kind: SessionKind::Web,
        email: "x@autotherm.test".into(),
        display_name: "Iroda".into(),
        role: Role::Office,
        must_change_password: false,
        hr_access: false,
        permissions: vec![],
    };
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let invoice = issued_invoice(&pool, order.id, 10_000_000, "HUF", -5).await;

    let (status, list) = call(
        &pool,
        "POST",
        &format!("/api/invoices/{invoice}/payments"),
        &office,
        Some(json!({ "amount_minor": 4_000_000, "method": "TRANSFER", "note": "első részlet" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 1);

    // More than is open is refused.
    let (status, err) = call(
        &pool,
        "POST",
        &format!("/api/invoices/{invoice}/payments"),
        &office,
        Some(json!({ "amount_minor": 7_000_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(err["error"]["code"], "overpayment");

    let (paid_amount, paid_at): (i64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT paid_amount, paid_at FROM invoices WHERE id = $1")
            .bind(invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (paid_amount, paid_at.is_some()),
        (4_000_000, false),
        "partly paid is not paid"
    );

    // The statement: one open invoice, overdue, with the payment.
    let (status, st) = call(
        &pool,
        "GET",
        &format!("/api/partners/{}/statement", order.partner_id),
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{st}");
    let huf = st["totals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["currency"] == "HUF")
        .unwrap();
    assert_eq!(huf["outstanding"], 6_000_000);
    assert_eq!(huf["overdue"], 6_000_000);
    assert_eq!(huf["received"], 4_000_000);
    assert_eq!(st["payments"].as_array().unwrap().len(), 1);

    // The rest settles it.
    let (status, _) = call(
        &pool,
        "POST",
        &format!("/api/invoices/{invoice}/payments"),
        &office,
        Some(json!({ "amount_minor": 6_000_000 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let bucket: String = sqlx::query_scalar("SELECT bucket FROM invoice_buckets WHERE id = $1")
        .bind(invoice)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(bucket, "paid");

    // Removing one opens it again; "unpaid" clears the rest.
    let first_id = list["items"][0]["id"].as_i64().unwrap();
    let (status, _) = call(
        &pool,
        "DELETE",
        &format!("/api/invoices/{invoice}/payments/{first_id}"),
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (paid_amount, paid_at): (i64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT paid_amount, paid_at FROM invoices WHERE id = $1")
            .bind(invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((paid_amount, paid_at.is_some()), (6_000_000, false));
    let (status, _) = call(
        &pool,
        "POST",
        &format!("/api/invoices/{invoice}/paid"),
        &office,
        Some(json!({ "paid": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM invoice_payments WHERE invoice_id = $1")
            .bind(invoice)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2, "marking paid books the remainder as a payment");
}

fn lead_input(email: &str) -> LeadInput {
    LeadInput {
        title: "Hűtős furgon".into(),
        partner_id: None,
        contact_id: None,
        contact_name: Some("Nagy Anna".into()),
        contact_email: Some(email.into()),
        contact_phone: None,
        source: None,
        source_detail: None,
        description: None,
        assigned_to: None,
        quoted_value_minor: None,
        currency: None,
        quote_valid_until: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_reply_from_the_conversation_threads_under_the_customers_letter(pool: PgPool) {
    let (office, user_id) = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let lead = autocrm::service::leads::create(
        &pool,
        &user,
        lead_input("anna@example.hu"),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
    )
    .await
    .unwrap();
    let raw = b"From: Nagy Anna <anna@example.hu>\r\nTo: sales@autotherm.hu\r\nSubject: Kerdes\r\n\
Message-ID: <q1@example.hu>\r\nDate: Tue, 6 Oct 2026 10:00:00 +0200\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nMennyibe kerul?\r\n";
    let inbound = mailbox::ingest(&pool, raw, "sales@autotherm.hu")
        .await
        .unwrap()
        .unwrap();

    let (status, conv) = call(
        &pool,
        "GET",
        &format!("/api/leads/{}/conversation", lead.id),
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(conv["items"][0]["direction"], "in");

    let (status, sent) = call(
        &pool,
        "POST",
        "/api/emails",
        &office,
        Some(json!({
            "lead_id": lead.id,
            "to": "anna@example.hu",
            "subject": "Re: Kerdes",
            "body": "Kedves Anna!",
            "reply_to_inbound_id": inbound,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{sent}");
    let (irt, refs): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT in_reply_to, reference_ids FROM email_messages WHERE id = $1")
            .bind(sent["id"].as_i64().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(irt.as_deref(), Some("<q1@example.hu>"));
    assert!(refs.unwrap().contains("<q1@example.hu>"));
    let _ = user_id;

    let (_, conv) = call(
        &pool,
        "GET",
        &format!("/api/leads/{}/conversation", lead.id),
        &office,
        None,
    )
    .await;
    assert_eq!(conv["items"].as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_calendar_feed_is_private_and_shows_my_tasks(pool: PgPool) {
    let (bearer, user_id) = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    sqlx::query(
        "INSERT INTO tasks (entity_type, entity_id, title, due_date, assigned_to, created_by)
         VALUES ('order', $1, 'Visszahívni, árajánlat', current_date + 2, $2, $2)",
    )
    .bind(order.id)
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();

    let (status, feed) = call(&pool, "POST", "/api/auth/calendar", &bearer, None).await;
    assert_eq!(status, StatusCode::OK);
    let url = feed["url"].as_str().unwrap();
    let path = &url[url.find("/api/").unwrap()..];
    let (status, ics) = raw_get(&pool, path).await;
    assert_eq!(status, StatusCode::OK);
    assert!(ics.starts_with("BEGIN:VCALENDAR"));
    assert!(
        ics.contains("SUMMARY:Feladat: Visszahívni\\, árajánlat"),
        "{ics}"
    );

    let (status, _) = raw_get(&pool, "/api/calendar/not-a-token.ics").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // A new link retires the old one.
    call(&pool, "POST", "/api/auth/calendar", &bearer, None).await;
    let (status, _) = raw_get(&pool, path).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn readers_get_the_variant_in_their_language(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    sqlx::query("UPDATE settings SET automatic_email_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    for (email, lang) in [("hu@example.hu", None), ("de@example.de", Some("de"))] {
        sqlx::query(
            "INSERT INTO newsletter_subscriptions (email, name, source, confirmed_at, language)
             VALUES ($1, 'Olvasó', 'import', now(), $2)",
        )
        .bind(email)
        .bind(lang)
        .execute(&pool)
        .await
        .unwrap();
    }
    let compose = sends::Compose {
        subject: "Őszi ajánlat".into(),
        body: "Kedves Olvasó!".into(),
        body_markdown: false,
        hero: None,
        tag_ids: vec![],
        attachment_document_ids: vec![],
        embed_document_ids: vec![],
        send_at: chrono::Utc::now() - chrono::TimeDelta::seconds(1),
        variants: vec![autocrm::repo::newsletter::SendVariant {
            language: "DE".into(),
            subject: "Herbstangebot".into(),
            body: "Liebe Leser!".into(),
            body_markdown: false,
            hero: None,
        }],
    };
    let (send_id, _) = sends::schedule(&state, &user, &compose).await.unwrap();
    assert_eq!(sends::dispatch(&state, send_id).await.unwrap(), 2);
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT to_address, subject, body_text FROM email_messages WHERE newsletter_send_id = $1 ORDER BY to_address",
    )
    .bind(send_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows[0].0, "de@example.de");
    assert_eq!(rows[0].1, "Herbstangebot");
    assert!(rows[0].2.contains("Abmelden"));
    assert_eq!(rows[1].1, "Őszi ajánlat");
    assert!(rows[1].2.contains("Leiratkozás"));
}

#[sqlx::test(migrations = "./migrations")]
async fn leaving_makes_tasks_and_switches_the_account_off(pool: PgPool) {
    let (admin, _) = login(&pool, Role::Admin).await;
    let leaver = common::user(&pool, Role::Office).await;
    let leaver_bearer = bearer_for(&pool, leaver.user_id).await;
    let employee: i64 = sqlx::query_scalar(
        "INSERT INTO employees (full_name) VALUES ('Kilépő Károly') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, e) = call(
        &pool,
        "PUT",
        &format!("/api/hr/employees/{employee}/user"),
        &admin,
        Some(json!({ "user_id": leaver.user_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{e}");
    assert_eq!(e["user_id"], leaver.user_id);

    let (status, started) = call(
        &pool,
        "POST",
        &format!("/api/hr/employees/{employee}/checklists"),
        &admin,
        Some(json!({ "kind": "offboarding", "deactivate_user": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{started}");
    assert_eq!(started["tasks_created"], 6);
    assert_eq!(started["user_deactivated"], true);

    let (status, tasks) = call(
        &pool,
        "GET",
        &format!("/api/tasks/for/employee/{employee}"),
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks["items"].as_array().unwrap().len(), 6);
    // The account is off and its sessions are gone.
    let (status, _) = call(&pool, "GET", "/api/auth/me", &leaver_bearer, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Someone without HR cannot read a person's tasks.
    let (office, _) = login(&pool, Role::Office).await;
    let (status, _) = call(
        &pool,
        "GET",
        &format!("/api/tasks/for/employee/{employee}"),
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_reports_answer_and_a_vin_reads(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;
    for path in [
        "/api/reports/sales-funnel",
        "/api/reports/salespeople",
        "/api/reports/first-response",
        "/api/reports/revenue-by-country",
        "/api/reports/cumulative-flow",
        "/api/reports/newsletter-trends",
    ] {
        let (status, body) = call(&pool, "GET", path, &office, None).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
    }
    let (status, vin) = call(
        &pool,
        "GET",
        "/api/vehicles/decode/WDB906635K1234567",
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(vin["manufacturer"], "Mercedes-Benz");
    assert_eq!(vin["model_year"], 2019);
    let (status, _) = call(&pool, "GET", "/api/vehicles/decode/TOOSHORT", &office, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_google_click_becomes_a_conversion_row_once_won(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let lead = autocrm::service::leads::create(
        &pool,
        &user,
        lead_input("g@example.hu"),
        &[],
        chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap(),
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO lead_attribution (lead_id, channel, gclid) VALUES ($1, 'paid', 'Cj0KCQtest')",
    )
    .bind(lead.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE orders SET lead_id = $1 WHERE id = $2")
        .bind(lead.id)
        .bind(order.id)
        .execute(&pool)
        .await
        .unwrap();
    let rows = autocrm::service::ads::google_conversions(&pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let csv = autocrm::service::ads::google_csv(&rows, "Megrendeles", chrono_tz::Europe::Budapest);
    assert!(csv.contains("Cj0KCQtest,Megrendeles,"));
}
