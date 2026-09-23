//! Tolerant search: a plate or phone in any spacing/prefix variant finds the record.
//!
//! "+36 30 ...", "06 30 ..." and "0036 30 ..." are the same number; "abc123"
//! is the same plate as "ABC-123". Each #[sqlx::test] gets a fresh database.

mod common;

use sqlx::PgPool;

use autocrm::domain::partner::PartnerKind;
use autocrm::domain::role::Role;
use autocrm::repo::leads::LeadInput;
use autocrm::repo::partners::PartnerInput;
use autocrm::repo::{leads, like_pattern, partners, phone_pattern, search, stages};

async fn phone_partner(pool: &PgPool) -> i64 {
    partners::insert(
        pool,
        &PartnerInput {
            kind: PartnerKind::Business,
            name: format!("Telefon Kft {}", common::rand_suffix()),
            tax_number: None,
            eu_tax_number: None,
            country: "HU".into(),
            default_currency: "HUF".into(),
            email: None,
            phone: Some("06-30-123-4567".into()),
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

async fn phone_lead(pool: &PgPool, user_id: i64) -> i64 {
    let lead = leads::insert(
        pool,
        &LeadInput {
            title: format!("Flotta ajánlat {}", common::rand_suffix()),
            partner_id: None,
            contact_id: None,
            contact_name: Some("Kereső Béla".into()),
            contact_email: None,
            contact_phone: Some("+36 30 123 4567".into()),
            source: None,
            description: None,
            assigned_to: None,
            quoted_value_minor: None,
            currency: None,
            quote_valid_until: None,
        },
        user_id,
    )
    .await
    .unwrap();
    stages::insert_lead_stage(pool, lead.id, "new", Some(user_id), None)
        .await
        .unwrap();
    lead.id
}

#[sqlx::test(migrations = "./migrations")]
async fn partner_phone_variants_all_match(pool: PgPool) {
    let id = phone_partner(&pool).await;
    for query in [
        "+36 30 123 4567",
        "06-30-123-4567",
        "0036301234567",
        "301234567",
        "30 123 4567",
    ] {
        let phone = phone_pattern(query);
        let rows = partners::search(
            &pool,
            None,
            phone.as_deref(),
            None,
            None,
            false,
            partners::DEFAULT_SORT,
            50,
            0,
        )
        .await
        .unwrap();
        assert!(
            rows.iter().any(|p| p.id == id),
            "partner list misses query {query:?}"
        );
        let hits = search::partners(&pool, query, 8).await.unwrap();
        assert!(
            hits.iter().any(|p| p.id == id),
            "global search misses query {query:?}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn lead_contact_phone_variants_all_match(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let id = phone_lead(&pool, user.user_id).await;
    for query in ["+36 30 123 4567", "06301234567", "301234567"] {
        let phone = phone_pattern(query);
        let rows = leads::search(
            &pool,
            None,
            phone.as_deref(),
            None,
            None,
            false,
            leads::DEFAULT_SORT,
            50,
            0,
        )
        .await
        .unwrap();
        assert!(
            rows.iter().any(|l| l.id == id),
            "lead list misses query {query:?}"
        );
        let hits = search::leads(&pool, query, 8).await.unwrap();
        assert!(
            hits.iter().any(|l| l.id == id),
            "global search misses query {query:?}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn digitless_query_matches_nothing_extra(pool: PgPool) {
    phone_partner(&pool).await;
    // No digits → no phone predicate; a nonsense name pattern must stay empty.
    let rows = partners::search(
        &pool,
        like_pattern("zzz-no-such-partner").as_deref(),
        None,
        None,
        None,
        false,
        partners::DEFAULT_SORT,
        50,
        0,
    )
    .await
    .unwrap();
    assert!(rows.is_empty());
    let hits = search::partners(&pool, "zzz-no-such-partner", 8)
        .await
        .unwrap();
    assert!(hits.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn legacy_plate_matches_without_dash_in_global_search(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    // Drop the vehicle link: the legacy orders.vehicle_plate column alone
    // must still match, like the list search does.
    sqlx::query("DELETE FROM order_vehicles WHERE order_id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .unwrap();
    let hits = search::orders(&pool, "abc123", 8).await.unwrap();
    assert!(
        hits.iter().any(|o| o.id == order.id),
        "global search misses legacy plate without dash"
    );
}
