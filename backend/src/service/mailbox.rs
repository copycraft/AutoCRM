//! Customer replies, read back from the sales mailbox. A reply is matched to what it is
//! about — by its headers when it answers one of our letters, else by the sender's address
//! — stored on that record's history, and stops the lead's waiting follow-ups: once the
//! customer has answered, "did you get our offer?" would be absurd.
//!
//! Mail that matches nothing in the CRM is left alone and not stored.

use chrono::{DateTime, Utc};
use mail_parser::{HeaderValue, MessageParser};
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use crate::AppState;
use crate::domain::email::normalize_address;
use crate::integrations::imap;
use crate::repo::{audit, followups, notifications};

/// What a message is about, as far as the CRM can tell.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Match {
    pub lead_id: Option<i64>,
    pub order_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub reply_to_email_id: Option<i64>,
}

impl Match {
    fn is_empty(&self) -> bool {
        self.lead_id.is_none() && self.order_id.is_none() && self.partner_id.is_none()
    }
}

/// Reads what arrived since last time. Runs every ten minutes from the scheduler.
pub async fn read_mailbox(state: &AppState) -> anyhow::Result<usize> {
    let Some(cfg) = &state.config.imap else {
        return Ok(0);
    };
    let cursor: Option<(i64, i64)> =
        sqlx::query_as("SELECT uid_validity, last_uid FROM mailbox_cursor WHERE folder = $1")
            .bind(&cfg.folder)
            .fetch_optional(&state.db)
            .await?;
    // A cursor from before the folder was renumbered is worthless.
    let probe = imap::fetch_new(cfg, cursor.map(|(_, uid)| uid as u32)).await?;
    let fetched = match cursor {
        Some((validity, _)) if validity != i64::from(probe.uid_validity) => {
            imap::fetch_new(cfg, None).await?
        }
        _ => probe,
    };
    let own = cfg.user.to_ascii_lowercase();
    let mut stored = 0;
    let mut last = cursor
        .filter(|(v, _)| *v == i64::from(fetched.uid_validity))
        .map(|(_, uid)| uid)
        .unwrap_or(0);
    for (uid, raw) in &fetched.messages {
        match ingest(&state.db, raw, &own).await {
            Ok(Some(_)) => stored += 1,
            Ok(None) => {}
            Err(e) => tracing::warn!(uid, error = %e, "could not store a mailbox message"),
        }
        last = last.max(i64::from(*uid));
    }
    sqlx::query(
        "INSERT INTO mailbox_cursor (folder, uid_validity, last_uid) VALUES ($1, $2, $3)
         ON CONFLICT (folder) DO UPDATE SET uid_validity = $2, last_uid = $3",
    )
    .bind(&cfg.folder)
    .bind(i64::from(fetched.uid_validity))
    .bind(last)
    .execute(&state.db)
    .await?;
    if stored > 0 {
        tracing::info!(stored, "customer replies read from the mailbox");
    }
    Ok(stored)
}

fn ids(value: &HeaderValue<'_>) -> Vec<String> {
    match value {
        HeaderValue::Text(t) => vec![t.to_string()],
        HeaderValue::TextList(list) => list.iter().map(|t| t.to_string()).collect(),
        _ => Vec::new(),
    }
}

fn bare(id: &str) -> String {
    id.trim().trim_start_matches('<').trim_end_matches('>').to_string()
}

