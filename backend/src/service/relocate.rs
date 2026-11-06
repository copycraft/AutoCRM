//! Re-sorting: puts every stored file where [`layout`] says it belongs. Photos and their
//! previews, documents, invoices, stornos and proformas, walkaround signatures and videos,
//! HR papers, CVs and incoming invoices: everything with a row is filed in a project folder
//! or in one of the general ones.
//!
//! Runs from `autocrm storage-relayout` (once after switching to the share; `--from-s3`
//! copies every file over from the old object store on the way), daily in the worker, and
//! right after an enquiry becomes an order, so its files join the project.
//!
//! A file counts as in place when it is in its expected folder, whatever its name; only
//! files elsewhere move. On the share a move is a hard link, the row update, then removing
//! the old name once no row points at it: the bytes are never copied and never missing.
//! The old key stays in `storage_moves`, so a download link issued before still works.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;
use crate::domain::media::{DocumentKind, ImageCategory};
use crate::media::layout::{self, InspectionPart, Shelf};
use crate::media::storage::Storage;
use crate::repo::audit;
use crate::repo::documents::Owner;

/// Held while re-sorting, so the worker and a hand-started run never move the same file.
const LOCK_ID: i64 = 0x6175_746f_6669_6c65; // "autofile"

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    /// An enquiry's documents, after it became an order.
    Lead(i64),
}

pub struct Options {
    pub scope: Scope,
    /// Copy from here (the old object store) instead of moving within the share.
    pub source: Option<Storage>,
    /// Report what would move; touch nothing.
    pub dry_run: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub checked: usize,
    pub in_place: usize,
    pub moved: usize,
    pub copied: usize,
    pub missing: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    ImageOriginal,
    ImageDisplay,
    ImageThumb,
    Document,
    DocumentThumb,
    EmployeePhoto,
    EmployeeDocument,
    Absence,
    Application,
    IncomingInvoice,
}

impl Column {
    fn update_sql(self) -> &'static str {
        match self {
            Column::ImageOriginal => {
                "UPDATE images SET storage_key = $3 WHERE id = $1 AND storage_key = $2"
            }
            Column::ImageDisplay => {
                "UPDATE images SET display_key = $3 WHERE id = $1 AND display_key = $2"
            }
            Column::ImageThumb => {
                "UPDATE images SET thumb_key = $3 WHERE id = $1 AND thumb_key = $2"
            }
            Column::Document => {
                "UPDATE documents SET storage_key = $3 WHERE id = $1 AND storage_key = $2"
            }
            Column::DocumentThumb => {
                "UPDATE documents SET thumb_key = $3 WHERE id = $1 AND thumb_key = $2"
            }
            Column::EmployeePhoto => {
                "UPDATE employees SET photo_key = $3 WHERE id = $1 AND photo_key = $2"
            }
            Column::EmployeeDocument => {
                "UPDATE employee_documents SET file_key = $3 WHERE id = $1 AND file_key = $2"
            }
            Column::Absence => "UPDATE absences SET file_key = $3 WHERE id = $1 AND file_key = $2",
            Column::Application => {
                "UPDATE job_applications SET resume_key = $3 WHERE id = $1 AND resume_key = $2"
            }
            Column::IncomingInvoice => {
                "UPDATE incoming_invoices SET file_key = $3 WHERE id = $1 AND file_key = $2"
            }
        }
    }
}

/// One stored file and where it belongs.
#[derive(Debug, Clone)]
struct Placement {
    column: Column,
    id: i64,
    key: String,
    dir: String,
    /// The name it gets if it has to move.
    name: String,
    sha256: Option<Vec<u8>>,
    size: Option<i64>,
    /// Intake evidence: read-only on the share, and its move is audited.
    evidence_of_order: Option<i64>,
}

impl Placement {
    fn target(&self) -> String {
        if layout::parent(&self.key) == self.dir {
            self.key.clone()
        } else {
            layout::join(&self.dir, &self.name)
        }
    }
}

/// Folder names, looked up once per run.
struct Folders<'a> {
    state: &'a AppState,
    orders: HashMap<i64, String>,
    employees: HashMap<i64, String>,
    postings: HashMap<i64, String>,
}

impl<'a> Folders<'a> {
    fn new(state: &'a AppState) -> Self {
        Folders {
            state,
            orders: HashMap::new(),
            employees: HashMap::new(),
            postings: HashMap::new(),
        }
    }

