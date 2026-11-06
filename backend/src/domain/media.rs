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
    /// Handover-inspection walkaround photos (átadás-átvétel). Mutable like
    /// production: drafts are retaken freely, and the lock lives on the
    /// inspection status rather than on the image row.
    Inspection,
}

impl ImageCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            ImageCategory::Intake => "intake",
            ImageCategory::Production => "production",
            ImageCategory::Completion => "completion",
            ImageCategory::Marketing => "marketing",
            ImageCategory::Inspection => "inspection",
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
    /// V2.5: an ATP certificate belongs to the vehicle for its life and expires. Kept
    /// distinct from Other so `GET /documents?kind=certificate&expiring_before=…` can be
    /// asked at all.
    Certificate,
    /// A reported invoice or storno, rendered from what NAV holds. Generated, never
    /// uploaded, and listed apart from the design files on the order screen.
    Invoice,
    /// A díjbekérő. Looks like an invoice, is not one, and is reported nowhere — which is
    /// exactly why it has a kind of its own rather than hiding among the invoices.
    Proforma,
    Other,
    /// A short clip recorded during a handover walkaround (0047). Never an email attachment.
    Video,
    /// The quotation the sales manager made outside the CRM and sent from their own
    /// mailbox (0057). Filed on the lead; recording it starts the follow-ups.
    Quote,
}

/// What kind of thumbnail a document gets, from its file name. `None`: no thumbnail (PDFs
/// preview in the browser instead).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbSource {
    Dxf,
    Dwg,
    Image,
}

pub fn thumb_source(filename: &str) -> Option<ThumbSource> {
    let ext = filename.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "dxf" => Some(ThumbSource::Dxf),
        "dwg" => Some(ThumbSource::Dwg),
        "jpg" | "jpeg" | "png" | "webp" => Some(ThumbSource::Image),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawings_and_pictures_get_thumbnails_other_files_do_not() {
        assert_eq!(thumb_source("alváz.DXF"), Some(ThumbSource::Dxf));
        assert_eq!(thumb_source("terv.v2.dwg"), Some(ThumbSource::Dwg));
        assert_eq!(thumb_source("fotó.jpeg"), Some(ThumbSource::Image));
        assert_eq!(thumb_source("ajánlat.pdf"), None);
        assert_eq!(thumb_source("README"), None);
    }

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
}
