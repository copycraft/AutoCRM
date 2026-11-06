//! Where each file is filed: one folder per project (an order), per enquiry (a lead), per
//! employee and per job posting, named for a person browsing the share.
//!
//! ```text
//! projektek/2026/2026-0042_Kovacs-Kft_ABC-123/
//!     ajanlat/                         the quotation and other files of the enquiry
//!     dokumentumok/{terv,cad,tanusitvany,video,egyeb}/
//!     szamlazas/{szamlak,sztornok,dijbekerok}/
//!     fotok/{bevetel,gyartas,atadas-meo,atadas-atvetel,marketing}/
//!     atadas-atvetel/2026-10-19_atvetel_ABC-123_17/{fotok,alairasok,videok}/
//!     torolt/…                         deleted in the app (the app keeps them too)
//!     .elonezet/                       previews and thumbnails, regenerated at will
//! ajanlatok/2026/L000123_Kovacs-Kft/   an enquiry that has not become an order
//! hr/dolgozok/0012_Nagy-Peter/{profilkep,dokumentumok/<fajta>,tavollet}/
//! hr/toborzas/0003_Villanyszerelo/
//! bejovo-szamlak/2026/10/
//! migracio/eredeti/                    MiniCRM downloads, until re-sorting files them
//! ```
//!
//! Names are ASCII (accents folded, anything else a dash), so they survive NFS, SMB, ZIP
//! files and scripts alike. A folder is named once and the name kept in `storage_folders`:
//! renaming the customer later never splits a project in two. A file name ends in the first
//! characters of its sha256, so two different "IMG_0001.jpg" never collide and the same
//! upload twice lands on the same name.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sqlx::PgPool;

use crate::AppState;
use crate::domain::media::{DocumentKind, ImageCategory};
use crate::repo::documents::Owner;

pub const PROJECTS: &str = "projektek";
pub const ENQUIRIES: &str = "ajanlatok";
pub const EMPLOYEES: &str = "hr/dolgozok";
pub const POSTINGS: &str = "hr/toborzas";
pub const INCOMING_INVOICES: &str = "bejovo-szamlak";
/// MiniCRM files are fetched before anyone knows which project they belong to.
pub const MIGRATION: &str = "migracio/eredeti";
pub const PREVIEWS: &str = ".elonezet";
pub const DELETED: &str = "torolt";
/// The enquiry's files, inside the project it became.
pub const QUOTE: &str = "ajanlat";

// ── Names ─────────────────────────────────────────────────────────────────────

/// A folder-name part: letters and digits, words joined by `-`, at most `max` characters.
pub fn slug(s: &str, max: usize) -> String {
    clean(s, max, false)
}

/// A file-name stem: like [`slug`], but `_` survives ("IMG_0001").
pub fn stem_slug(s: &str, max: usize) -> String {
    clean(s, max, true)
}

fn clean(s: &str, max: usize, keep_underscore: bool) -> String {
    let mut out = String::with_capacity(s.len().min(max));
    let mut gap = false;
    for c in s.chars() {
        let folded = fold(c);
        for m in folded.chars() {
            if m.is_ascii_alphanumeric() || (keep_underscore && m == '_') {
                if gap && !out.is_empty() {
                    out.push('-');
                }
                out.push(m);
                gap = false;
            } else {
                gap = true;
            }
        }
    }
    out.truncate(max);
    out.trim_end_matches(['-', '_']).to_string()
}

/// Accented Latin letters to their plain form; anything else unknown becomes a gap.
fn fold(c: char) -> String {
    if c.is_ascii() {
        return c.to_string();
    }
    let lower = c.to_lowercase().next().unwrap_or(c);
    let base = match lower {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ą' | 'ă' => "a",
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'í' | 'ì' | 'î' | 'ï' | 'ī' => "i",
        'ó' | 'ò' | 'ô' | 'ö' | 'ő' | 'õ' | 'ø' | 'ō' => "o",
        'ú' | 'ù' | 'û' | 'ü' | 'ű' | 'ū' | 'ů' => "u",
        'ý' | 'ÿ' => "y",
        'ç' | 'č' | 'ć' => "c",
        'š' | 'ś' | 'ş' | 'ș' => "s",
        'ž' | 'ź' | 'ż' => "z",
        'ñ' | 'ń' | 'ň' => "n",
        'ř' => "r",
        'ď' => "d",
        'ť' | 'ț' => "t",
        'ł' | 'ľ' | 'ĺ' => "l",
        'ğ' => "g",
        'ß' => "ss",
        'æ' => "ae",
        'œ' => "oe",
        _ => return " ".into(),
    };
    if c.is_uppercase() && lower != 'ß' {
        base.to_uppercase()
    } else {
        base.to_string()
    }
}