/// Parses and stores one raw (RFC 822) message. `own_address` is the mailbox itself: what
/// it sent is not a reply. Returns the stored row's id, or None when the message was
/// already stored, came from us, or matches nothing.
pub async fn ingest(db: &PgPool, raw: &[u8], own_address: &str) -> anyhow::Result<Option<i64>> {
    let Some(msg) = MessageParser::default().parse(raw) else {
        return Ok(None);
    };
    let Some(from) = msg
        .from()
        .and_then(|a| a.first())
        .and_then(|a| a.address())
        .and_then(normalize_address)
    else {
        return Ok(None);
    };
    if from == own_address {
        return Ok(None);
    }
    let from_name = msg
        .from()
        .and_then(|a| a.first())
        .and_then(|a| a.name())
        .map(str::to_string);
    let subject = msg.subject().unwrap_or("").to_string();
    let body = msg
        .body_text(0)
        .map(|b| b.to_string())
        .unwrap_or_default();
    let received_at = msg
        .date()
        .and_then(|d| DateTime::<Utc>::from_timestamp(d.to_timestamp(), 0))
        .unwrap_or_else(Utc::now);
    let mut refs: Vec<String> = ids(msg.in_reply_to()).iter().map(|s| bare(s)).collect();
    // References lists the thread oldest first; the newest is the likeliest ours.
    refs.extend(ids(msg.references()).iter().rev().map(|s| bare(s)));
    let in_reply_to = refs.first().cloned();
    let message_id = msg
        .message_id()
        .map(bare)
        .unwrap_or_else(|| format!("{from}:{}:{}", received_at.timestamp(), subject.len()));

    let mut tx = db.begin().await?;
    let found = match_message(&mut tx, &refs, &from).await?;
    if found.is_empty() {
        return Ok(None);
    }
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO inbound_emails (message_id, in_reply_to, from_address, from_name, subject,
                                     body_text, received_at, lead_id, order_id, partner_id,
                                     reply_to_email_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT (message_id) DO NOTHING RETURNING id",
    )
    .bind(&message_id)
    .bind(&in_reply_to)
    .bind(&from)
    .bind(&from_name)
    .bind(&subject)
    .bind(body.chars().take(20_000).collect::<String>())
    .bind(received_at)
    .bind(found.lead_id)
    .bind(found.order_id)
    .bind(found.partner_id)
    .bind(found.reply_to_email_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(id) = id else {
        return Ok(None);
    };
    if let Some(lead_id) = found.lead_id {
        let stopped =
            followups::cancel_for_lead(&mut *tx, lead_id, "az ügyfél válaszolt").await?;
        audit::record(
            &mut *tx,
            None,
            "lead",
            lead_id,
            "customer_replied",
            json!({ "inbound_email_id": id, "followups_stopped": stopped }),
        )
        .await?;
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT assigned_to FROM leads WHERE id = $1")
                .bind(lead_id)
                .fetch_one(&mut *tx)
                .await?;
        if let Some(user) = owner {
            notifications::notify_user(
                &mut *tx,
                user,
                "customer_reply",
                "Válasz érkezett egy ügyféltől",
                Some(&format!("{}: {}", from_name.as_deref().unwrap_or(&from), subject)),
                Some(&format!("/leads/{lead_id}")),
            )
            .await?;
        }
    }
    tx.commit().await?;
    Ok(Some(id))
}

/// Our letter it answers, by its headers; else the newest open lead, an order, or a
/// partner with this address.
async fn match_message(
    conn: &mut PgConnection,
    refs: &[String],
    from: &str,
) -> sqlx::Result<Match> {
    for r in refs {
        let hit: Option<(i64, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, lead_id, order_id, partner_id FROM email_messages
              WHERE provider_id = $1 OR provider_id = '<' || $1 || '>'
              LIMIT 1",
        )
        .bind(r)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some((email_id, lead_id, order_id, partner_id)) = hit {
            if lead_id.is_some() || order_id.is_some() || partner_id.is_some() {
                return Ok(Match {
                    lead_id,
                    order_id,
                    partner_id,
                    reply_to_email_id: Some(email_id),
                });
            }
        }
    }
    let lead: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT l.id, l.partner_id FROM leads l
          LEFT JOIN partners p ON p.id = l.partner_id
          LEFT JOIN lead_current_stage cs ON cs.lead_id = l.id
          LEFT JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
          WHERE (lower(l.contact_email) = $1 OR lower(p.email) = $1)
          ORDER BY coalesce(sd.is_terminal, false), l.updated_at DESC
          LIMIT 1",
    )
    .bind(from)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((lead_id, partner_id)) = lead {
        return Ok(Match {
            lead_id: Some(lead_id),
            partner_id,
            ..Default::default()
        });
    }
    let partner: Option<i64> = sqlx::query_scalar(
        "SELECT p.id FROM partners p WHERE lower(p.email) = $1
         UNION ALL
         SELECT c.partner_id FROM contacts c WHERE lower(c.email) = $1 AND c.partner_id IS NOT NULL
         LIMIT 1",
    )
    .bind(from)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(partner_id) = partner {
        let order: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM orders WHERE partner_id = $1 ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(partner_id)
        .fetch_optional(&mut *conn)
        .await?;
        return Ok(Match {
            partner_id: Some(partner_id),
            order_id: order,
            ..Default::default()
        });
    }
    Ok(Match::default())
}
