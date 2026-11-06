//! Files on the share (STORAGE_BACKEND=fs): every upload lands in its project folder by the
//! same three steps the phone and the browser always took, downloads come back through the
//! signed links, and re-sorting files whatever was stored before in its folder.

mod common;

use std::path::Path;
use std::time::Duration;

use autocrm::AppState;
use autocrm::api;
use autocrm::domain::invoice::InvoiceKind;
use autocrm::domain::media::{DocumentKind, ImageCategory};
use autocrm::domain::role::Role;
use autocrm::media::upload_token::UploadTarget;
use autocrm::repo::documents::{self, NewDocument, Owner};
use autocrm::repo::images::{self, NewImage};
use autocrm::repo::invoices::{self, NewInvoice};
use autocrm::service::auth::AuthUser;
use autocrm::service::media::{
    Completed, UploadRequest, UploadResponse, complete_upload, request_upload,
};
use autocrm::service::relocate::{self, Options, Scope};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;

/// The path part of a link the server signed for PUBLIC_BASE_URL.
fn path_of(url: &str) -> &str {
    url.strip_prefix("http://localhost:3000")
        .expect("link on the public address")
}

async fn send(
    state: &AppState,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = api::router(state.clone()).oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, body)
}

/// Ticket, PUT to the share, complete: the phone's three steps.
async fn upload(
    state: &AppState,
    user: &AuthUser,
    owner: Owner,
    target: UploadTarget,
    filename: &str,
    content_type: &str,
    bytes: &[u8],
) -> Completed {
    let ticket = request_upload(
        state,
        user,
        owner,
        UploadRequest {
            target,
            filename: Some(filename.into()),
            content_type: content_type.into(),
            byte_size: bytes.len() as i64,
            sha256: hex::encode(Sha256::digest(bytes)),
        },
    )
    .await
    .unwrap();
    let UploadResponse::Upload { ticket, upload, .. } = ticket else {
        panic!("expected an upload ticket");
    };
    assert_eq!(upload.method, "PUT");
    let mut req = Request::builder().method("PUT").uri(path_of(&upload.url));
    for (name, value) in &upload.headers {
        req = req.header(name, value);
    }
    let (status, _, body) = send(state, req.body(Body::from(bytes.to_vec())).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    complete_upload(state, user, &ticket).await.unwrap()
}

fn on_share(dir: &Path, key: &str) -> Vec<u8> {
    std::fs::read(dir.join(key)).unwrap_or_else(|e| panic!("{key} on the share: {e}"))
}

#[sqlx::test(migrations = "./migrations")]
async fn uploads_are_plain_files_in_the_project_folder(pool: PgPool) {
    let (state, dir) = common::state_fs(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let partner_id = common::partner(&pool, "Kovács és Társa Kft.", None).await;
    let mut fields = common::fields(partner_id, "HUF");
    fields.vehicle_plate = Some("ABC 123".into());
    let mut tx = pool.begin().await.unwrap();
    let order = autocrm::service::orders::create_in_tx(
        &mut tx,
        user.user_id,
        NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
        fields,
        vec![],
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let photo = b"not really a jpeg, but bytes are bytes".to_vec();
    let Completed::Image { image, created } = upload(
        &state,
        &user,
        Owner::Order(order.id),
        UploadTarget::Image {
            category: ImageCategory::Intake,
        },
        "IMG_0001.JPG",
        "image/jpeg",
        &photo,
    )
    .await
    else {
        panic!("expected an image");
    };
    assert!(created);
    let project = format!("{}_Kovacs-es-Tarsa-Kft_ABC-123", order.number);
    assert!(
        image.storage_key.starts_with("projektek/")
            && image
                .storage_key
                .contains(&format!("/{project}/fotok/bevetel/IMG_0001_")),
        "{}",
        image.storage_key
    );
    assert_eq!(on_share(&dir, &image.storage_key), photo);
    // Evidence is a read-only file.
    assert!(
        std::fs::metadata(dir.join(&image.storage_key))
            .unwrap()
            .permissions()
            .readonly()
    );

    let drawing = b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n".to_vec();
    let Completed::Document { document, .. } = upload(
        &state,
        &user,
        Owner::Order(order.id),
        UploadTarget::Document {
            kind: DocumentKind::Cad,
            replaces: None,
        },
        "Alváz terv.dxf",
        "image/vnd.dxf",
        &drawing,
    )
    .await
    else {
        panic!("expected a document");
    };
    assert!(
        document
            .storage_key
            .contains(&format!("/{project}/dokumentumok/cad/Alvaz-terv_")),
        "{}",
        document.storage_key
    );

    // The signed download link serves the file, as an attachment unless it is safe inline.
    let link = state
        .storage
        .presign_get(&document.storage_key, Duration::from_secs(60), None)
        .await
        .unwrap();
    let (status, headers, body) = send(
        &state,
        Request::get(path_of(&link)).body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, drawing);
    assert_eq!(headers[header::CONTENT_DISPOSITION], "attachment");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");

    // Ranges, for videos that seek.
    let photo_link = state
        .storage
        .presign_get(&image.storage_key, Duration::from_secs(60), None)
        .await
        .unwrap();
    let (status, _, body) = send(
        &state,
        Request::get(path_of(&photo_link))
            .header(header::RANGE, "bytes=0-2")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, b"not");

    std::fs::remove_dir_all(dir).ok();
}

#[sqlx::test(migrations = "./migrations")]
async fn the_share_takes_only_the_file_the_ticket_was_for(pool: PgPool) {
    let (state, dir) = common::state_fs(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let bytes = b"the real photo".to_vec();
    let UploadResponse::Upload { upload, .. } = request_upload(
        &state,
        &user,
        Owner::Order(order.id),
        UploadRequest {
            target: UploadTarget::Image {
                category: ImageCategory::Production,
            },
            filename: Some("a.jpg".into()),
            content_type: "image/jpeg".into(),
            byte_size: bytes.len() as i64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        },
    )
    .await
    .unwrap() else {
        panic!("expected an upload ticket");
    };
    let put = |body: &[u8], uri: &str| {
        Request::builder()
            .method("PUT")
            .uri(uri)
            .body(Body::from(body.to_vec()))
            .unwrap()
    };
    let uri = path_of(&upload.url).to_string();
    // Same length, other bytes; then a forged token.
    let (status, _, _) = send(&state, put(b"the fake photo", &uri)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = send(&state, put(&bytes, &format!("{uri}x"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Nothing was filed, and nothing half-written is left behind.
    assert!(!dir.join("projektek").exists());
    assert_eq!(std::fs::read_dir(dir.join(".tmp")).unwrap().count(), 0);
    // A download link is not an upload ticket.
    let (status, _, _) = send(
        &state,
        put(&bytes, &uri.replace("/api/files/", "/api/files/x")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    std::fs::remove_dir_all(dir).ok();
}

#[sqlx::test(migrations = "./migrations")]
async fn re_sorting_files_what_was_stored_before(pool: PgPool) {
    let (state, dir) = common::state_fs(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    // As the old layout stored them: an intake photo, and an invoice and its storno.
    let photo = b"intake photo".to_vec();
    let photo_hash = Sha256::digest(&photo).to_vec();
    let old_photo = format!(
        "orders/{}/intake/{}.jpg",
        order.id,
        hex::encode(&photo_hash)
    );
    state
        .storage
        .put_bytes(&old_photo, photo.clone(), "image/jpeg")
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let (image, _) = images::insert(
        &mut tx,
        &NewImage {
            order_id: order.id,
            category: ImageCategory::Intake,
            storage_key: &old_photo,
            content_type: "image/jpeg",
            original_filename: Some("IMG_7.jpg"),
            content_hash: &photo_hash,
            byte_size: photo.len() as i64,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let mut stored = Vec::new();
    let mut original = None;
    let day = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
    for (number, kind, sign) in [
        ("AT2026-0001", InvoiceKind::Invoice, 1),
        ("AT2026-0002", InvoiceKind::Storno, -1),
    ] {
        let pdf = format!("%PDF {number}").into_bytes();
        let hash = Sha256::digest(&pdf).to_vec();
        let key = format!("orders/{}/documents/{}.pdf", order.id, hex::encode(&hash));
        state
            .storage
            .put_bytes(&key, pdf.clone(), "application/pdf")
            .await
            .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let (doc, _) = documents::insert(
            &mut conn,
            &NewDocument {
                owner: Owner::Order(order.id),
                vehicle_id: None,
                kind: DocumentKind::Invoice,
                filename: &format!("{number}.pdf"),
                content_type: "application/pdf",
                storage_key: &key,
                content_hash: &hash,
                byte_size: pdf.len() as i64,
                uploaded_by: None,
                source_ref: Some(number),
                previous_version_id: None,
                version: 1,
            },
        )
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let invoice = invoices::insert(
            &mut tx,
            &NewInvoice {
                order_id: order.id,
                number,
                kind,
                original_invoice_id: original,
                currency: "HUF",
                issue_date: day,
                delivery_date: day,
                payment_date: None,
                payment_method: "TRANSFER",
                net_amount: sign * 100_000,
                vat_amount: sign * 27_000,
                gross_amount: sign * 127_000,
                created_by: Some(user.user_id),
            },
        )
        .await
        .unwrap();
        if original.is_none() {
            sqlx::query("UPDATE invoices SET status = 'issued', issued_at = now() WHERE id = $1")
                .bind(invoice.id)
                .execute(&mut *tx)
                .await
                .unwrap();
            original = Some(invoice.id);
        }
        tx.commit().await.unwrap();
        let mut conn = pool.acquire().await.unwrap();
        invoices::set_document(&mut conn, invoice.id, doc.id)
            .await
            .unwrap();
        stored.push((doc.id, key));
    }

    // A link handed out before the move.
    let old_link = state
        .storage
        .presign_get(&old_photo, Duration::from_secs(60), None)
        .await
        .unwrap();

    let opts = Options {
        scope: Scope::All,
        source: None,
        dry_run: false,
    };
    let first = relocate::run(&state, &opts).await.unwrap();
    assert_eq!(
        (first.moved, first.failed, first.missing),
        (3, 0, 0),
        "{first:?}"
    );

    let image = images::find(&pool, image.id).await.unwrap().unwrap();
    assert!(
        image.storage_key.contains("/fotok/bevetel/IMG_7_"),
        "{}",
        image.storage_key
    );
    assert_eq!(on_share(&dir, &image.storage_key), photo);
    let invoice_doc = documents::find(&pool, stored[0].0).await.unwrap().unwrap();
    let storno_doc = documents::find(&pool, stored[1].0).await.unwrap().unwrap();
    assert!(
        invoice_doc
            .storage_key
            .contains("/szamlazas/szamlak/AT2026-0001_"),
        "{}",
        invoice_doc.storage_key
    );
    assert!(
        storno_doc
            .storage_key
            .contains("/szamlazas/sztornok/AT2026-0002_"),
        "{}",
        storno_doc.storage_key
    );
    // The old tree is gone entirely, folders included.
    assert!(!dir.join("orders").exists());

    // The link from before still finds the photo.
    let (status, _, body) = send(
        &state,
        Request::get(path_of(&old_link))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, photo);

    // Evidence moved, and the move is on record.
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'image_relocated' AND entity_id = $1",
    )
    .bind(order.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);

    let again = relocate::run(&state, &opts).await.unwrap();
    assert_eq!((again.moved, again.in_place), (0, 3), "{again:?}");
    std::fs::remove_dir_all(dir).ok();
}

#[sqlx::test(migrations = "./migrations")]
async fn an_enquirys_files_join_the_project_it_becomes(pool: PgPool) {
    let (state, dir) = common::state_fs(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let partner_id = common::partner(&pool, "Hűtő Bt", None).await;
    let lead = autocrm::service::leads::create(
        &pool,
        &user,
        autocrm::repo::leads::LeadInput {
            title: "Sprinter hűtés".into(),
            partner_id: Some(partner_id),
            contact_id: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            source: None,
            source_detail: None,
            description: None,
            assigned_to: None,
            quoted_value_minor: None,
            currency: None,
            quote_valid_until: None,
        },
        &[],
        NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
    )
    .await
    .unwrap();
    let quote = b"%PDF quotation".to_vec();
    let Completed::Document { document, .. } = upload(
        &state,
        &user,
        Owner::Lead(lead.id),
        UploadTarget::Document {
            kind: DocumentKind::Other,
            replaces: None,
        },
        "Árajánlat.pdf",
        "application/pdf",
        &quote,
    )
    .await
    else {
        panic!("expected a document");
    };
    assert!(
        document.storage_key.starts_with("ajanlatok/")
            && document
                .storage_key
                .contains(&format!("/L{:06}_Huto-Bt/Arajanlat_", lead.id)),
        "{}",
        document.storage_key
    );

    let order = autocrm::service::leads::convert(
        &pool,
        &user,
        lead.id,
        NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
        autocrm::service::leads::Conversion {
            partner_id: Some(partner_id),
            fields: common::fields(partner_id, "HUF"),
            items: vec![],
            answers: Some(
                    serde_json::json!({ "heats": false, "cools": false })
                        .as_object()
                        .cloned()
                        .unwrap(),
                ),
        },
    )
    .await
    .unwrap();
    relocate::tidy(&state, Scope::Lead(lead.id)).await.unwrap();

    let moved = documents::find(&pool, document.id).await.unwrap().unwrap();
    assert!(
        moved.storage_key.contains(&format!(
            "/{}_Huto-Bt_ABC-123/ajanlat/Arajanlat_",
            order.number
        )),
        "{}",
        moved.storage_key
    );
    assert_eq!(on_share(&dir, &moved.storage_key), quote);
    assert!(
        !dir.join("ajanlatok").exists(),
        "the empty enquiry folder is cleared away"
    );
    std::fs::remove_dir_all(dir).ok();
}
