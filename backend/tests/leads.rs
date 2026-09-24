//! Lead relations: the partner exists and takes new work, the contact exists and
//! belongs to the partner — on create and, since the second sweep, on update too.

mod common;

use sqlx::PgPool;

use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::contacts::{self, ContactInput};
use autocrm::repo::leads::LeadInput;
use autocrm::service::leads;

fn input(partner_id: Option<i64>, contact_id: Option<i64>) -> LeadInput {
    LeadInput {
        title: "Hűtőkamra".into(),
        partner_id,
        contact_id,
        contact_name: None,
        contact_email: None,
        contact_phone: None,
        source: None,
        description: None,
        assigned_to: None,
        quoted_value_minor: None,
        currency: None,
        quote_valid_until: None,
    }
}

fn validation_message(error: AppError) -> String {
    match error {
        AppError::Validation(message) => message,
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn lead_relations_are_checked_the_same_on_every_write(pool: PgPool) {
    let a = common::partner(&pool, "Partner A", None).await;
    let b = common::partner(&pool, &format!("Partner B {}", common::rand_suffix()), None).await;
    let contact = contacts::insert(
        &pool,
        a,
        &ContactInput {
            name: "Kapcsolat".into(),
            email: None,
            phone: None,
            position: None,
            notes: None,
        },
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    // The consistent pair passes.
    leads::check_relations(&mut conn, Some(a), Some(contact.id))
        .await
        .unwrap();
    // A contact kept while the partner is swapped does not.
    assert_eq!(
        validation_message(
            leads::check_relations(&mut conn, Some(b), Some(contact.id))
                .await
                .unwrap_err()
        ),
        "contact belongs to a different partner",
    );
    // Dangling references do not.
    assert_eq!(
        validation_message(
            leads::check_relations(&mut conn, Some(999_999_999), None)
                .await
                .unwrap_err()
        ),
        "partner does not exist",
    );
    assert_eq!(
        validation_message(
            leads::check_relations(&mut conn, Some(a), Some(999_999_999))
                .await
                .unwrap_err()
        ),
        "contact does not exist",
    );
    // An archived partner takes no new work — on leads as on orders.
    sqlx::query("UPDATE partners SET archived_at = now() WHERE id = $1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        validation_message(
            leads::check_relations(&mut conn, Some(b), None)
                .await
                .unwrap_err()
        ),
        "partner is archived",
    );
    drop(conn);

    // …including on create.
    let user = common::user(&pool, Role::Office).await;
    assert_eq!(
        validation_message(
            leads::create(&pool, &user, input(Some(b), None))
                .await
                .unwrap_err()
        ),
        "partner is archived",
    );
    // …while a live partner still works.
    let lead = leads::create(&pool, &user, input(Some(a), Some(contact.id)))
        .await
        .unwrap();
    assert_eq!(lead.partner_id, Some(a));
}
