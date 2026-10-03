//! HR leave and absence: booking, the overlap rule, working-day counting, balances, access.

mod common;

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

async fn admin_token(pool: &PgPool, role: Role) -> String {
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

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
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

async fn employee(pool: &PgPool, token: &str, name: &str, leave_days: Option<i64>) -> i64 {
    let mut body = json!({ "full_name": name });
    if let Some(d) = leave_days {
        body["annual_leave_days"] = json!(d);
    }
    let (status, e) = call(pool, "POST", "/api/hr/employees", token, Some(body)).await;
    assert_eq!(status, StatusCode::CREATED);
    e["id"].as_i64().unwrap()
}

fn absence(kind: &str, from: &str, to: &str) -> Value {
    json!({ "kind": kind, "start_date": from, "end_date": to })
}

#[sqlx::test(migrations = "./migrations")]
async fn leave_is_hr_only(pool: PgPool) {
    let admin = admin_token(&pool, Role::Admin).await;
    let office = admin_token(&pool, Role::Office).await;
    let id = employee(&pool, &admin, "Kiss Péter", None).await;
    for (method, uri, body) in [
        (
            "GET",
            "/api/hr/absences?from=2026-06-01&to=2026-06-30".to_string(),
            None,
        ),
        ("GET", "/api/hr/leave-summary".to_string(), None),
        (
            "POST",
            format!("/api/hr/employees/{id}/absences"),
            Some(absence("annual", "2026-06-08", "2026-06-12")),
        ),
        ("DELETE", "/api/hr/absences/1".to_string(), None),
    ] {
        let (status, _) = call(&pool, method, &uri, &office, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn booking_counts_working_days_and_refuses_overlaps(pool: PgPool) {
    let admin = admin_token(&pool, Role::Admin).await;
    let id = employee(&pool, &admin, "Kiss Péter", Some(25)).await;
    let url = format!("/api/hr/employees/{id}/absences");

    // Mon 2026-03-30 .. Fri 2026-04-10: Good Friday and Easter Monday do not count -> 8.
    let (status, a) = call(
        &pool,
        "POST",
        &url,
        &admin,
        Some(absence("annual", "2026-03-30", "2026-04-10")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(a["working_days"], 8);
    assert_eq!(a["employee_name"], "Kiss Péter");
    let first_id = a["id"].as_i64().unwrap();

    // Touching the period (even one day) is refused, with a code the UI can show.
    let (status, e) = call(
        &pool,
        "POST",
        &url,
        &admin,
        Some(absence("sick", "2026-04-10", "2026-04-14")),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(e["error"]["code"], "overlap");

    // The day after is fine.
    let (status, _) = call(
        &pool,
        "POST",
        &url,
        &admin,
        Some(absence("sick", "2026-04-13", "2026-04-14")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Bad input.
    for (body, why) in [
        (
            absence("annual", "2026-06-13", "2026-06-14"),
            "weekend only",
        ),
        (
            absence("annual", "2026-06-10", "2026-06-09"),
            "end before start",
        ),
        (absence("annual", "2026-01-01", "2027-06-01"), "over a year"),
        (
            json!({ "kind": "nap", "start_date": "2026-06-08", "end_date": "2026-06-08" }),
            "unknown kind",
        ),
    ] {
        let (status, _) = call(&pool, "POST", &url, &admin, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{why}");
    }

    // Removing one frees the period again.
    assert_eq!(
        call(
            &pool,
            "DELETE",
            &format!("/api/hr/absences/{first_id}"),
            &admin,
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(
            &pool,
            "DELETE",
            &format!("/api/hr/absences/{first_id}"),
            &admin,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = call(
        &pool,
        "POST",
        &url,
        &admin,
        Some(absence("annual", "2026-04-01", "2026-04-02")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // Unknown employee.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/hr/employees/99999/absences",
        &admin,
        Some(absence("annual", "2026-06-08", "2026-06-08")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_calendar_and_the_balances_add_up(pool: PgPool) {
    let admin = admin_token(&pool, Role::Admin).await;
    let a = employee(&pool, &admin, "Kiss Péter", Some(25)).await;
    let b = employee(&pool, &admin, "Nagy Anna", None).await;

    for (emp, kind, from, to) in [
        (a, "annual", "2026-03-30", "2026-04-10"), // 8 working days
        (a, "sick", "2026-06-01", "2026-06-02"),   // 2
        (a, "annual", "2026-12-28", "2027-01-08"), // spans the new year
        (b, "unpaid", "2026-05-04", "2026-05-08"), // 5
    ] {
        let (status, _) = call(
            &pool,
            "POST",
            &format!("/api/hr/employees/{emp}/absences"),
            &admin,
            Some(absence(kind, from, to)),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{kind} {from}");
    }

    // The calendar shows what touches the month, nothing else.
    let (_, june) = call(
        &pool,
        "GET",
        "/api/hr/absences?from=2026-06-01&to=2026-06-30",
        &admin,
        None,
    )
    .await;
    let june = june["items"].as_array().unwrap();
    assert_eq!(june.len(), 1);
    assert_eq!(june[0]["kind"], "sick");
    let (_, one) = call(
        &pool,
        "GET",
        &format!("/api/hr/absences?from=2026-01-01&to=2026-12-31&employee_id={b}"),
        &admin,
        None,
    )
    .await;
    assert_eq!(one["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        call(
            &pool,
            "GET",
            "/api/hr/absences?from=2026-06-30&to=2026-06-01",
            &admin,
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // 2026 balance: annual = 8 + the 2026 part of the year-end break (28, 29, 30, 31 Dec = 4).
    let (_, summary) = call(
        &pool,
        "GET",
        "/api/hr/leave-summary?year=2026",
        &admin,
        None,
    )
    .await;
    let rows = summary["items"].as_array().unwrap();
    let row = |id: i64| rows.iter().find(|r| r["employee_id"] == id).unwrap();
    assert_eq!(row(a)["allowance_days"], 25);
    assert_eq!(row(a)["used_annual"], 12);
    assert_eq!(row(a)["remaining"], 13);
    assert_eq!(row(a)["sick_days"], 2);
    assert_eq!(row(b)["allowance_days"], 20); // the default
    assert_eq!(row(b)["unpaid_days"], 5);
    assert_eq!(row(b)["remaining"], 20);

    // 2027 gets the other part: Jan 1 is a holiday, so 4, 5, 6, 7, 8 Jan = 5 working days.
    let (_, next) = call(
        &pool,
        "GET",
        "/api/hr/leave-summary?year=2027",
        &admin,
        None,
    )
    .await;
    let next = next["items"].as_array().unwrap();
    assert_eq!(
        next.iter().find(|r| r["employee_id"] == a).unwrap()["used_annual"],
        5
    );

    // Employees who have left drop out of the summary and take no new leave.
    call(
        &pool,
        "POST",
        &format!("/api/hr/employees/{b}/archive"),
        &admin,
        None,
    )
    .await;
    let (_, after) = call(
        &pool,
        "GET",
        "/api/hr/leave-summary?year=2026",
        &admin,
        None,
    )
    .await;
    assert_eq!(after["items"].as_array().unwrap().len(), 1);
    let (status, _) = call(
        &pool,
        "POST",
        &format!("/api/hr/employees/{b}/absences"),
        &admin,
        Some(absence("annual", "2026-09-01", "2026-09-01")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_allowance_is_editable(pool: PgPool) {
    let admin = admin_token(&pool, Role::Admin).await;
    let id = employee(&pool, &admin, "Kiss Péter", None).await;
    let (status, e) = call(
        &pool,
        "PATCH",
        &format!("/api/hr/employees/{id}"),
        &admin,
        Some(json!({ "annual_leave_days": 27 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(e["annual_leave_days"], 27);
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/hr/employees/{id}"),
        &admin,
        Some(json!({ "annual_leave_days": 500 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
