//! Image and document classification rules.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "image_category", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ImageCategory {
    Intake,
    Production,
    Completion,
    Marketing,
}

impl ImageCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageCategory::Intake => "intake",
            ImageCategory::Production => "production",
            ImageCategory::Completion => "completion",
            ImageCategory::Marketing => "marketing",
        }
    }

    /// Intake photos document pre-existing vehicle damage. They are evidence: write-once,
    /// never deleted or replaced, and locked in object storage.
    pub fn is_immutable(self) -> bool {
        matches!(self, ImageCategory::Intake)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "document_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DocumentKind {
    Design,
    Cad,
    Other,
}

pub const MAX_IMAGE_BYTES: i64 = 50 * 1024 * 1024;
pub const MAX_DOCUMENT_BYTES: i64 = 2 * 1024 * 1024 * 1024;

/// Image formats we accept for upload, with the extension used in storage keys.
/// HEIC is accepted and stored (it is the original), but derived copies cannot be
/// generated for it yet; the image row records a processing error instead.
pub fn image_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/heic" | "image/heif" => Some("heic"),
        _ => None,
    }
}

/// Documents: anything that is not executable-looking. Extension comes from the filename.
pub fn document_extension(filename: &str) -> Option<String> {
    let ext = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    let ok = !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric());
    const BLOCKED: [&str; 8] = ["exe", "bat", "cmd", "com", "msi", "ps1", "scr", "js"];
    (ok && !BLOCKED.contains(&ext.as_str())).then_some(ext)
}

/// Content-addressed object key for an original image.
pub fn image_storage_key(
    order_id: i64,
    category: ImageCategory,
    hash_hex: &str,
    ext: &str,
) -> String {
    format!("orders/{order_id}/{}/{hash_hex}.{ext}", category.as_str())
}

pub fn image_derived_key(
    order_id: i64,
    category: ImageCategory,
    hash_hex: &str,
    variant: &str,
) -> String {
    format!(
        "orders/{order_id}/{}/derived/{hash_hex}.{variant}.jpg",
        category.as_str()
    )
}

pub fn document_storage_key(order_id: i64, hash_hex: &str, ext: &str) -> String {
    format!("orders/{order_id}/documents/{hash_hex}.{ext}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_intake_is_immutable() {
        assert!(ImageCategory::Intake.is_immutable());
        assert!(!ImageCategory::Completion.is_immutable());
    }

    #[test]
    fn document_extensions() {
        assert_eq!(document_extension("terv.PDF").as_deref(), Some("pdf"));
        assert_eq!(document_extension("model.step").as_deref(), Some("step"));
        assert_eq!(document_extension("virus.exe"), None);
        assert_eq!(document_extension("noext"), None);
        assert_eq!(document_extension("weird.p d f"), None);
    }

    #[test]
    fn keys_are_content_addressed() {
        assert_eq!(
            image_storage_key(42, ImageCategory::Intake, "abcd", "jpg"),
            "orders/42/intake/abcd.jpg"
        );
    }
}
