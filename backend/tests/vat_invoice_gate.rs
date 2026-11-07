//! The invoice gate: a 0% invoice to a business in another member state is issued only on a
//! fresh VALID VIES answer about the buyer's EU VAT number, and that answer is kept with
//! the invoice. INVALID blocks, an outage blocks unless overrides are on and a reason is
//! given, and nothing else is touched by any of it.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use autocrm::AppState;
use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::invoices;
use autocrm::service::invoicing::{self, IssueRequest};
use autocrm::service::orders::NewItem;
use axum::routing::post;
use serde_json::{Value, json};
use sqlx::PgPool;

/// Number 100 is valid, 200 is not, anything else finds the member state down.
async fn stub_vies() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let app = axum::Router::new().route(
        "/check-vat-number",
        post(move |axum::Json(body): axum::Json<Value>| {
            counter.fetch_add(1, Ordering::SeqCst);
            async move {
                axum::Json(match body["vatNumber"].as_str().unwrap() {
                    "100" => json!({ "valid": true, "name": "KUNDE GMBH" }),
                    "200" => json!({ "valid": false }),
                    _ => json!({ "actionSucceed": false, "errorWrappers": [{ "error": "MS_UNAVAILABLE" }] }),
                })
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), calls)
}

/// Invoicing configured (the sidecar is only called by the job, which no test runs here),
/// VIES pointed at the stub.
async fn state(pool: &PgPool, allow_override: bool) -> (AppState, Arc<AtomicUsize>) {
    let (url, calls) = stub_vies().await;
    let mut state = common::state_with_nav(
        pool.clone(),
        "http://127.0.0.1:9",
        common::MOCK_SUPPLIER_TAX_NUMBER,
    );
    let mut config = (*state.config).clone();
    config.vies.base_url = url;
    config.vies.backoff_base = Duration::ZERO;
    config.vies_invoice_allow_override = allow_override;
    state.config = Arc::new(config);
    (state, calls)
}

/// An order for a German business with `eu_number` (None: no EU VAT number at all).
async fn eu_order(pool: &PgPool, eu_number: Option<&str>) -> (i64, i64) {
    let user = common::user(pool, Role::Admin).await;
    let order = common::invoiceable_order(
        pool,
        &user,
        "EUR",
        vec![NewItem {
            description: "Kühlaufbau".into(),
            quantity: "1".parse().unwrap(),
            unit_price: 1_000_000,
        }],
    )
    .await;
    sqlx::query(
        "UPDATE partners SET country = 'DE', tax_number = NULL, eu_tax_number = $2,
                postal_code = '80331', city = 'München', address_line = 'Kaufinger utca 1.'
         WHERE id = $1",
    )
    .bind(order.partner_id)
    .bind(eu_number)
    .execute(pool)
    .await
    .unwrap();
    // A rate for the fulfilment date, so only the gate can stop the invoice.
    sqlx::query("INSERT INTO fx_rates (day, base, quote, rate) VALUES (CURRENT_DATE - 1, 'EUR', 'HUF', 400) ON CONFLICT DO NOTHING")
        .execute(pool)
        .await
        .unwrap();
    (order.id, user.user_id)
}

fn zero_vat() -> IssueRequest {
    IssueRequest {
        vat_rate: Some("0".parse().unwrap()),
        ..IssueRequest::default()
    }
}

fn rule_code(e: AppError) -> &'static str {
    match e {
        AppError::Rule { code, .. } => code,
        other => panic!("expected a rule error, got {other:?}"),
    }
}

