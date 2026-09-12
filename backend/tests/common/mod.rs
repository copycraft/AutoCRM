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
            endpoint: Some("http://127.0.0.1:9".into()),
            region: "us-east-1".into(),
            bucket: "test".into(),
            access_key: "test".into(),
            secret_key: "test".into(),
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
