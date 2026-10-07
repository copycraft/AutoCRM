//! A record's history in one list, newest first, the way MiniCRM showed it: data changes,
//! status changes, uploaded files, tasks, emails and notes. Built from the tables that
//! already hold each fact; nothing is written twice to feed it.
//!
//! The audit log is the base. Where a dedicated table holds the same fact with more in it
//! (stage history with its label and note, the file itself, imported MiniCRM rows that
//! never went through the audit log), that table speaks and the audit row is skipped.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimelineEntity {
    Lead,
    Order,
    Partner,
    Employee,
    IncomingInvoice,
}

impl TimelineEntity {
    /// The `audit_log.entity` value.
    fn audit_name(self) -> &'static str {
        match self {
            TimelineEntity::Lead => "lead",
            TimelineEntity::Order => "order",
            TimelineEntity::Partner => "partner",
            TimelineEntity::Employee => "employee",
            TimelineEntity::IncomingInvoice => "incoming_invoice",
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct TimelineEvent {
    pub at: DateTime<Utc>,
    /// create | change | stage | file | task | email | note | event
    pub kind: String,
    /// The finer action: the audit action, the stage key, the task's created/done,
    /// the email status, the image category or document kind.
    pub action: String,
    /// Who did it; null for the system, the website, or an import.
    pub user_name: Option<String>,
    /// For `change` (and some events): `{field: [old, new]}`, references already turned
    /// into names (partner, contact, user).
    #[schema(value_type = Object)]
    pub changes: Value,
    /// The headline: stage label, task title, email subject, note body, order number.
    pub text: Option<String>,
    /// A second line: stage note, email recipient, task due date.
    pub detail: Option<String>,
    pub file_name: Option<String>,
    pub file_size: Option<i64>,
    /// Open with `/documents/{id}/download`.
    pub document_id: Option<i64>,
    /// Open with `/images/{id}/original`.
    pub image_id: Option<i64>,
    /// Open with `/incoming-invoices/{id}/file`.
    pub incoming_invoice_id: Option<i64>,
    /// The email at `/emails/{id}`.
    pub email_id: Option<i64>,
    /// An order this event links to (a partner's new order, a lead's conversion).
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
}

/// Audit rows, minus the actions a dedicated source below shows better.
const AUDIT: &str = "
    SELECT a.at,
           CASE WHEN a.action = 'create' THEN 'create'
                WHEN a.action = 'update' THEN 'change'
                ELSE 'event' END AS kind,
           a.action, u.display_name AS user_name, a.changes,
           NULL::text AS text, NULL::text AS detail, NULL::text AS file_name, NULL::bigint AS file_size,
           NULL::bigint AS document_id, NULL::bigint AS image_id, NULL::bigint AS incoming_invoice_id,
           NULL::bigint AS email_id, NULL::bigint AS order_id, NULL::bigint AS lead_id
    FROM audit_log a LEFT JOIN users u ON u.id = a.user_id
    WHERE a.entity = $1 AND a.entity_id = $2
      AND a.action NOT IN ('stage_change', 'document_add', 'document_version', 'image_add', 'upload', 'vehicle_moved', 'incident')";

/// One row shape for everything that is not an audit row; `{cols}` fills the columns
/// in the order of `TimelineEvent`.
fn row(cols: &str) -> String {
    format!("SELECT {cols}")
}

fn documents(owner_col: &str) -> String {
    row(&format!(
        "d.uploaded_at, 'file', d.kind::text, u.display_name, '{{}}'::jsonb,
         NULL, NULL, d.filename, d.byte_size, d.id, NULL, NULL, NULL, NULL, NULL
         FROM documents d LEFT JOIN users u ON u.id = d.uploaded_by
         WHERE d.{owner_col} = $2 AND d.deleted_at IS NULL"
    ))
}

fn tasks(entity: &str) -> String {
    format!(
        "{created} UNION ALL {done}",
        created = row(&format!(
            "t.created_at, 'task', 'created', u.display_name, '{{}}'::jsonb,
             t.title, t.due_date::text, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
             FROM tasks t LEFT JOIN users u ON u.id = t.created_by
             WHERE t.entity_type = '{entity}' AND t.entity_id = $2"
        )),
        done = row(&format!(
            "t.done_at, 'task', 'done', u.display_name, '{{}}'::jsonb,
             t.title, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
             FROM tasks t LEFT JOIN users u ON u.id = t.assigned_to
             WHERE t.entity_type = '{entity}' AND t.entity_id = $2 AND t.done_at IS NOT NULL"
        )),
    )
}

fn emails(owner_col: &str) -> String {
    row(&format!(
        "coalesce(e.sent_at, e.queued_at), 'email', e.status::text, u.display_name,
         jsonb_build_object('automatic', e.is_automatic),
         e.subject, e.to_address, NULL, NULL, NULL, NULL, NULL, e.id, NULL, NULL
         FROM email_messages e LEFT JOIN users u ON u.id = e.sent_by
         WHERE e.{owner_col} = $2"
    ))
}

/// Replies read back from the sales mailbox.
fn inbound(owner_col: &str) -> String {
    row(&format!(
        "m.received_at, 'email', 'received', coalesce(m.from_name, m.from_address),
         jsonb_build_object('inbound', true, 'from', m.from_address),
         m.subject, left(m.body_text, 400), NULL, NULL, NULL, NULL, NULL, m.reply_to_email_id, NULL, NULL
         FROM inbound_emails m WHERE m.{owner_col} = $2"
    ))
}

fn comments(entity: &str) -> String {
    row(&format!(
        "c.created_at, 'note', 'comment', u.display_name, '{{}}'::jsonb,
         c.body, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
         FROM comments c JOIN users u ON u.id = c.created_by
         WHERE c.entity_type = '{entity}' AND c.entity_id = $2 AND c.deleted_at IS NULL"
    ))
}

fn stages(table: &str, entity: &str, owner_col: &str) -> String {
    row(&format!(
        "s.entered_at, 'stage', s.stage_key, u.display_name, '{{}}'::jsonb,
         coalesce(sd.label_hu, s.stage_key), s.note, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
         FROM {table} s
         LEFT JOIN stage_definitions sd ON sd.entity = '{entity}' AND sd.key = s.stage_key
         LEFT JOIN users u ON u.id = s.entered_by
         WHERE s.{owner_col} = $2"
    ))
}

fn sources(entity: TimelineEntity) -> Vec<String> {
    match entity {
        TimelineEntity::Lead => vec![
            stages("lead_stages", "lead", "lead_id"),
            documents("lead_id"),
            tasks("lead"),
            emails("lead_id"),
            inbound("lead_id"),
            comments("lead"),
            // Every order this enquiry became.
            row("o.created_at, 'event', 'order_created', NULL, '{}'::jsonb,
                 o.number, NULL, NULL, NULL, NULL, NULL, NULL, NULL, o.id, NULL
                 FROM orders o WHERE o.lead_id = $2"),
        ],
        TimelineEntity::Order => vec![
            stages("order_stages", "order", "order_id"),
            documents("order_id"),
            row("i.uploaded_at, 'file', i.category::text, u.display_name, '{}'::jsonb,
                 NULL, NULL, i.original_filename, i.byte_size, NULL, i.id, NULL, NULL, NULL, NULL
                 FROM images i LEFT JOIN users u ON u.id = i.uploaded_by
                 WHERE i.order_id = $2 AND i.deleted_at IS NULL"),
            tasks("order"),
            emails("order_id"),
            inbound("order_id"),
            comments("order"),
            // Where the vehicle was moved on the yard for this job.
            row("m.moved_at, 'event', 'vehicle_moved', u.display_name, '{}'::jsonb,
                 coalesce(l.name, 'Elvitték a telephelyről'), coalesce(v.plate, v.vin), NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
                 FROM vehicle_moves m JOIN vehicles v ON v.id = m.vehicle_id
                 LEFT JOIN yard_locations l ON l.id = m.location_id
                 LEFT JOIN users u ON u.id = m.moved_by
                 WHERE m.order_id = $2"),
            // Internal incidents logged against the job.
            row("i.created_at, 'event', 'incident', u.display_name, '{}'::jsonb,
                 i.title, i.description, NULL, NULL, NULL, NULL, NULL, NULL, i.rework_order_id, NULL
                 FROM incidents i LEFT JOIN users u ON u.id = i.created_by
                 WHERE i.order_id = $2"),
            // MiniCRM's own history of the job, imported as notes.
            row("n.occurred_at, 'note', 'note', n.author_name, '{}'::jsonb,
                 n.body, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
                 FROM order_notes n WHERE n.order_id = $2"),
        ],
        TimelineEntity::Partner => vec![
            tasks("partner"),
            emails("partner_id"),
            inbound("partner_id"),
            row("o.created_at, 'event', 'order_created', NULL, '{}'::jsonb,
                 o.number || ' · ' || o.title, NULL, NULL, NULL, NULL, NULL, NULL, NULL, o.id, NULL
                 FROM orders o WHERE o.partner_id = $2"),
            row("l.created_at, 'event', 'lead_created', NULL, '{}'::jsonb,
                 l.title, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, l.id
                 FROM leads l WHERE l.partner_id = $2"),
        ],
        TimelineEntity::Employee => vec![row(
            "d.created_at, 'file', d.kind, u.display_name, '{}'::jsonb,
             d.title, d.valid_until::text, d.file_name, d.file_size, NULL, NULL, NULL, NULL, NULL, NULL
             FROM employee_documents d LEFT JOIN users u ON u.id = d.created_by
             WHERE d.employee_id = $2 AND d.deleted_at IS NULL",
        )],
        TimelineEntity::IncomingInvoice => vec![row(
            "i.created_at, 'file', 'upload', u.display_name, '{}'::jsonb,
             NULL, NULL, i.file_name, i.file_size, NULL, NULL, i.id, NULL, NULL, NULL
             FROM incoming_invoices i LEFT JOIN users u ON u.id = i.created_by
             WHERE i.id = $2 AND i.file_key IS NOT NULL",
        )],
    }
}

pub async fn list(
    db: impl PgExecutor<'_>,
    entity: TimelineEntity,
    id: i64,
    limit: i64,
) -> sqlx::Result<Vec<TimelineEvent>> {
    let mut parts = vec![AUDIT.to_string()];
    parts.extend(sources(entity));
    let sql = format!(
        "SELECT * FROM ({}) t WHERE t.at IS NOT NULL ORDER BY t.at DESC LIMIT $3",
        parts.join(" UNION ALL ")
    );
    sqlx::query_as(&sql)
        .bind(entity.audit_name())
        .bind(id)
        .bind(limit)
        .fetch_all(db)
        .await
}

/// The names behind the ids a change mentions, so the history reads "Busa Ádám
/// (Felelős)" rather than "7 (assigned_to)".
pub async fn names(
    db: impl PgExecutor<'_>,
    users: &[i64],
    partners: &[i64],
    contacts: &[i64],
) -> sqlx::Result<Vec<(String, i64, String)>> {
    sqlx::query_as(
        "SELECT 'user', id, display_name FROM users WHERE id = ANY($1)
         UNION ALL SELECT 'partner', id, name FROM partners WHERE id = ANY($2)
         UNION ALL SELECT 'contact', id, name FROM contacts WHERE id = ANY($3)",
    )
    .bind(users)
    .bind(partners)
    .bind(contacts)
    .fetch_all(db)
    .await
}