async fn invoice_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM invoices")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn a_valid_number_lets_the_invoice_through_and_is_kept_with_it(pool: PgPool) {
    let (state, calls) = state(&pool, false).await;
    let (order_id, user_id) = eu_order(&pool, Some("DE100")).await;
    let user = common::user(&pool, Role::Admin).await;
    let _ = user_id;

    let invoice = invoicing::create_invoice(&state, &user, order_id, &zero_vat())
        .await
        .unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "no check on file: VIES is asked now"
    );

    let (check_id, status): (Option<i64>, String) = sqlx::query_as(
        "SELECT i.vat_check_id, c.status FROM invoices i JOIN vat_checks c ON c.id = i.vat_check_id WHERE i.id = $1",
    )
    .bind(invoice.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(check_id.is_some());
    assert_eq!(status, "VALID");
    assert!(invoices::find(&pool, invoice.id).await.unwrap().is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_fresh_verdict_on_file_is_reused(pool: PgPool) {
    let (state, calls) = state(&pool, false).await;
    let (order_id, _) = eu_order(&pool, Some("DE100")).await;
    let user = common::user(&pool, Role::Admin).await;
    // Checked an hour ago.
    autocrm::service::vat::check(
        &pool,
        &state.config,
        autocrm::service::vat::CheckRequest {
            input: "DE100",
            default_country: None,
            partner_id: None,
            force: true,
            created_by: "test".into(),
            interactive: true,
            bulk: false,
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    invoicing::create_invoice(&state, &user, order_id, &zero_vat())
        .await
        .unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a VALID answer inside 24 h is enough"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_stale_verdict_is_asked_again(pool: PgPool) {
    let (state, calls) = state(&pool, false).await;
    let (order_id, _) = eu_order(&pool, Some("DE100")).await;
    let user = common::user(&pool, Role::Admin).await;
    autocrm::service::vat::check(
        &pool,
        &state.config,
        autocrm::service::vat::CheckRequest {
            input: "DE100",
            default_country: None,
            partner_id: None,
            force: true,
            created_by: "test".into(),
            interactive: true,
            bulk: false,
        },
    )
    .await
    .unwrap()
    .unwrap();
    // The table is append-only, so age the answer by shrinking the window instead.
    let mut config = (*state.config).clone();
    config.vies_invoice_max_age_hours = 0;
    let state = AppState {
        config: Arc::new(config),
        ..state
    };

    invoicing::create_invoice(&state, &user, order_id, &zero_vat())
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_invalid_number_blocks_and_draws_no_number(pool: PgPool) {
    let (state, _) = state(&pool, true).await;
    let (order_id, _) = eu_order(&pool, Some("DE200")).await;
    let user = common::user(&pool, Role::Admin).await;
    let mut req = zero_vat();
    // Even with overrides on: INVALID is a verdict, not an outage.
    req.vat_override_reason = Some("ügyfél kérte".into());
    let e = invoicing::create_invoice(&state, &user, order_id, &req)
        .await
        .unwrap_err();
    assert_eq!(rule_code(e), "eu_vat_number_invalid");
    assert_eq!(invoice_count(&pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_outage_blocks_unless_overridden_with_a_reason(pool: PgPool) {
    let (state, _) = state(&pool, false).await;
    let (order_id, _) = eu_order(&pool, Some("DE301")).await;
    let user = common::user(&pool, Role::Admin).await;
    let mut req = zero_vat();
    req.vat_override_reason = Some("VIES leállás, a vevő igazolta".into());

    // Overrides off: a reason changes nothing.
    let e = invoicing::create_invoice(&state, &user, order_id, &req)
        .await
        .unwrap_err();
    assert_eq!(rule_code(e), "eu_vat_check_unavailable");
    assert_eq!(invoice_count(&pool).await, 0);

    // Overrides on: no reason, no invoice; with one, it goes through and says why.
    let (state, _) = self::state(&pool, true).await;
    let e = invoicing::create_invoice(&state, &user, order_id, &zero_vat())
        .await
        .unwrap_err();
    assert_eq!(rule_code(e), "eu_vat_check_unavailable");
    let invoice = invoicing::create_invoice(&state, &user, order_id, &req)
        .await
        .unwrap();
    let reason: Option<String> =
        sqlx::query_scalar("SELECT vat_override_reason FROM invoices WHERE id = $1")
            .bind(invoice.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("VIES leállás, a vevő igazolta"));
}

#[sqlx::test(migrations = "./migrations")]
async fn no_eu_number_means_no_zero_rate_eu_invoice(pool: PgPool) {
    let (state, calls) = state(&pool, false).await;
    let (order_id, _) = eu_order(&pool, None).await;
    let user = common::user(&pool, Role::Admin).await;
    let e = invoicing::create_invoice(&state, &user, order_id, &zero_vat())
        .await
        .unwrap_err();
    assert_eq!(rule_code(e), "eu_vat_number_required");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_vat_charging_invoice_is_not_gated(pool: PgPool) {
    let (state, calls) = state(&pool, false).await;
    // Even an INVALID number: with VAT charged, no EU treatment is claimed.
    let (order_id, _) = eu_order(&pool, Some("DE200")).await;
    let user = common::user(&pool, Role::Admin).await;
    let req = IssueRequest {
        vat_rate: Some("0.27".parse().unwrap()),
        ..IssueRequest::default()
    };
    let invoice = invoicing::create_invoice(&state, &user, order_id, &req)
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let check: Option<i64> = sqlx::query_scalar("SELECT vat_check_id FROM invoices WHERE id = $1")
        .bind(invoice.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(check, None);
}
