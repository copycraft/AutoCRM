//! Upload flow: request ticket → client PUTs straight to object storage → finalize.
//! Photos never pass through the API server, which matters for 120-photo mobile batches.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::AppState;
use crate::domain::media::{
    MAX_DOCUMENT_BYTES, MAX_IMAGE_BYTES, document_extension, document_storage_key, image_extension,
    image_storage_key, lead_document_storage_key,
};
use crate::error::{AppError, AppResult};
use crate::media::storage::PresignedRequest;
use crate::media::upload_token::{self, UploadClaims, UploadTarget};
use crate::repo::documents::{self, Document, NewDocument, Owner};
use crate::repo::images::{self, Image, NewImage};
use crate::repo::{audit, jobs, leads, orders};
use crate::service::auth::AuthUser;

const TICKET_TTL: TimeDelta = TimeDelta::hours(2);

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UploadRequest {
    pub target: UploadTarget,
    pub filename: Option<String>,
    pub content_type: String,
    pub byte_size: i64,
    /// Hex sha256 of the file, computed by the client before upload.
    pub sha256: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum UploadResponse {
    /// Perform `upload` exactly as given (all headers are signed), then POST the ticket to
    /// /api/uploads/complete.
    Upload {
        ticket: String,
        upload: PresignedRequest,
        expires_at: DateTime<Utc>,
    },
    /// This exact file is already attached to the order. Nothing to upload.
    AlreadyUploaded {
        image_id: Option<i64>,
        document_id: Option<i64>,
    },
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Completed {
    Image { image: Image, created: bool },
    Document { document: Document, created: bool },
}

fn parse_sha256(hex_str: &str) -> AppResult<Vec<u8>> {
    let s = hex_str.trim().to_ascii_lowercase();
    if s.len() != 64 {
        return Err(AppError::validation("sha256 must be 64 hex characters"));
    }
    hex::decode(&s).map_err(|_| AppError::validation("sha256 must be hex"))
}

fn clean_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect::<String>()
        .trim()
        .to_string()
}

fn validate_content_type(ct: &str) -> AppResult<()> {
    let ok = !ct.is_empty()
        && ct.len() <= 100
        && ct.chars().all(|c| c.is_ascii_graphic() || c == ' ')
        && ct.contains('/');
    if ok {
        Ok(())
    } else {
        Err(AppError::validation("content_type is invalid"))
    }
}

pub async fn request_upload(
    state: &AppState,
    _user: &AuthUser,
    owner: Owner,
    req: UploadRequest,
) -> AppResult<UploadResponse> {
    match owner {
        Owner::Order(id) => {
            orders::find(&state.db, id)
                .await?
                .ok_or(AppError::NotFound("order"))?;
        }
        Owner::Lead(id) => {
            leads::find(&state.db, id)
                .await?
                .ok_or(AppError::NotFound("lead"))?;
        }
    }
    let hash = parse_sha256(&req.sha256)?;
    let hash_hex = hex::encode(&hash);
    validate_content_type(&req.content_type)?;
    if req.byte_size <= 0 {
        return Err(AppError::validation("byte_size must be positive"));
    }
    let filename = req
        .filename
        .as_deref()
        .map(clean_filename)
        .filter(|f| !f.is_empty());
    let now = Utc::now();

    let (key, lock_until) = match &req.target {
        UploadTarget::Image { category } => {
            // Evidence is tied to the job, not the enquiry: a lead has no MEO photos.
            let Owner::Order(order_id) = owner else {
                return Err(AppError::validation(
                    "images belong to an order, not a lead",
                ));
            };
            if req.byte_size > MAX_IMAGE_BYTES {
                return Err(AppError::validation("image is too large"));
            }
            let ext = image_extension(&req.content_type).ok_or_else(|| {
                AppError::validation("unsupported image type (jpeg, png, webp, heic)")
            })?;
            if let Some(existing) = images::find_by_hash(&state.db, order_id, &hash).await? {
                return Ok(UploadResponse::AlreadyUploaded {
                    image_id: Some(existing.id),
                    document_id: None,
                });
            }
            let lock = if category.is_immutable() {
                state.storage.intake_lock_until(now)
            } else {
                None
            };
            (image_storage_key(order_id, *category, &hash_hex, ext), lock)
        }
        UploadTarget::Document { .. } => {
            if req.byte_size > MAX_DOCUMENT_BYTES {
                return Err(AppError::validation("document is too large"));
            }
            let name = filename
                .as_deref()
                .ok_or_else(|| AppError::validation("filename is required for documents"))?;
            let ext = document_extension(name)
                .ok_or_else(|| AppError::validation("document needs a permitted file extension"))?;
            if let Some(existing) = documents::find_by_hash(&state.db, owner, &hash).await? {
                return Ok(UploadResponse::AlreadyUploaded {
                    image_id: None,
                    document_id: Some(existing.id),
                });
            }
            let key = match owner {
                Owner::Order(id) => document_storage_key(id, &hash_hex, &ext),
                Owner::Lead(id) => lead_document_storage_key(id, &hash_hex, &ext),
            };
            (key, None)
        }
    };

    let upload = state
        .storage
        .presign_put(
            &key,
            &req.content_type,
            req.byte_size,
            &STANDARD.encode(&hash),
            lock_until,
            Duration::from_secs(TICKET_TTL.num_seconds().unsigned_abs()),
        )
        .await?;
    let expires_at = now + TICKET_TTL;
    let (order_id, lead_id) = match owner {
        Owner::Order(id) => (Some(id), None),
        Owner::Lead(id) => (None, Some(id)),
    };
    let claims = UploadClaims {
        target: req.target,
        order_id,
        lead_id,
        user_id: _user.user_id,
        storage_key: key,
        sha256_hex: hash_hex,
        byte_size: req.byte_size,
        content_type: req.content_type,
        original_filename: filename,
        expires_at: expires_at.timestamp(),
    };
    let ticket = upload_token::sign(&state.config.upload_signing_key, &claims);
    Ok(UploadResponse::Upload {
        ticket,
        upload,
        expires_at,
    })
}

pub async fn complete_upload(
    state: &AppState,
    user: &AuthUser,
    ticket: &str,
) -> AppResult<Completed> {
    let claims = upload_token::verify(
        &state.config.upload_signing_key,
        ticket,
        Utc::now().timestamp(),
    )
    .map_err(|e| AppError::rule("invalid_ticket", e.to_string()))?;
    if claims.user_id != user.user_id {
        return Err(AppError::Forbidden);
    }
    let hash = hex::decode(&claims.sha256_hex)
        .map_err(|e| AppError::internal(format!("ticket hash: {e}")))?;

    let info = state
        .storage
        .head(&claims.storage_key)
        .await?
        .ok_or_else(|| AppError::rule("upload_missing", "the file has not been uploaded yet"))?;
    if info.size != claims.byte_size {
        return Err(AppError::rule(
            "upload_mismatch",
            "uploaded file size differs from the declared size",
        ));
    }
    match info.sha256_b64 {
        Some(stored) if stored == STANDARD.encode(&hash) => {}
        Some(_) => {
            return Err(AppError::rule(
                "upload_mismatch",
                "uploaded file hash differs from the declared hash",
            ));
        }
        // Store without checksum support: verify by reading the object back.
        None => {
            let bytes = state.storage.get_bytes(&claims.storage_key).await?;
            if Sha256::digest(&bytes).as_slice() != hash.as_slice() {
                return Err(AppError::rule(
                    "upload_mismatch",
                    "uploaded file hash differs from the declared hash",
                ));
            }
        }
    }

    let owner = match (claims.order_id, claims.lead_id) {
        (Some(id), None) => Owner::Order(id),
        (None, Some(id)) => Owner::Lead(id),
        _ => return Err(AppError::rule("invalid_ticket", "ticket names no owner")),
    };
    let mut tx = state.db.begin().await?;
    if let Owner::Order(order_id) = owner {
        orders::lock(&mut *tx, order_id)
            .await?
            .ok_or(AppError::NotFound("order"))?;
    }
    let completed = match claims.target {
        UploadTarget::Image { category } => {
            let Owner::Order(order_id) = owner else {
                return Err(AppError::rule(
                    "invalid_ticket",
                    "images belong to an order",
                ));
            };
            let (image, created) = images::insert(
                &mut tx,
                &NewImage {
                    order_id,
                    category,
                    storage_key: &claims.storage_key,
                    content_type: &claims.content_type,
                    original_filename: claims.original_filename.as_deref(),
                    content_hash: &hash,
                    byte_size: claims.byte_size,
                    uploaded_by: Some(user.user_id),
                    source_ref: None,
                },
            )
            .await?;
            if created {
                audit::record(
                    &mut *tx,
                    Some(user.user_id),
                    "order",
                    order_id,
                    "image_add",
                    json!({ "image_id": image.id, "category": category, "sha256": claims.sha256_hex }),
                )
                .await?;
                jobs::enqueue(
                    &mut *tx,
                    "process_image",
                    json!({ "image_id": image.id }),
                    None,
                    Some(&format!("process_image:{}", image.id)),
                )
                .await?;
            }
            Completed::Image { image, created }
        }
        UploadTarget::Document { kind } => {
            let filename = claims
                .original_filename
                .clone()
                .unwrap_or_else(|| "document".into());
            let (document, created) = documents::insert(
                &mut tx,
                &NewDocument {
                    owner,
                    vehicle_id: None,
                    kind,
                    filename: &filename,
                    content_type: &claims.content_type,
                    storage_key: &claims.storage_key,
                    content_hash: &hash,
                    byte_size: claims.byte_size,
                    uploaded_by: Some(user.user_id),
                    source_ref: None,
                },
            )
            .await?;
            if created {
                let (entity, entity_id) = match owner {
                    Owner::Order(id) => ("order", id),
                    Owner::Lead(id) => ("lead", id),
                };
                audit::record(
                    &mut *tx,
                    Some(user.user_id),
                    entity,
                    entity_id,
                    "document_add",
                    json!({ "document_id": document.id, "filename": filename, "sha256": claims.sha256_hex }),
                )
                .await?;
            }
            Completed::Document { document, created }
        }
    };
    tx.commit().await?;
    Ok(completed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_lose_paths_and_control_chars() {
        assert_eq!(clean_filename("C:\\Users\\x\\IMG_0001.JPG"), "IMG_0001.JPG");
        assert_eq!(clean_filename("../../etc/passwd"), "passwd");
        assert_eq!(clean_filename("a\r\nb.pdf"), "ab.pdf");
    }

    #[test]
    fn sha256_parsing() {
        assert!(parse_sha256(&"AB".repeat(32)).is_ok());
        assert!(parse_sha256("abc").is_err());
        assert!(parse_sha256(&"zz".repeat(32)).is_err());
    }
}
