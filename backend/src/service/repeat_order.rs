//! A repeat order: a new project started as a copy of an earlier one.
//!
//! The same customer ordering the same body again should not mean typing it all again. The
//! copy takes the data row (customer, contact, title, project type, the vehicle's make and
//! model), the line items at their old prices, the build spec, each department's technical
//! content, and the drawings and photos that content uses. It does not take what belongs
//! to the earlier vehicle or job: plate, VIN, the cooling unit's serial, intake and
//! handover records, stages, invoices, comments, blockers, deadlines. Everything copied
//! stays editable on the new project, which records where it came from (`repeat`).
//!
//! Files are copied byte for byte into the new project's own folders, after the project is
//! committed: the project never waits on storage, and a file that fails to copy is named in
//! the answer rather than undoing the whole copy.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::domain::media::{DocumentKind, document_extension, image_extension, thumb_source};
use crate::error::{AppError, AppResult};
use crate::media::layout;
use crate::repo::documents::{self, NewDocument, Owner};
use crate::repo::images::{self, NewImage};
use crate::repo::orders::{Order, OrderFields};
use crate::repo::{audit, jobs, order_items, order_modules, orders, questionnaire};
use crate::service::auth::AuthUser;
use crate::service::business_today;
use crate::service::orders::{NewItem, create_in_tx};

fn yes() -> bool {
    true
}

/// What to take from the earlier project. Everything is taken unless switched off.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct CopyRequest {
    /// The new project's title; the earlier one's when omitted.
    pub title: Option<String>,
    #[serde(default = "yes")]
    pub items: bool,
    #[serde(default = "yes")]
    pub spec: bool,
    /// Each department's technical content, as drafts to be released again.
    #[serde(default = "yes")]
    pub modules: bool,
    /// Drawings, CAD files and other documents, and the photos the modules use.
    #[serde(default = "yes")]
    pub files: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CopyResult {
    pub order: Order,
    pub copied_documents: usize,
    pub copied_images: usize,
    /// Files that could not be copied, by name. The project exists either way.
    pub failed_files: Vec<String>,
}

pub async fn copy_order(
    state: &AppState,
    user: &AuthUser,
    source_id: i64,
    req: &CopyRequest,
) -> AppResult<CopyResult> {
    let today = business_today(state.config.business_tz);
    let mut tx = state.db.begin().await?;
    let source = orders::find(&mut *tx, source_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;

    let mut fields = OrderFields::from(&source);
    if let Some(title) = req
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        fields.title = title.to_string();
    }
    // The same kind of vehicle, not the same vehicle.
    fields.vehicle_plate = None;
    fields.vehicle_vin = None;
    fields.valuation_date = today;
    fields.due_date = None;
    fields.related_order_id = Some(source.id);
    fields.relation = Some("repeat".into());
    fields.mileage_in = None;
    fields.intake_condition = None;
    fields.fuel_level = None;
    fields.key_count = None;
    fields.valuables_declared = None;
    fields.valuables = None;

    let items = if req.items {
        order_items::list(&mut *tx, source.id)
            .await?
            .into_iter()
            .map(|i| NewItem {
                description: i.description,
                quantity: i.quantity,
                unit_price: i.unit_price,
            })
            .collect()
    } else {
        Vec::new()
    };
    let order = create_in_tx(&mut tx, user.user_id, today, fields, items, None).await?;

    if req.spec {
        // The questionnaire's answers, as they were. Not the serial: the new body gets its
        // own cooling unit.
        let mut answers = questionnaire::order_answers(&mut *tx, source.id).await?;
        answers.remove("cooling_unit_serial");
        if !answers.is_empty() {
            questionnaire::set_order_answers(&mut *tx, order.id, &answers).await?;
        }
    }

    // Old module id → new module id, for the file links below.
    let mut module_map: HashMap<i64, i64> = HashMap::new();
    if req.modules {
        for m in order_modules::list_for_order(&mut *tx, source.id).await? {
            let id = order_modules::insert(
                &mut *tx,
                order.id,
                m.department_id,
                &m.content,
                user.user_id,
            )
            .await?;
            module_map.insert(m.id, id);
        }
    }
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        order.id,
        "copied_from",
        json!({
            "source_order_id": source.id,
            "source_number": source.number,
            "items": req.items,
            "spec": req.spec,
            "modules": module_map.len(),
        }),
    )
    .await?;
    tx.commit().await?;

    let mut result = CopyResult {
        order,
        copied_documents: 0,
        copied_images: 0,
        failed_files: Vec::new(),
    };
    if req.files {
        copy_files(state, user, source.id, &module_map, &mut result).await?;
    }
    Ok(result)
}

