//! Shared fixtures for integration tests. Each #[sqlx::test] gets a fresh, migrated database.
#![allow(dead_code)]

use std::sync::Arc;

use chrono::NaiveDate;
use sqlx::PgPool;

use autocrm::AppState;
use autocrm::config::{AppEnv, Config, EmailConfig, EmailTransportConfig, LogFormat, S3Config};
use autocrm::domain::partner::PartnerKind;
use autocrm::domain::role::Role;
use autocrm::media::storage::Storage;
use autocrm::repo::orders::{Order, OrderFields};
use autocrm::repo::partners::{self, PartnerInput};
use autocrm::repo::sessions::SessionKind;
use autocrm::repo::users;
use autocrm::service::auth::AuthUser;
use autocrm::service::orders::{NewItem, create_in_tx};

pub fn config() -> Config {
    Config {
        env: AppEnv::Dev,
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        allowed_origins: vec!["http://localhost:3000".into()],
        cookie_secure: false,
        public_base_url: "http://localhost:3000".into(),
        database_url: String::new(),
        database_max_connections: 5,
        upload_signing_key: vec![7; 32],
        s3: S3Config {
            // Unreachable by default: storage tests skip. TEST_S3_ENDPOINT=http://127.0.0.1:9000
            // (with the docker-compose minio and its dev credentials) runs them for real.
            endpoint: Some(
                std::env::var("TEST_S3_ENDPOINT").unwrap_or_else(|_| "http://127.0.0.1:9".into()),
            ),
            region: "us-east-1".into(),
            bucket: if std::env::var("TEST_S3_ENDPOINT").is_ok() {
                "autocrm"
            } else {
                "test"
            }
            .into(),
            access_key: if std::env::var("TEST_S3_ENDPOINT").is_ok() {
                "autocrm"
            } else {
                "test"
            }
            .into(),
            secret_key: if std::env::var("TEST_S3_ENDPOINT").is_ok() {
                "autocrm-dev-secret"
            } else {
                "test"
            }
            .into(),
            force_path_style: true,
            intake_lock_years: 0,
        },
        email: EmailConfig {
            transport: EmailTransportConfig::DryRun,
            from_automatic: "noreply@autotherm.hu".into(),
            from_name: "Autotherm".into(),
            reply_to_default: "iroda@autotherm.hu".into(),
            message_id_domain: "autotherm.test".into(),
            sender_domains: vec![],
            redirect_to: None,
        },
        mnb_endpoint: "http://127.0.0.1:9/mnb".into(),
        // Invoicing off by default: a CRM with no NAV sidecar configured is a supported
        // state, and most tests have nothing to do with it.
        nav: None,
        newsletter_api_key: None,
        leads_api_key: None,
        business_tz: chrono_tz::Europe::Budapest,
        worker_enabled: false,
        worker_id: "test".into(),
        log_format: LogFormat::Pretty,
        log_dir: None,
    }
}

pub fn state(pool: PgPool) -> AppState {
    let config = config();
    let storage = Storage::new(&config.s3);
    AppState {
        db: pool,
        config: Arc::new(config),
        storage,
    }
}

pub async fn user(pool: &PgPool, role: Role) -> AuthUser {
    let email = format!(
        "{}-{}@autotherm.test",
        format!("{role:?}").to_lowercase(),
        rand_suffix()
    );
    let u = users::insert(
        pool,
        &email,
        "Teszt Elek",
        role,
        "$argon2id$not-used",
        false,
    )
    .await
    .unwrap();
    AuthUser {
        user_id: u.id,
        session_id: 0,
        session_kind: SessionKind::Web,
        email: u.email,
        display_name: u.display_name,
        role,
        must_change_password: false,
        hr_access: false,
    }
}

pub fn rand_suffix() -> u32 {
    rand::random::<u32>()
}

pub async fn partner(pool: &PgPool, name: &str, email: Option<&str>) -> i64 {
    partners::insert(
        pool,
        &PartnerInput {
            kind: PartnerKind::Business,
            name: name.into(),
            tax_number: None,
            eu_tax_number: None,
            country: "HU".into(),
            default_currency: "HUF".into(),
            email: email.map(str::to_string),
            phone: None,
            website: None,
            postal_code: None,
            city: None,
            address_line: None,
            notes: None,
            role: None,
        },
    )
    .await
    .unwrap()
    .id
}

pub fn fields(partner_id: i64, currency: &str) -> OrderFields {
    OrderFields {
        title: "Hűtőfelépítmény".into(),
        partner_id,
        contact_id: None,
        project_type_id: None,
        currency: currency.into(),
        valuation_date: NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
        vehicle_make: Some("Iveco".into()),
        vehicle_model: Some("Daily".into()),
        vehicle_plate: Some("ABC-123".into()),
        vehicle_vin: None,
        description: None,
        due_date: None,
        assigned_to: None,
        related_order_id: None,
        relation: None,
        mileage_in: None,
        intake_condition: None,
        fuel_level: None,
        key_count: None,
        valuables_declared: None,
        valuables: None,
    }
}