fn join_parts(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("_")
}

/// `projektek/2026/2026-0042_Kovacs-Kft_ABC-123`.
pub fn order_folder(year: i32, number: &str, partner: &str, plate: Option<&str>) -> String {
    let mut name = join_parts(&[
        slug(number, 30),
        slug(partner, 40),
        plate.map(|p| slug(p, 16)).unwrap_or_default(),
    ]);
    if name.is_empty() {
        name = "projekt".into();
    }
    format!("{PROJECTS}/{year}/{name}")
}

/// `ajanlatok/2026/L000123_Kovacs-Kft`.
pub fn lead_folder(year: i32, lead_id: i64, name: &str) -> String {
    let name = join_parts(&[format!("L{lead_id:06}"), slug(name, 40)]);
    format!("{ENQUIRIES}/{year}/{name}")
}

/// `hr/dolgozok/0012_Nagy-Peter`.
pub fn employee_folder(employee_id: i64, name: &str) -> String {
    let name = join_parts(&[format!("{employee_id:04}"), slug(name, 40)]);
    format!("{EMPLOYEES}/{name}")
}

/// `hr/toborzas/0003_Villanyszerelo`.
pub fn posting_folder(posting_id: i64, title: &str) -> String {
    let name = join_parts(&[format!("{posting_id:04}"), slug(title, 40)]);
    format!("{POSTINGS}/{name}")
}

pub fn category_dir(category: ImageCategory) -> &'static str {
    match category {
        ImageCategory::Intake => "bevetel",
        ImageCategory::Production => "gyartas",
        ImageCategory::Completion => "atadas-meo",
        ImageCategory::Marketing => "marketing",
        ImageCategory::Inspection => "atadas-atvetel",
    }
}

/// What a document is, as far as its folder goes. Invoices, stornos and proformas are
/// all PDFs NAV's sidecar rendered, but the bookkeeper looks for them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    Kind(DocumentKind),
    Storno,
}

impl From<DocumentKind> for Shelf {
    fn from(kind: DocumentKind) -> Self {
        Shelf::Kind(kind)
    }
}

/// A document's folder inside its project.
pub fn shelf_dir(shelf: Shelf) -> &'static str {
    match shelf {
        Shelf::Kind(DocumentKind::Design) => "dokumentumok/terv",
        Shelf::Kind(DocumentKind::Cad) => "dokumentumok/cad",
        Shelf::Kind(DocumentKind::Certificate) => "dokumentumok/tanusitvany",
        Shelf::Kind(DocumentKind::Video) => "dokumentumok/video",
        Shelf::Kind(DocumentKind::Other) => "dokumentumok/egyeb",
        Shelf::Kind(DocumentKind::Invoice) => "szamlazas/szamlak",
        Shelf::Storno => "szamlazas/sztornok",
        Shelf::Kind(DocumentKind::Proforma) => "szamlazas/dijbekerok",
        Shelf::Kind(DocumentKind::Quote) => QUOTE,
    }
}

/// One walkaround's folder: `atadas-atvetel/2026-10-19_atvetel_ABC-123_17`. Checkout is
/// "Átvétel" in the app, checkin "Kiadás".
pub fn inspection_dir(project: &str, on: NaiveDate, kind: &str, plate: &str, id: i64) -> String {
    let kind = match kind {
        "checkout" => "atvetel",
        "checkin" => "kiadas",
        _ => "szemle",
    };
    let name = join_parts(&[on.to_string(), kind.into(), slug(plate, 16), id.to_string()]);
    format!("{project}/atadas-atvetel/{name}")
}