    async fn order(&mut self, id: i64) -> sqlx::Result<String> {
        if let Some(f) = self.orders.get(&id) {
            return Ok(f.clone());
        }
        let f = layout::order_folder_of(self.state, id).await?;
        self.orders.insert(id, f.clone());
        Ok(f)
    }

    async fn employee(&mut self, id: i64) -> sqlx::Result<String> {
        if let Some(f) = self.employees.get(&id) {
            return Ok(f.clone());
        }
        let f = layout::employee_folder_of(self.state, id).await?;
        self.employees.insert(id, f.clone());
        Ok(f)
    }

    async fn posting(&mut self, id: i64) -> sqlx::Result<String> {
        if let Some(f) = self.postings.get(&id) {
            return Ok(f.clone());
        }
        let f = layout::posting_folder_of(self.state, id).await?;
        self.postings.insert(id, f.clone());
        Ok(f)
    }

    fn local_date(&self, at: DateTime<Utc>) -> NaiveDate {
        at.with_timezone(&self.state.config.business_tz)
            .date_naive()
    }
}

/// The walkaround a photo, signature or video belongs to.
#[derive(Debug, Clone)]
struct Walkaround {
    id: i64,
    kind: String,
    plate: String,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct ImageRow {
    id: i64,
    order_id: i64,
    category: ImageCategory,
    storage_key: String,
    display_key: Option<String>,
    thumb_key: Option<String>,
    original_filename: Option<String>,
    content_hash: Vec<u8>,
    byte_size: i64,
    immutable: bool,
    deleted: bool,
    inspection_id: Option<i64>,
    inspection_kind: Option<String>,
    inspection_plate: Option<String>,
    inspection_created_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct DocumentRow {
    id: i64,
    order_id: Option<i64>,
    lead_id: Option<i64>,
    kind: DocumentKind,
    filename: String,
    version: i32,
    storage_key: String,
    thumb_key: Option<String>,
    content_hash: Vec<u8>,
    byte_size: i64,
    deleted: bool,
    storno: bool,
    inspection_part: Option<String>,
    inspection_id: Option<i64>,
    inspection_kind: Option<String>,
    inspection_plate: Option<String>,
    inspection_created_at: Option<DateTime<Utc>>,
}

fn walkaround(
    id: Option<i64>,
    kind: Option<String>,
    plate: Option<String>,
    created_at: Option<DateTime<Utc>>,
) -> Option<Walkaround> {
    Some(Walkaround {
        id: id?,
        kind: kind?,
        plate: plate?,
        created_at: created_at?,
    })
}

async fn walkaround_dir(
    folders: &mut Folders<'_>,
    order_id: i64,
    w: &Walkaround,
    part: InspectionPart,
) -> sqlx::Result<String> {
    let project = folders.order(order_id).await?;
    let on = folders.local_date(w.created_at);
    Ok(layout::inspection_part(
        &layout::inspection_dir(&project, on, &w.kind, &w.plate, w.id),
        part,
    ))
}

async fn image_placements(folders: &mut Folders<'_>, out: &mut Vec<Placement>) -> sqlx::Result<()> {
    let rows: Vec<ImageRow> = sqlx::query_as(
        "SELECT i.id, i.order_id, i.category, i.storage_key, i.display_key, i.thumb_key,
                i.original_filename, i.content_hash, i.byte_size, i.immutable,
                i.deleted_at IS NOT NULL AS deleted,
                ins.id AS inspection_id, ins.kind AS inspection_kind,
                ins.vehicle_plate AS inspection_plate, ins.created_at AS inspection_created_at
           FROM images i
           LEFT JOIN inspection_photos ip ON ip.image_id = i.id
           LEFT JOIN inspections ins ON ins.id = ip.inspection_id
          ORDER BY i.id",
    )
    .fetch_all(&folders.state.db)
    .await?;
    for r in rows {
        let hash_hex = hex::encode(&r.content_hash);
        let project = folders.order(r.order_id).await?;
        let walk = walkaround(
            r.inspection_id,
            r.inspection_kind,
            r.inspection_plate,
            r.inspection_created_at,
        );
        let dir = match (&walk, r.deleted) {
            (Some(w), false) => {
                walkaround_dir(folders, r.order_id, w, InspectionPart::Photo).await?
            }
            _ => layout::photo_dir(&project, r.category, r.deleted),
        };
        out.push(Placement {
            column: Column::ImageOriginal,
            id: r.id,
            name: layout::photo_name(
                r.original_filename.as_deref(),
                &hash_hex,
                &layout::ext_of(&r.storage_key),
            ),
            key: r.storage_key,
            dir,
            sha256: Some(r.content_hash.clone()),
            size: Some(r.byte_size),
            evidence_of_order: r.immutable.then_some(r.order_id),
        });
        let previews = layout::previews_dir(&project);
        for (column, key, variant) in [
            (Column::ImageDisplay, r.display_key, "display"),
            (Column::ImageThumb, r.thumb_key, "thumb"),
        ] {
            if let Some(key) = key {
                out.push(Placement {
                    column,
                    id: r.id,
                    key,
                    dir: previews.clone(),
                    name: layout::derived_name(&hash_hex, variant, "jpg"),
                    sha256: None,
                    size: None,
                    evidence_of_order: None,
                });
            }
        }
    }
    Ok(())
}

async fn document_placements(
    folders: &mut Folders<'_>,
    scope: Scope,
    out: &mut Vec<Placement>,
) -> sqlx::Result<()> {
    let lead = match scope {
        Scope::All => None,
        Scope::Lead(id) => Some(id),
    };
    let rows: Vec<DocumentRow> = sqlx::query_as(
        "SELECT DISTINCT ON (d.id)
                d.id, d.order_id, d.lead_id, d.kind, d.filename, d.version, d.storage_key,
                d.thumb_key, d.content_hash, d.byte_size, d.deleted_at IS NOT NULL AS deleted,
                coalesce(inv.kind::text = 'storno', false) AS storno,
                CASE WHEN sig.id IS NOT NULL THEN 'signature'
                     WHEN vid.id IS NOT NULL THEN 'video' END AS inspection_part,
                ins.id AS inspection_id, ins.kind AS inspection_kind,
                ins.vehicle_plate AS inspection_plate, ins.created_at AS inspection_created_at
           FROM documents d
           LEFT JOIN invoices inv ON inv.document_id = d.id
           LEFT JOIN inspection_signatures sig ON sig.document_id = d.id
           LEFT JOIN inspection_videos vid ON vid.document_id = d.id
           LEFT JOIN inspections ins ON ins.id = coalesce(sig.inspection_id, vid.inspection_id)
          WHERE $1::bigint IS NULL OR d.lead_id = $1
          ORDER BY d.id",
    )
    .bind(lead)
    .fetch_all(&folders.state.db)
    .await?;
    for r in rows {
        let Some(owner) = Owner::from_about(r.order_id, r.lead_id) else {
            continue;
        };
        let hash_hex = hex::encode(&r.content_hash);
        let shelf = if r.storno {
            Shelf::Storno
        } else {
            Shelf::Kind(r.kind)
        };
        let walk = walkaround(
            r.inspection_id,
            r.inspection_kind,
            r.inspection_plate,
            r.inspection_created_at,
        );
        let part = match r.inspection_part.as_deref() {
            Some("signature") => Some(InspectionPart::Signature),
            Some("video") => Some(InspectionPart::Video),
            _ => None,
        };
        let dir = match (owner, &walk, part, r.deleted) {
            (Owner::Order(order_id), Some(w), Some(part), false) => {
                walkaround_dir(folders, order_id, w, part).await?
            }
            (Owner::Order(order_id), _, _, deleted) => {
                layout::document_dir(&folders.order(order_id).await?, shelf, deleted)
            }
            (Owner::Lead(_), _, _, deleted) => {
                layout::document_dir_of(folders.state, owner, shelf, deleted).await?
            }
        };
        out.push(Placement {
            column: Column::Document,
            id: r.id,
            name: layout::document_name(
                &r.filename,
                r.version,
                &hash_hex,
                &layout::ext_of(&r.storage_key),
            ),
            key: r.storage_key,
            dir,
            sha256: Some(r.content_hash.clone()),
            size: Some(r.byte_size),
            evidence_of_order: None,
        });
        if let Some(key) = r.thumb_key {
            out.push(Placement {
                column: Column::DocumentThumb,
                id: r.id,
                key,
                dir: layout::previews_dir(&layout::owner_folder(folders.state, owner).await?),
                name: layout::derived_name(&hash_hex, "thumb", "png"),
                sha256: None,
                size: None,
                evidence_of_order: None,
            });
        }
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct PaperRow {
    id: i64,
    employee_id: i64,
    kind: String,
    file_name: Option<String>,
    file_key: String,
    file_size: Option<i64>,
    deleted: bool,
}

#[derive(sqlx::FromRow)]
struct AbsenceFileRow {
    id: i64,
    employee_id: i64,
    start_date: NaiveDate,
    file_name: Option<String>,
    file_key: String,
    file_size: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct IncomingRow {
    id: i64,
    created_at: DateTime<Utc>,
    file_name: Option<String>,
    file_key: String,
    file_hash: Option<Vec<u8>>,
    file_size: Option<i64>,
    deleted: bool,
}

async fn hr_placements(folders: &mut Folders<'_>, out: &mut Vec<Placement>) -> sqlx::Result<()> {
    let db = &folders.state.db;
    let photos: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, photo_key FROM employees WHERE photo_key IS NOT NULL ORDER BY id",
    )
    .fetch_all(db)
    .await?;
    let papers: Vec<PaperRow> = sqlx::query_as(
        "SELECT id, employee_id, kind, file_name, file_key, file_size, deleted_at IS NOT NULL AS deleted
           FROM employee_documents WHERE file_key IS NOT NULL ORDER BY id",
    )
    .fetch_all(db)
    .await?;
    let absences: Vec<AbsenceFileRow> = sqlx::query_as(
        "SELECT id, employee_id, start_date, file_name, file_key, file_size
           FROM absences WHERE file_key IS NOT NULL ORDER BY id",
    )
    .fetch_all(db)
    .await?;
    let resumes: Vec<(i64, i64, String, DateTime<Utc>, String)> = sqlx::query_as(
        "SELECT id, posting_id, full_name, created_at, resume_key FROM job_applications ORDER BY id",
    )
    .fetch_all(db)
    .await?;

    for (id, key) in photos {
        out.push(Placement {
            column: Column::EmployeePhoto,
            id,
            dir: layout::employee_photo_dir(&folders.employee(id).await?),
            name: format!("profilkep_{}.jpg", layout::random_tag()),
            key,
            sha256: None,
            size: None,
            evidence_of_order: None,
        });
    }
    for PaperRow {
        id,
        employee_id,
        kind,
        file_name,
        file_key: key,
        file_size: size,
        deleted,
    } in papers
    {
        let folder = folders.employee(employee_id).await?;
        out.push(Placement {
            column: Column::EmployeeDocument,
            id,
            dir: layout::employee_document_dir(&folder, &kind, deleted),
            name: layout::file_name(
                file_name.as_deref(),
                "dokumentum",
                &layout::random_tag(),
                &layout::ext_of(&key),
            ),
            key,
            sha256: None,
            size,
            evidence_of_order: None,
        });
    }
    for AbsenceFileRow {
        id,
        employee_id,
        start_date: start,
        file_name,
        file_key: key,
        file_size: size,
    } in absences
    {
        let folder = folders.employee(employee_id).await?;
        out.push(Placement {
            column: Column::Absence,
            id,
            dir: layout::absence_dir(&folder),
            name: format!(
                "{start}_{}",
                layout::file_name(
                    file_name.as_deref(),
                    "igazolas",
                    &layout::random_tag(),
                    &layout::ext_of(&key),
                )
            ),
            key,
            sha256: None,
            size,
            evidence_of_order: None,
        });
    }
    for (id, posting_id, full_name, created_at, key) in resumes {
        let on = folders.local_date(created_at);
        out.push(Placement {
            column: Column::Application,
            id,
            dir: folders.posting(posting_id).await?,
            name: format!(
                "{on}_{}_{}.{}",
                layout::stem_slug(&full_name, 40),
                layout::random_tag(),
                layout::ext_of(&key)
            ),
            key,
            sha256: None,
            size: None,
            evidence_of_order: None,
        });
    }
    Ok(())
}

async fn incoming_invoice_placements(
    folders: &mut Folders<'_>,
    out: &mut Vec<Placement>,
) -> sqlx::Result<()> {
    let rows: Vec<IncomingRow> = sqlx::query_as(
        "SELECT id, created_at, file_name, file_key, file_hash, file_size,
                    deleted_at IS NOT NULL AS deleted
               FROM incoming_invoices WHERE file_key IS NOT NULL ORDER BY id",
    )
    .fetch_all(&folders.state.db)
    .await?;
    for IncomingRow {
        id,
        created_at,
        file_name,
        file_key: key,
        file_hash: hash,
        file_size: size,
        deleted,
    } in rows
    {
        let tag = hash
            .as_deref()
            .map(|h| layout::hash_tag(&hex::encode(h)).to_string())
            .unwrap_or_else(layout::random_tag);
        out.push(Placement {
            column: Column::IncomingInvoice,
            id,
            dir: layout::incoming_invoice_dir(folders.local_date(created_at), deleted),
            name: layout::file_name(file_name.as_deref(), "szamla", &tag, &layout::ext_of(&key)),
            key,
            sha256: hash,
            size,
            evidence_of_order: None,
        });
    }
    Ok(())
}

/// Every key a row points at: what may not be deleted.
async fn referenced_keys(db: &PgPool) -> sqlx::Result<HashSet<String>> {
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT k FROM (
            SELECT storage_key AS k FROM images
            UNION ALL SELECT display_key FROM images
            UNION ALL SELECT thumb_key FROM images
            UNION ALL SELECT storage_key FROM documents
            UNION ALL SELECT thumb_key FROM documents
            UNION ALL SELECT photo_key FROM employees
            UNION ALL SELECT file_key FROM employee_documents
            UNION ALL SELECT file_key FROM absences
            UNION ALL SELECT resume_key FROM job_applications
            UNION ALL SELECT file_key FROM incoming_invoices
         ) refs WHERE k IS NOT NULL",
    )
    .fetch_all(db)
    .await?;
    Ok(keys.into_iter().collect())
}

/// Points the row at its new key. False when the row changed meanwhile (nothing written).
async fn record_move(
    state: &AppState,
    p: &Placement,
    target: &str,
    forward: bool,
) -> sqlx::Result<bool> {
    let mut tx = state.db.begin().await?;
    if p.column == Column::ImageOriginal {
        // The one way past the write-once trigger (0052): same bytes, new place.
        sqlx::query("SET LOCAL autocrm.relocating = 'on'")
            .execute(&mut *tx)
            .await?;
    }
    let done = sqlx::query(p.column.update_sql())
        .bind(p.id)
        .bind(&p.key)
        .bind(target)
        .execute(&mut *tx)
        .await?;
    if done.rows_affected() == 0 {
        return Ok(false);
    }
    if forward {
        sqlx::query(
            "INSERT INTO storage_moves (old_key, new_key) VALUES ($1, $2)
             ON CONFLICT (old_key) DO UPDATE SET new_key = EXCLUDED.new_key, moved_at = now()",
        )
        .bind(&p.key)
        .bind(target)
        .execute(&mut *tx)
        .await?;
    }
    if let Some(order_id) = p.evidence_of_order {
        audit::record(
            &mut *tx,
            None,
            "order",
            order_id,
            "image_relocated",
            json!({ "image_id": p.id, "from": p.key, "to": target }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Files every stored file in its folder. Needs `STORAGE_BACKEND=fs`.
pub async fn run(state: &AppState, opts: &Options) -> anyhow::Result<Summary> {
    let fs = state
        .storage
        .as_fs()
        .ok_or_else(|| anyhow::anyhow!("re-sorting files needs STORAGE_BACKEND=fs"))?;

    // One run at a time. A transaction's lock, not a session's: a run cut short (the
    // worker's job timeout) rolls the transaction back and the lock goes with it, instead of
    // staying on a pooled connection forever.
    let mut lock = state.db.begin().await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(LOCK_ID)
        .fetch_one(&mut *lock)
        .await?;
    if !locked {
        tracing::info!("files are being re-sorted elsewhere; skipping");
        return Ok(Summary::default());
    }
    let result = sort(state, fs, opts).await;
    let _ = lock.rollback().await;
    result
}

async fn sort(
    state: &AppState,
    fs: &crate::media::fs_store::FsStore,
    opts: &Options,
) -> anyhow::Result<Summary> {
    let mut folders = Folders::new(state);
    let mut placements = Vec::new();
    if opts.scope == Scope::All {
        image_placements(&mut folders, &mut placements).await?;
    }
    document_placements(&mut folders, opts.scope, &mut placements).await?;
    if opts.scope == Scope::All {
        hr_placements(&mut folders, &mut placements).await?;
        incoming_invoice_placements(&mut folders, &mut placements).await?;
    }

    let mut summary = Summary::default();
    // (old key, new key, read-only) of every move within the share.
    let mut moved: Vec<(String, String, bool)> = Vec::new();
    for p in &placements {
        summary.checked += 1;
        let target = p.target();
        match &opts.source {
            None => {
                if target == p.key {
                    summary.in_place += 1;
                    continue;
                }
                if opts.dry_run {
                    tracing::info!(from = %p.key, to = %target, "would move");
                    summary.moved += 1;
                    continue;
                }
                if fs.head(&p.key).await?.is_none() {
                    tracing::warn!(key = %p.key, "re-sorting: the file is missing");
                    summary.missing += 1;
                    continue;
                }
                if let Err(e) = fs.link(&p.key, &target).await {
                    tracing::error!(from = %p.key, to = %target, error = %e, "re-sorting: could not move");
                    summary.failed += 1;
                    continue;
                }
                match record_move(state, p, &target, true).await {
                    Ok(true) => {
                        summary.moved += 1;
                        moved.push((p.key.clone(), target, p.evidence_of_order.is_some()));
                    }
                    // Changed under us: leave it for the next run.
                    Ok(false) => {}
                    Err(e) => {
                        tracing::error!(key = %p.key, error = %e, "re-sorting: could not update the row");
                        summary.failed += 1;
                    }
                }
            }
            Some(source) => {
                if fs.head(&target).await?.is_some() {
                    // Copied by an earlier run that stopped before the row changed.
                    if target == p.key {
                        summary.in_place += 1;
                    } else if !opts.dry_run && record_move(state, p, &target, false).await? {
                        summary.copied += 1;
                    }
                    continue;
                }
                if opts.dry_run {
                    tracing::info!(from = %p.key, to = %target, "would copy");
                    summary.copied += 1;
                    continue;
                }
                let stream = match source.open(&p.key).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(key = %p.key, error = %e, "copying: not in the object store");
                        summary.missing += 1;
                        continue;
                    }
                };
                let read_only = p.evidence_of_order.is_some();
                if let Err(e) = fs
                    .write_stream(&target, stream, p.size, p.sha256.as_deref(), read_only)
                    .await
                {
                    tracing::error!(from = %p.key, to = %target, error = %e, "copying failed");
                    summary.failed += 1;
                    continue;
                }
                if target != p.key && !record_move(state, p, &target, false).await? {
                    continue;
                }
                summary.copied += 1;
            }
        }
    }

    // Old names nothing points at any more. A file several rows shared (a MiniCRM download
    // attached to two orders) goes only once every row has moved.
    if !moved.is_empty() {
        let referenced = referenced_keys(&state.db).await?;
        for (old, new, read_only) in &moved {
            if referenced.contains(old) {
                continue;
            }
            if let Err(e) = fs.delete(old).await {
                tracing::warn!(key = %old, error = %e, "re-sorting: could not remove the old name");
            }
            // Windows clears read-only on the shared file to delete one of its names.
            if *read_only {
                let _ = fs.lock_object(new, Utc::now()).await;
            }
        }
    }
    Ok(summary)
}

/// The worker's daily pass: re-sort, forget forwarding older than any link, and clear
/// uploads that died half way. Nothing to do on an object store.
pub async fn tidy(state: &AppState, scope: Scope) -> anyhow::Result<()> {
    let Some(fs) = state.storage.as_fs() else {
        return Ok(());
    };
    let summary = run(
        state,
        &Options {
            scope,
            source: None,
            dry_run: false,
        },
    )
    .await?;
    if summary.moved + summary.missing + summary.failed > 0 {
        tracing::info!(?summary, ?scope, "files re-sorted");
    }
    if scope == Scope::All {
        // Download links live a week at most.
        sqlx::query("DELETE FROM storage_moves WHERE moved_at < now() - interval '30 days'")
            .execute(&state.db)
            .await?;
        let swept = fs.sweep_partials().await?;
        if swept > 0 {
            tracing::info!(swept, "abandoned partial uploads removed");
        }
    }
    Ok(())
}