pub async fn order(pool: &PgPool, user: &AuthUser, currency: &str, items: Vec<NewItem>) -> Order {
    let partner_id = partner(
        pool,
        &format!("Partner {}", rand_suffix()),
        Some("partner@example.hu"),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
    let order = create_in_tx(
        &mut tx,
        user.user_id,
        today,
        fields(partner_id, currency),
        items,
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    order
}

// ── The NAV invoicing sidecar ───────────────────────────────────────────────

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use autocrm::config::{NavConfig, SupplierConfig};

/// The fake taxpayer the bundled mock NAV service authenticates as. An invoice whose
/// supplier is anyone else is rejected — which is also how the rejection path is tested.
pub const MOCK_SUPPLIER_TAX_NUMBER: &str = "99999999";

/// A sidecar process running in MOCK_MODE, killed when the test ends.
pub struct Sidecar {
    pub url: String,
    child: Child,
}

impl Drop for Sidecar {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn sidecar_entrypoint() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("nav-sidecar")
        .join("dist")
        .join("index.js")
}

/// The token the test sidecar demands and the test config sends.
pub const SIDECAR_TOKEN: &str = "test-sidecar-token-0123456789abcdef";

/// Starts the sidecar in mock mode on a free port, or `None` when it is not built.
///
/// `None` rather than a panic: the sidecar is a separate Node service, and a Rust-only
/// checkout should still be able to run `cargo test`. Every test that needs it says out
/// loud that it was skipped.
pub fn start_sidecar() -> Option<Sidecar> {
    let entrypoint = sidecar_entrypoint();
    if !entrypoint.exists() {
        eprintln!(
            "skipping: {} is not built — run `npm ci && npm run build` in nav-sidecar/",
            entrypoint.display()
        );
        return None;
    }
    let mut child = match Command::new("node")
        .arg(&entrypoint)
        .env("MOCK_MODE", "true")
        .env("PORT", "0")
        .env("HOST", "127.0.0.1")
        // Mock mode would run without one; set it so every test goes through the check.
        .env("SIDECAR_TOKEN", SIDECAR_TOKEN)
        // The mock settles a transaction immediately; no need to pace the polling.
        .env("NAV_POLL_INITIAL_DELAY_MS", "10")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("skipping: could not start the NAV sidecar ({e}); is node installed?");
            return None;
        }
    };

    // The sidecar prints where it is listening once it is ready, which is both the port
    // and the readiness signal.
    let stdout = child.stdout.take().expect("piped stdout");
    let mut reader = BufReader::new(stdout);
    let mut url = None;
    for _ in 0..20 {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(rest) = line.split("listening on ").nth(1) {
            url = rest.split_whitespace().next().map(str::to_string);
            break;
        }
    }
    match url {
        Some(url) => Some(Sidecar { url, child }),
        None => {
            let _ = child.kill();
            eprintln!("skipping: the NAV sidecar did not report a listening address");
            None
        }
    }
}

/// App state wired to a sidecar, reporting as `supplier_tax_number`.
///
/// Pass anything other than [`MOCK_SUPPLIER_TAX_NUMBER`] to have the mock refuse the
/// report the way NAV refuses one filed for another taxpayer.
pub fn state_with_nav(pool: PgPool, sidecar_url: &str, supplier_tax_number: &str) -> AppState {
    let mut config = config();
    // Documents are real objects: point at the dev MinIO, and let the tests that need it
    // check whether it is actually up.
    config.s3 = S3Config {
        endpoint: Some(
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".into()),
        ),
        region: "us-east-1".into(),
        bucket: std::env::var("S3_BUCKET").unwrap_or_else(|_| "autocrm".into()),
        access_key: std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "autocrm".into()),
        secret_key: std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "autocrm-dev-secret".into()),
        force_path_style: true,
        intake_lock_years: 0,
    };
    config.nav = Some(NavConfig {
        sidecar_url: sidecar_url.trim_end_matches('/').to_string(),
        sidecar_token: SIDECAR_TOKEN.into(),
        timeout: std::time::Duration::from_secs(30),
        supplier: SupplierConfig {
            name: "Autotherm Kft".into(),
            tax_number: supplier_tax_number.into(),
            postal_code: "1117".into(),
            city: "Budapest".into(),
            street_name: "Kossuth".into(),
            public_place_category: "utca".into(),
            number: "12".into(),
            bank_account: Some("12345678-12345678-12345678".into()),
        },
        default_vat_rate: "0.27".parse().unwrap(),
        invoice_prefix: "AT".into(),
        proforma_prefix: "DB".into(),
        payment_days: 8,
    });
    let storage = Storage::new(&config.s3);
    AppState {
        db: pool,
        config: Arc::new(config),
        storage,
    }
}

/// A partner that can actually be invoiced: tax number, and an address NAV can read.
pub async fn invoiceable_partner(pool: &PgPool, email: Option<&str>) -> i64 {
    partners::insert(
        pool,
        &PartnerInput {
            kind: PartnerKind::Business,
            name: format!("Beszerző Kft {}", rand_suffix()),
            tax_number: Some("99887764-2-02".into()),
            eu_tax_number: None,
            country: "HU".into(),
            default_currency: "HUF".into(),
            email: email.map(str::to_string),
            phone: None,
            website: None,
            postal_code: Some("6000".into()),
            city: Some("Kecskemét".into()),
            address_line: Some("Petőfi Sándor utca 3.".into()),
            notes: None,
            role: None,
        },
    )
    .await
    .unwrap()
    .id
}

/// Whether the object store is reachable. Invoice and proforma PDFs are real files.
pub async fn storage_available(state: &AppState) -> bool {
    if state.storage.check().await.is_ok() {
        return true;
    }
    eprintln!(
        "skipping: the object store is not reachable — run `docker compose up -d minio minio-init`"
    );
    false
}

/// An order whose partner carries everything an invoice needs.
pub async fn invoiceable_order(
    pool: &PgPool,
    user: &AuthUser,
    currency: &str,
    items: Vec<NewItem>,
) -> Order {
    let partner_id = invoiceable_partner(pool, Some("vevo@example.hu")).await;
    let mut tx = pool.begin().await.unwrap();
    let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
    let order = create_in_tx(
        &mut tx,
        user.user_id,
        today,
        fields(partner_id, currency),
        items,
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    order
}