/// What a file is within a walkaround folder.
pub fn inspection_part(dir: &str, part: InspectionPart) -> String {
    let sub = match part {
        InspectionPart::Photo => "fotok",
        InspectionPart::Signature => "alairasok",
        InspectionPart::Video => "videok",
    };
    format!("{dir}/{sub}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionPart {
    Photo,
    Signature,
    Video,
}

/// HR paper kinds (`employee_documents.kind`).
pub fn employee_document_dir_name(kind: &str) -> &'static str {
    match kind {
        "medical" => "orvosi",
        "contract" => "szerzodes",
        "licence" => "engedely",
        "training" => "kepzes",
        _ => "egyeb",
    }
}

/// `rel` inside `folder`, or inside its `torolt/` when the record is deleted in the app.
fn within(folder: &str, rel: &str, deleted: bool) -> String {
    match (deleted, rel.is_empty()) {
        (false, true) => folder.to_string(),
        (false, false) => format!("{folder}/{rel}"),
        (true, true) => format!("{folder}/{DELETED}"),
        (true, false) => format!("{folder}/{DELETED}/{rel}"),
    }
}

pub fn photo_dir(project: &str, category: ImageCategory, deleted: bool) -> String {
    within(
        project,
        &format!("fotok/{}", category_dir(category)),
        deleted,
    )
}

pub fn document_dir(project: &str, shelf: impl Into<Shelf>, deleted: bool) -> String {
    within(project, shelf_dir(shelf.into()), deleted)
}

pub fn previews_dir(folder: &str) -> String {
    format!("{folder}/{PREVIEWS}")
}

pub fn employee_photo_dir(folder: &str) -> String {
    format!("{folder}/profilkep")
}

pub fn employee_document_dir(folder: &str, kind: &str, deleted: bool) -> String {
    within(
        folder,
        &format!("dokumentumok/{}", employee_document_dir_name(kind)),
        deleted,
    )
}

pub fn absence_dir(folder: &str) -> String {
    format!("{folder}/tavollet")
}

/// `bejovo-szamlak/2026/10`, by the day it was filed.
pub fn incoming_invoice_dir(filed_on: NaiveDate, deleted: bool) -> String {
    let month = format!("{:04}/{:02}", filed_on.year(), filed_on.month());
    if deleted {
        format!("{INCOMING_INVOICES}/{DELETED}/{month}")
    } else {
        format!("{INCOMING_INVOICES}/{month}")
    }
}

/// The first eight characters of a hex hash: enough to tell files apart in one folder.
pub fn hash_tag(hash_hex: &str) -> &str {
    &hash_hex[..hash_hex.len().min(8)]
}

/// A tag for files with no hash to hand (HR papers), random like their keys always were.
pub fn random_tag() -> String {
    hex::encode(rand::random::<[u8; 4]>())
}

/// `<stem>_<tag>.<ext>`; the stem from the original file name, or `fallback`.
pub fn file_name(original: Option<&str>, fallback: &str, tag: &str, ext: &str) -> String {
    let stem = original
        .map(|f| f.rsplit(['/', '\\']).next().unwrap_or(f))
        .map(|f| f.rsplit_once('.').map_or(f, |(stem, _)| stem))
        .map(|s| stem_slug(s, 60))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string());
    if ext.is_empty() {
        format!("{stem}_{tag}")
    } else {
        format!("{stem}_{tag}.{ext}")
    }
}

pub fn photo_name(original: Option<&str>, hash_hex: &str, ext: &str) -> String {
    file_name(original, "foto", hash_tag(hash_hex), ext)
}

/// A later version says so: `terv_v2_1a2b3c4d.pdf`.
pub fn document_name(filename: &str, version: i32, hash_hex: &str, ext: &str) -> String {
    let tag = if version > 1 {
        format!("v{version}_{}", hash_tag(hash_hex))
    } else {
        hash_tag(hash_hex).to_string()
    };
    file_name(Some(filename), "dokumentum", &tag, ext)
}

