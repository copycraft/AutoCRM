//! Login lockout, as a stranger sees it.
//!
//! Ten wrong passwords lock an account. What the lock must not do is announce itself:
//! a locked account asked with a wrong password answers exactly like an address that has
//! no account, so locking cannot be used to find out which addresses exist.

mod common;

use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::sessions::SessionKind;
use autocrm::repo::users;
use autocrm::service::auth::{self, LoginRequest};
use sqlx::PgPool;

const PASSWORD: &str = "correct horse battery staple";

fn attempt(email: &str, password: &str) -> LoginRequest {
    LoginRequest {
        email: email.to_string(),
        password: password.to_string(),
        kind: SessionKind::Web,
        device_label: None,
        user_agent: None,
        ip: None,
        totp_code: None,
    }
}

async fn account(pool: &PgPool, email: &str) {
    let hash = auth::hash_password(PASSWORD).unwrap();
    users::insert(pool, email, "Teszt Elek", Role::Office, &hash, false)
        .await
        .unwrap();
}

async fn lock(pool: &PgPool, email: &str) {
    for _ in 0..10 {
        let r = auth::login(pool, &common::config(), attempt(email, "wrong password")).await;
        assert!(matches!(r, Err(AppError::Unauthenticated)), "{:?}", r.err());
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_locked_account_with_a_wrong_password_looks_like_no_account(pool: PgPool) {
    account(&pool, "locked@autotherm.test").await;
    lock(&pool, "locked@autotherm.test").await;

    let locked = auth::login(
        &pool,
        &common::config(),
        attempt("locked@autotherm.test", "still wrong"),
    )
    .await;
    let unknown = auth::login(
        &pool,
        &common::config(),
        attempt("nobody@autotherm.test", "still wrong"),
    )
    .await;
    assert!(
        matches!(locked, Err(AppError::Unauthenticated)),
        "{:?}",
        locked.err()
    );
    assert!(
        matches!(unknown, Err(AppError::Unauthenticated)),
        "{:?}",
        unknown.err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn the_right_password_on_a_locked_account_is_refused_and_told_why(pool: PgPool) {
    account(&pool, "owner@autotherm.test").await;
    lock(&pool, "owner@autotherm.test").await;

    let r = auth::login(
        &pool,
        &common::config(),
        attempt("owner@autotherm.test", PASSWORD),
    )
    .await;
    assert!(matches!(r, Err(AppError::TooManyRequests)), "{:?}", r.err());
}

#[sqlx::test(migrations = "./migrations")]
async fn below_the_threshold_the_right_password_still_works(pool: PgPool) {
    account(&pool, "typo@autotherm.test").await;
    for _ in 0..9 {
        let _ = auth::login(
            &pool,
            &common::config(),
            attempt("typo@autotherm.test", "wrong password"),
        )
        .await;
    }
    assert!(
        auth::login(
            &pool,
            &common::config(),
            attempt("typo@autotherm.test", PASSWORD)
        )
        .await
        .is_ok()
    );
}