/// Copies the earlier project's documents (drawings, CAD, other) and the photos its modules
/// use into the new project, and links them to the new modules as they were linked before.
async fn copy_files(
    state: &AppState,
    user: &AuthUser,
    source_id: i64,
    module_map: &HashMap<i64, i64>,
    result: &mut CopyResult,
) -> AppResult<()> {
    let target = result.order.id;
    let links = order_modules::files_for_order(&state.db, source_id).await?;

    let mut doc_map: HashMap<i64, i64> = HashMap::new();
    for d in documents::list_for_order(&state.db, source_id).await? {
        if !matches!(
            d.kind,
            DocumentKind::Design | DocumentKind::Cad | DocumentKind::Other
        ) {
            continue;
        }
        match copy_document(state, user, target, &d).await {
            Ok(id) => {
                doc_map.insert(d.id, id);
                result.copied_documents += 1;
            }
            Err(e) => {
                tracing::warn!(document_id = d.id, error = %e, "could not copy a document");
                result.failed_files.push(d.filename.clone());
            }
        }
    }

    let mut image_map: HashMap<i64, i64> = HashMap::new();
    for image_id in links.iter().filter_map(|l| l.image_id) {
        if image_map.contains_key(&image_id) {
            continue;
        }
        let Some(image) = images::find(&state.db, image_id).await? else {
            continue;
        };
        match copy_image(state, user, target, &image).await {
            Ok(id) => {
                image_map.insert(image_id, id);
                result.copied_images += 1;
            }
            Err(e) => {
                tracing::warn!(image_id, error = %e, "could not copy a photo");
                result.failed_files.push(
                    image
                        .original_filename
                        .clone()
                        .unwrap_or_else(|| format!("fotó #{image_id}")),
                );
            }
        }
    }

    for link in &links {
        let Some(&module) = module_map.get(&link.module_id) else {
            continue;
        };
        let image = link.image_id.and_then(|id| image_map.get(&id).copied());
        let document = link.document_id.and_then(|id| doc_map.get(&id).copied());
        if image.is_some() || document.is_some() {
            order_modules::add_file(&state.db, module, image, document).await?;
        }
    }
    Ok(())
}

async fn copy_document(
    state: &AppState,
    user: &AuthUser,
    order_id: i64,
    d: &documents::Document,
) -> anyhow::Result<i64> {
    let bytes = state.storage.get_bytes(&d.storage_key).await?;
    let hash = Sha256::digest(&bytes);
    let hash_hex = hex::encode(hash);
    let ext = document_extension(&d.filename).unwrap_or_else(|| "bin".into());
    let owner = Owner::Order(order_id);
    let key = layout::document_key(state, owner, d.kind, &d.filename, 1, &hash_hex, &ext).await?;
    let byte_size = i64::try_from(bytes.len())?;
    state
        .storage
        .put_bytes(&key, bytes, &d.content_type)
        .await?;
    let mut tx = state.db.begin().await?;
    let (document, created) = documents::insert(
        &mut tx,
        &NewDocument {
            owner,
            vehicle_id: None,
            kind: d.kind,
            filename: &d.filename,
            content_type: &d.content_type,
            storage_key: &key,
            content_hash: hash.as_slice(),
            byte_size,
            uploaded_by: Some(user.user_id),
            source_ref: None,
            previous_version_id: None,
            version: 1,
        },
    )
    .await?;
    if created && thumb_source(&d.filename).is_some() {
        jobs::enqueue(
            &mut *tx,
            "process_document",
            json!({ "document_id": document.id }),
            None,
            Some(&format!("process_document:{}", document.id)),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(document.id)
}

async fn copy_image(
    state: &AppState,
    user: &AuthUser,
    order_id: i64,
    image: &images::Image,
) -> anyhow::Result<i64> {
    let bytes = state.storage.get_bytes(&image.storage_key).await?;
    let hash = Sha256::digest(&bytes);
    let hash_hex = hex::encode(hash);
    let ext = image_extension(&image.content_type).unwrap_or("jpg");
    let key = layout::image_key(
        state,
        order_id,
        image.category,
        image.original_filename.as_deref(),
        &hash_hex,
        ext,
    )
    .await?;
    let byte_size = i64::try_from(bytes.len())?;
    state
        .storage
        .put_bytes(&key, bytes, &image.content_type)
        .await?;
    let mut tx = state.db.begin().await?;
    let (copy, created) = images::insert(
        &mut tx,
        &NewImage {
            order_id,
            category: image.category,
            storage_key: &key,
            content_type: &image.content_type,
            original_filename: image.original_filename.as_deref(),
            content_hash: hash.as_slice(),
            byte_size,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await?;
    if created {
        jobs::enqueue(
            &mut *tx,
            "process_image",
            json!({ "image_id": copy.id }),
            None,
            Some(&format!("process_image:{}", copy.id)),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(copy.id)
}