/// `.elonezet/<hash16>.<variant>.<ext>`.
pub fn derived_name(hash_hex: &str, variant: &str, ext: &str) -> String {
    format!("{}.{variant}.{ext}", &hash_hex[..hash_hex.len().min(16)])
}

/// The extension a stored key ends in, lower case; `bin` when there is none.
pub fn ext_of(key: &str) -> String {
    let base = key.rsplit('/').next().unwrap_or(key);
    match base.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && !ext.is_empty()
                && ext.len() <= 10
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            ext.to_ascii_lowercase()
        }
        _ => "bin".into(),
    }
}

/// The folder a key is in.
pub fn parent(key: &str) -> &str {
    key.rsplit_once('/').map_or("", |(dir, _)| dir)
}

pub fn join(dir: &str, name: &str) -> String {
    format!("{dir}/{name}")
}

// ── Folders, named once ──────────────────────────────────────────────────────

async fn existing(db: &PgPool, entity: &str, id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT folder FROM storage_folders WHERE entity = $1 AND entity_id = $2")
        .bind(entity)
        .bind(id)
        .fetch_optional(db)
        .await
}

/// The folder already named for this record, or `wanted` (with the id appended when another
/// record took that name first).
async fn assign(db: &PgPool, entity: &str, id: i64, wanted: String) -> sqlx::Result<String> {
    if let Some(folder) = existing(db, entity, id).await? {
        return Ok(folder);
    }
    let fallback = format!("{wanted}_{id}");
    for candidate in [wanted, fallback] {
        let inserted = sqlx::query_scalar::<_, String>(
            "INSERT INTO storage_folders (entity, entity_id, folder) VALUES ($1, $2, $3)
             ON CONFLICT (entity, entity_id) DO NOTHING RETURNING folder",
        )
        .bind(entity)
        .bind(id)
        .bind(&candidate)
        .fetch_optional(db)
        .await;
        match inserted {
            Ok(Some(folder)) => return Ok(folder),
            // Named by a request running at the same moment.
            Ok(None) => {
                if let Some(folder) = existing(db, entity, id).await? {
                    return Ok(folder);
                }
            }
            Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {}
            Err(e) => return Err(e),
        }
    }
    Err(sqlx::Error::Protocol(format!(
        "no free storage folder name for {entity} {id}"
    )))
}

fn local_year(state: &AppState, at: DateTime<Utc>) -> i32 {
    at.with_timezone(&state.config.business_tz).year()
}

/// The project folder of an order.
pub async fn order_folder_of(state: &AppState, order_id: i64) -> sqlx::Result<String> {
    if let Some(folder) = existing(&state.db, "order", order_id).await? {
        return Ok(folder);
    }
    let (number, created_at, partner, plate): (String, DateTime<Utc>, String, Option<String>) =
        sqlx::query_as(
            "SELECT o.number, o.created_at, p.name, o.vehicle_plate
               FROM orders o JOIN partners p ON p.id = o.partner_id
              WHERE o.id = $1",
        )
        .bind(order_id)
        .fetch_one(&state.db)
        .await?;
    let wanted = order_folder(
        local_year(state, created_at),
        &number,
        &partner,
        plate.as_deref(),
    );
    assign(&state.db, "order", order_id, wanted).await
}

/// An enquiry's own folder (before, or without, an order).
pub async fn lead_folder_of(state: &AppState, lead_id: i64) -> sqlx::Result<String> {
    if let Some(folder) = existing(&state.db, "lead", lead_id).await? {
        return Ok(folder);
    }
    let (created_at, name): (DateTime<Utc>, String) = sqlx::query_as(
        "SELECT l.created_at, coalesce(p.name, nullif(btrim(l.contact_name), ''), l.title)
           FROM leads l LEFT JOIN partners p ON p.id = l.partner_id
          WHERE l.id = $1",
    )
    .bind(lead_id)
    .fetch_one(&state.db)
    .await?;
    let wanted = lead_folder(local_year(state, created_at), lead_id, &name);
    assign(&state.db, "lead", lead_id, wanted).await
}

