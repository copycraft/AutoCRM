//! Quote follow-ups: scheduling the sequence when a quotation goes out, and sending what
//! is due (hourly job). A lead that is won, lost or converted gets no more letters; a lead
//! with no address left is skipped with the reason, never failed silently.

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use sqlx::PgConnection;

use crate::AppState;
use crate::domain::email::normalize_address;
use crate::error::{AppError, AppResult};
use crate::repo::{audit, config, followups};
use crate::service::email::{About, AutomaticEmail, queue_automatic, triggers};

/// Schedules the follow-ups of a quotation sent `sent_at`, replacing any still waiting
/// from an earlier quotation of the same lead. `step_ids`: None is every active default
/// step; an empty list schedules nothing. Returns how many were scheduled.
pub async fn schedule_for_quotation(
    conn: &mut PgConnection,
    lead_id: i64,
    step_ids: Option<&[i64]>,
    sent_at: DateTime<Utc>,
    user_id: i64,
) -> AppResult<usize> {
    followups::cancel_for_lead(&mut *conn, lead_id, "új árajánlat ment ki").await?;
    let steps = followups::steps(&mut *conn, "quote").await?;
    let chosen: Vec<_> = match step_ids {
        None => steps.into_iter().filter(|s| s.is_active).collect(),
        Some(ids) => {
            let picked: Vec<_> = steps.into_iter().filter(|s| ids.contains(&s.id)).collect();
            if picked.len() != {
                let mut unique = ids.to_vec();
                unique.sort_unstable();
                unique.dedup();
                unique.len()
            } {
                return Err(AppError::validation("a follow-up step does not exist"));
            }
            picked
        }
    };
    for s in &chosen {
        followups::schedule(
            &mut *conn,
            lead_id,
            &s.label,
            &s.template_key,
            sent_at + TimeDelta::days(i64::from(s.delay_days)),
            Some(user_id),
        )
        .await?;
    }
    Ok(chosen.len())
}

/// Sends the follow-ups that are due. Hourly; the send window and the daily cap are the
/// email queue's business, so a letter due at night simply waits there for the morning.
pub async fn send_due(state: &AppState) -> anyhow::Result<usize> {
    let settings = config::settings(&state.db).await?;
    if !settings.automatic_email_enabled {
        tracing::debug!("automatic email is off; not sending quote follow-ups");
        return Ok(0);
    }
    let mut sent = 0;
    loop {
        let mut tx = state.db.begin().await?;
        let due = followups::lock_due(&mut *tx, Utc::now(), 20).await?;
        if due.is_empty() {
            break;
        }
        for d in due {
            let skip = if d.converted {
                Some("a lead már megrendelés lett")
            } else if d.lead_closed {
                Some("a lead lezárult")
            } else {
                None
            };
            if let Some(reason) = skip {
                followups::mark_skipped(&mut *tx, d.id, reason).await?;
                continue;
            }
            let to = d
                .contact_email
                .as_deref()
                .and_then(normalize_address)
                .or_else(|| d.partner_email.as_deref().and_then(normalize_address));
            let Some(to) = to else {
                followups::mark_skipped(&mut *tx, d.id, "nincs e-mail cím a leadhez").await?;
                continue;
            };
            let queued = queue_automatic(
                &mut tx,
                &state.config,
                AutomaticEmail {
                    template_key: &d.template_key,
                    about: About {
                        lead_id: Some(d.lead_id),
                        ..Default::default()
                    },
                    to: &to,
                    trigger: triggers::QUOTE_FOLLOWUP,
                    idempotency_key: format!("followup:{}", d.id),
                    attachments: Vec::new(),
                    extra_values: Default::default(),
                    sender: d.sender_name.clone().zip(d.sender_email.clone()),
                },
            )
            .await;
            match queued {
                Ok(Some(email_id)) => {
                    followups::mark_sent(&mut *tx, d.id, email_id).await?;
                    audit::record(
                        &mut *tx,
                        None,
                        "lead",
                        d.lead_id,
                        "followup_sent",
                        json!({ "followup_id": d.id, "email_id": email_id, "template": d.template_key }),
                    )
                    .await?;
                    sent += 1;
                }
                // Already queued under this key by an earlier, interrupted run.
                Ok(None) => followups::mark_skipped(&mut *tx, d.id, "már sorban volt").await?,
                // A broken template or address: say so on the row rather than retry forever.
                Err(e) => {
                    tracing::warn!(followup_id = d.id, error = %e, "could not queue a quote follow-up");
                    followups::mark_skipped(&mut *tx, d.id, &format!("nem sikerült: {e}")).await?;
                }
            }
        }
        tx.commit().await?;
    }
    if sent > 0 {
        tracing::info!(sent, "quote follow-ups queued");
    }
    Ok(sent)
}
