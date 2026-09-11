//! MiniCRM load + reconcile against fixture data shaped like the R3 API output.

mod common;

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde_json::{Value, json};
use sqlx::PgPool;

use autocrm::migration::load::{self, Mapping};
use autocrm::migration::reconcile;

fn write(path: PathBuf, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn fixture() -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("autocrm-migration-test-{}", rand::random::<u64>()));
    let raw = dir.join("raw");
    write(
        raw.join("contacts/100.json"),
        &json!({"Id": 100, "Type": "Business", "Name": "Müller Kühlung GmbH", "Email": "info@mueller.example", "BusinessId": 0}),
    );
    write(
        raw.join("contacts/101.json"),
        &json!({"Id": 101, "Type": "Person", "FirstName": "Michael", "LastName": "Müller", "Email": "michael@mueller.example", "BusinessId": 100}),
    );
    write(
        raw.join("contacts/102.json"),
        &json!({"Id": 102, "Type": "Person", "Name": "Kovács János", "Email": "nem-email", "BusinessId": ""}),
    );
    write(
        raw.join("projects/500.json"),
        &json!({
            "Id": 500, "Name": "Iveco hűtős felépítmény", "CategoryId": 12, "ContactId": 101, "StatusId": 3003,
            "CreatedAt": "2019-05-10 12:33:00", "Osszeg": "1 234 567,50", "Egyedi_mezo": "megmarad",
            "Atveteli_fotok": ["https://r3.minicrm.hu/files/500/IMG_1.jpg"]
        }),
    );
    write(
        raw.join("projects/501.json"),
        &json!({"Id": 501, "Name": "Érdeklődés", "CategoryId": 15, "ContactId": 102, "StatusId": 2002}),
    );
    write(
        raw.join("projects/502.json"),
        &json!({"Id": 502, "Name": "Ismeretlen kategória", "CategoryId": 99, "ContactId": 100}),
    );
    write(
        raw.join("projects/503.json"),
        &json!({"Id": 503, "Name": "Hiányzó kontakt", "CategoryId": 12, "ContactId": 999}),
    );

    let sha = "ab".repeat(32);
    std::fs::write(
        dir.join("manifest.jsonl"),
        format!(
            "{}\n",
            json!({"source_url": "https://r3.minicrm.hu/files/500/IMG_1.jpg", "minicrm_project_id": 500, "minicrm_contact_id": 101,
                   "field": "$.Atveteli_fotok[0]", "filename": "IMG_1.jpg", "extension": "jpg"})
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("fetched.jsonl"),
        format!(
            "{}\n",
            json!({"source_url": "https://r3.minicrm.hu/files/500/IMG_1.jpg", "sha256": sha, "byte_size": 2048,
                   "content_type": "image/jpeg", "storage_key": format!("migration/originals/{sha}.jpg"), "fetched_at": "2026-09-11T10:00:00Z"})
        ),
    )
    .unwrap();
    dir
}

fn mapping(dir: &Path) -> Mapping {
    let path = dir.join("mapping.json");
    write(
        path.clone(),
        &json!({
            "categories": {"12": "order", "15": "lead"},
            "order_statuses": {"3003": "production"},
            "lead_statuses": {"2002": "contacted"},
            "default_order_stage": "completed",
            "default_lead_stage": "lost",
            "image_fields": {"$.Atveteli_fotok[]": "intake"},
            "order_value_field": "Osszeg"
        }),
    );
    Mapping::load(&path).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn load_is_complete_idempotent_and_reconcilable(pool: PgPool) {
    let state = common::state(pool.clone());
    let dir = fixture();
    let mapping = mapping(&dir);
    let raw = dir.join("raw");

    let first = load::run(&state, &mapping, &raw, &dir).await.unwrap();
    assert_eq!(
        (
            first.partners,
            first.contacts,
            first.orders,
            first.leads,
            first.images
        ),
        (2, 1, 1, 1, 1)
    );
    assert_eq!(first.skipped_projects, 2);
    assert!(
        first
            .problems
            .iter()
            .any(|p| p.contains("category Some(99)")),
        "{:?}",
        first.problems
    );
    assert!(
        first.problems.iter().any(|p| p.contains("project 503")),
        "{:?}",
        first.problems
    );
    assert!(
        first.problems.iter().any(|p| p.contains("nem-email")),
        "{:?}",
        first.problems
    );

    // Re-running changes nothing and creates no duplicates.
    let second = load::run(&state, &mapping, &raw, &dir).await.unwrap();
    assert_eq!(second.images, 0);
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stages: i64 = sqlx::query_scalar("SELECT count(*) FROM order_stages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((orders, stages), (1, 1));

    let order = sqlx::query!(
        r#"SELECT o.id, o.number, o.valuation_date, o.raw_import, p.name AS partner, c.name AS "contact?",
                  (SELECT stage_key FROM order_stages s WHERE s.order_id = o.id) AS "stage!",
                  (SELECT unit_price FROM order_items i WHERE i.order_id = o.id) AS "unit_price!"
           FROM orders o JOIN partners p ON p.id = o.partner_id LEFT JOIN contacts c ON c.id = o.contact_id
           WHERE o.minicrm_id = 500"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(order.number, "MC-500");
    assert_eq!(order.partner, "Müller Kühlung GmbH");
    assert_eq!(order.contact.as_deref(), Some("Müller Michael"));
    assert_eq!(order.stage, "production");
    assert_eq!(order.unit_price, 123_456_750);
    assert_eq!(
        order.valuation_date,
        NaiveDate::from_ymd_opt(2019, 5, 10).unwrap()
    );
    assert_eq!(
        order.raw_import.unwrap()["Egyedi_mezo"],
        "megmarad",
        "unmapped fields are preserved"
    );

    let image = sqlx::query!("SELECT category::text AS \"category!\", immutable, source_ref FROM images WHERE order_id = $1", order.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(image.category, "intake");
    assert!(image.immutable);
    assert!(image.source_ref.unwrap().ends_with("IMG_1.jpg"));
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'process_image'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    let lead_stage: String = sqlx::query_scalar("SELECT ls.stage_key FROM lead_stages ls JOIN leads l ON l.id = ls.lead_id WHERE l.minicrm_id = 501")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lead_stage, "contacted");

    let report = reconcile::run(&state, &mapping, &raw, &dir).await.unwrap();
    assert!(report.contains("| Orders | 2 | 1 | ⚠️ |"), "{report}");
    assert!(report.contains("MiniCRM project 503"), "{report}");
    assert!(
        report.contains("| Partners (businesses + standalone people) | 2 | 2 | ✅ |"),
        "{report}"
    );
    assert!(
        report.contains("Projects in unmapped categories: **1**"),
        "{report}"
    );

    let _ = std::fs::remove_dir_all(dir);
}