pub async fn employee_folder_of(state: &AppState, employee_id: i64) -> sqlx::Result<String> {
    if let Some(folder) = existing(&state.db, "employee", employee_id).await? {
        return Ok(folder);
    }
    let name: String = sqlx::query_scalar("SELECT full_name FROM employees WHERE id = $1")
        .bind(employee_id)
        .fetch_one(&state.db)
        .await?;
    assign(
        &state.db,
        "employee",
        employee_id,
        employee_folder(employee_id, &name),
    )
    .await
}

pub async fn posting_folder_of(state: &AppState, posting_id: i64) -> sqlx::Result<String> {
    if let Some(folder) = existing(&state.db, "job_posting", posting_id).await? {
        return Ok(folder);
    }
    let title: String = sqlx::query_scalar("SELECT title FROM job_postings WHERE id = $1")
        .bind(posting_id)
        .fetch_one(&state.db)
        .await?;
    assign(
        &state.db,
        "job_posting",
        posting_id,
        posting_folder(posting_id, &title),
    )
    .await
}

/// The order an enquiry became; the first, when one enquiry became several (V2.7).
pub async fn order_of_lead(db: &PgPool, lead_id: i64) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT id FROM orders WHERE lead_id = $1 ORDER BY id LIMIT 1")
        .bind(lead_id)
        .fetch_optional(db)
        .await
}

/// The folder an owner's previews go in: the project's, or the enquiry's while it has none.
pub async fn owner_folder(state: &AppState, owner: Owner) -> sqlx::Result<String> {
    match owner {
        Owner::Order(id) => order_folder_of(state, id).await,
        Owner::Lead(id) => match order_of_lead(&state.db, id).await? {
            Some(order_id) => order_folder_of(state, order_id).await,
            None => lead_folder_of(state, id).await,
        },
    }
}

// ── Keys ──────────────────────────────────────────────────────────────────────

pub async fn image_dir(
    state: &AppState,
    order_id: i64,
    category: ImageCategory,
    deleted: bool,
) -> sqlx::Result<String> {
    Ok(photo_dir(
        &order_folder_of(state, order_id).await?,
        category,
        deleted,
    ))
}

/// Where an owner's document of this kind goes. An enquiry's files are filed as a whole in
/// `ajanlat/` of the project it became, or in its own folder before that.
pub async fn document_dir_of(
    state: &AppState,
    owner: Owner,
    shelf: Shelf,
    deleted: bool,
) -> sqlx::Result<String> {
    match owner {
        Owner::Order(id) => Ok(document_dir(
            &order_folder_of(state, id).await?,
            shelf,
            deleted,
        )),
        Owner::Lead(id) => match order_of_lead(&state.db, id).await? {
            Some(order_id) => Ok(within(
                &order_folder_of(state, order_id).await?,
                QUOTE,
                deleted,
            )),
            None => Ok(within(&lead_folder_of(state, id).await?, "", deleted)),
        },
    }
}

pub async fn image_key(
    state: &AppState,
    order_id: i64,
    category: ImageCategory,
    original_filename: Option<&str>,
    hash_hex: &str,
    ext: &str,
) -> sqlx::Result<String> {
    Ok(join(
        &image_dir(state, order_id, category, false).await?,
        &photo_name(original_filename, hash_hex, ext),
    ))
}

/// A display copy or thumbnail of a photo: `.elonezet/` of its project.
pub async fn image_derived_key(
    state: &AppState,
    order_id: i64,
    hash_hex: &str,
    variant: &str,
) -> sqlx::Result<String> {
    Ok(join(
        &previews_dir(&order_folder_of(state, order_id).await?),
        &derived_name(hash_hex, variant, "jpg"),
    ))
}

pub async fn document_key(
    state: &AppState,
    owner: Owner,
    shelf: impl Into<Shelf>,
    filename: &str,
    version: i32,
    hash_hex: &str,
    ext: &str,
) -> sqlx::Result<String> {
    Ok(join(
        &document_dir_of(state, owner, shelf.into(), false).await?,
        &document_name(filename, version, hash_hex, ext),
    ))
}

pub async fn document_thumb_key(
    state: &AppState,
    owner: Owner,
    hash_hex: &str,
) -> sqlx::Result<String> {
    Ok(join(
        &previews_dir(&owner_folder(state, owner).await?),
        &derived_name(hash_hex, "thumb", "png"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain_ascii_a_person_can_read() {
        assert_eq!(slug("Kovács és Társa Kft.", 40), "Kovacs-es-Tarsa-Kft");
        assert_eq!(slug("ŐRSÉGI ÜZEMÁG", 40), "ORSEGI-UZEMAG");
        assert_eq!(slug("ABC 123", 16), "ABC-123");
        assert_eq!(slug("  --  ", 10), "");
        assert_eq!(slug("Straße/Weg", 20), "Strasse-Weg");
        assert_eq!(stem_slug("IMG_0001 (2)", 60), "IMG_0001-2");
        assert_eq!(slug("abcdefghij-klm", 11), "abcdefghij");
    }

    #[test]
    fn project_folders() {
        assert_eq!(
            order_folder(2026, "2026-0042", "Kovács Kft.", Some("abc-123")),
            "projektek/2026/2026-0042_Kovacs-Kft_abc-123"
        );
        assert_eq!(
            order_folder(2026, "2026-0043", "Hűtő Bt", None),
            "projektek/2026/2026-0043_Huto-Bt"
        );
        assert_eq!(
            lead_folder(2026, 123, "Nagy Péter"),
            "ajanlatok/2026/L000123_Nagy-Peter"
        );
        assert_eq!(
            employee_folder(12, "Nagy Péter"),
            "hr/dolgozok/0012_Nagy-Peter"
        );
        assert_eq!(
            posting_folder(3, "Villanyszerelő (B)"),
            "hr/toborzas/0003_Villanyszerelo-B"
        );
    }

    #[test]
    fn folders_inside_a_project() {
        let p = "projektek/2026/2026-0042_Kovacs-Kft";
        assert_eq!(
            photo_dir(p, ImageCategory::Intake, false),
            "projektek/2026/2026-0042_Kovacs-Kft/fotok/bevetel"
        );
        assert_eq!(
            document_dir(p, DocumentKind::Cad, true),
            "projektek/2026/2026-0042_Kovacs-Kft/torolt/dokumentumok/cad"
        );
        assert_eq!(
            previews_dir(p),
            "projektek/2026/2026-0042_Kovacs-Kft/.elonezet"
        );
        assert_eq!(
            incoming_invoice_dir(NaiveDate::from_ymd_opt(2026, 3, 9).unwrap(), false),
            "bejovo-szamlak/2026/03"
        );
    }

    #[test]
    fn file_names_keep_the_original_and_a_piece_of_the_hash() {
        let h = "9f2c4be1aa00ffee";
        assert_eq!(
            photo_name(Some("IMG_0001.JPG"), h, "jpg"),
            "IMG_0001_9f2c4be1.jpg"
        );
        assert_eq!(photo_name(None, h, "jpg"), "foto_9f2c4be1.jpg");
        assert_eq!(
            photo_name(Some("../../x/Árvíztűrő.heic"), h, "heic"),
            "Arvizturo_9f2c4be1.heic"
        );
        assert_eq!(
            document_name("Ajánlat.pdf", 1, h, "pdf"),
            "Ajanlat_9f2c4be1.pdf"
        );
        assert_eq!(
            document_name("terv.dxf", 3, h, "dxf"),
            "terv_v3_9f2c4be1.dxf"
        );
        assert_eq!(
            derived_name(h, "thumb", "jpg"),
            "9f2c4be1aa00ffee.thumb.jpg"
        );
    }

    #[test]
    fn extensions_and_parents() {
        assert_eq!(ext_of("orders/1/intake/ab.JPG"), "jpg");
        assert_eq!(ext_of("orders/1/documents/ab.dxf.thumb.png"), "png");
        assert_eq!(ext_of("a/README"), "bin");
        assert_eq!(ext_of("a/.hidden"), "bin");
        assert_eq!(parent("a/b/c.jpg"), "a/b");
        assert_eq!(parent("c.jpg"), "");
    }
}
