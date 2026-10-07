//! Automatic work: blocker nudges, stalled-order alerts, stage-change notifications,
//! exchange rates, image processing. Called from background jobs.

use chrono::{Datelike, NaiveDate, TimeDelta, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

use crate::AppState;
use crate::domain::blocker::{
    NudgeDecision, NudgePolicy, NudgeState, decide, nudge_idempotency_key,
};
use crate::domain::media::image_derived_key;
use crate::domain::money::Currency;
use crate::error::AppResult;
use crate::integrations::mnb::MnbClient;
use crate::media::pipeline;
use crate::repo::{audit, blockers, config, contacts, fx, images, orders, partners, reports};
use crate::service::business_today;
use crate::service::email::{About, AutomaticEmail, queue_automatic, triggers};

pub async fn nudge_blockers(state: &AppState) -> anyhow::Result<usize> {
    let settings = config::settings(&state.db).await?;
    if !settings.automatic_email_enabled {
        tracing::debug!("automatic email is off; not queueing blocker nudges");
        return Ok(0);
    }
    let policy = NudgePolicy {
        interval_days: settings.nudge_interval_days,
        escalate_after: settings.nudge_escalate_after,
    };
    let today = business_today(state.config.business_tz);
    let now = Utc::now();
    let mut queued = 0;

    for candidate in blockers::nudge_candidates(&state.db, today).await? {
        let decision = decide(
            NudgeState {
                due_date: candidate.due_date,
                resolved: false,
                nudge_enabled: candidate.nudge_enabled,
                last_nudged_at: candidate.last_nudged_at,
                nudge_count: candidate.nudge_count,
            },
            policy,
            today,
            now,
        );
        let NudgeDecision::Nudge { escalated, .. } = decision else {
            continue;
        };
        let Some(recipient) = candidate.recipient.as_deref() else {
            tracing::warn!(
                blocker_id = candidate.id,
                "blocker is overdue but has no recipient address; skipping nudge"
            );
            continue;
        };

        let mut tx = state.db.begin().await?;
        // Re-check under lock: another worker may have nudged or someone resolved it meanwhile.
        let Some(count) = blockers::lock_for_nudge(&mut *tx, candidate.id).await? else {
            continue;
        };
        if count != candidate.nudge_count {
            continue;
        }
        let template_key = if escalated {
            "blocker_nudge_escalated"
        } else {
            "blocker_nudge_first"
        };
        let email_id = queue_automatic(
            &mut tx,
            &state.config,
            AutomaticEmail {
                template_key,
                about: About {
                    order_id: Some(candidate.order_id),
                    blocker_id: Some(candidate.id),
                    ..Default::default()
                },
                to: recipient,
                trigger: triggers::NUDGE_BLOCKER,
                idempotency_key: nudge_idempotency_key(candidate.id, count + 1),
                attachments: Vec::new(),
                extra_values: Default::default(),
                sender: None,
            },
        )
        .await;
        match email_id {
            Ok(Some(email_id)) => {
                blockers::record_nudge(&mut *tx, candidate.id).await?;
                audit::record(
                    &mut *tx,
                    None,
                    "order",
                    candidate.order_id,
                    "blocker_nudge",
                    json!({ "blocker_id": candidate.id, "email_id": email_id, "nudge": count + 1, "escalated": escalated }),
                )
                .await?;
                tx.commit().await?;
                queued += 1;
            }
            Ok(None) => {} // already queued under this key
            Err(e) => {
                tracing::warn!(blocker_id = candidate.id, error = %e, "could not queue nudge")
            }
        }
    }
    if queued > 0 {
        tracing::info!(queued, "blocker nudges queued");
    }
    Ok(queued)
}

pub async fn stalled_order_alerts(state: &AppState) -> anyhow::Result<usize> {
    let settings = config::settings(&state.db).await?;
    if !settings.automatic_email_enabled || settings.stalled_alert_recipients.is_empty() {
        return Ok(0);
    }
    let week = business_today(state.config.business_tz).iso_week();
    let mut queued = 0;
    for stalled in reports::stalled_orders(&state.db).await? {
        for recipient in &settings.stalled_alert_recipients {
            let mut tx = state.db.begin().await?;
            // One alert per order, per stage visit, per recipient, per ISO week.
            let key = format!(
                "stalled:{}:{}:{}:{}-W{:02}",
                stalled.order_id,
                stalled.entered_at.timestamp(),
                recipient,
                week.year(),
                week.week()
            );
            let result = queue_automatic(
                &mut tx,
                &state.config,
                AutomaticEmail {
                    template_key: "stalled_order_alert",
                    about: About {
                        order_id: Some(stalled.order_id),
                        ..Default::default()
                    },
                    to: recipient,
                    trigger: triggers::STALLED_ORDER,
                    idempotency_key: key,
                    attachments: Vec::new(),
                    extra_values: Default::default(),
                    sender: None,
                },
            )
            .await;
            match result {
                Ok(Some(_)) => {
                    tx.commit().await?;
                    queued += 1;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(order_id = stalled.order_id, error = %e, "could not queue stalled-order alert")
                }
            }
        }
    }
    Ok(queued)
}

/// The order's contact email, else the partner's. Shared by the progress and pickup
/// mails so the two never disagree about who "the customer" is.
/// The customer address an order writes to: the named contact if it has one, else the
/// partner. Shared with invoicing, which sends to the same person.
pub async fn customer_recipient(
    conn: &mut PgConnection,
    order: &orders::Order,
) -> sqlx::Result<Option<String>> {
    if let Some(cid) = order.contact_id {
        if let Some(email) = contacts::find(&mut *conn, cid).await?.and_then(|c| c.email) {
            return Ok(Some(email));
        }
    }
    Ok(partners::find(&mut *conn, order.partner_id)
        .await?
        .and_then(|p| p.email))
}

/// Queues the customer-facing "your order moved on" email, inside the stage-change transaction.
/// Only when enabled in settings and the order has an address to write to.
pub async fn notify_stage_changed(
    conn: &mut PgConnection,
    state: &AppState,
    order_id: i64,
    stage_row_id: i64,
) -> AppResult<()> {
    let settings = config::settings(&mut *conn).await?;
    if !settings.stage_change_notifications || !settings.automatic_email_enabled {
        return Ok(());
    }
    let Some(order) = orders::find(&mut *conn, order_id).await? else {
        return Ok(());
    };
    let Some(recipient) = customer_recipient(&mut *conn, &order).await? else {
        return Ok(());
    };
    queue_automatic(
        conn,
        &state.config,
        AutomaticEmail {
            template_key: "order_stage_changed",
            about: About {
                order_id: Some(order_id),
                ..Default::default()
            },
            to: &recipient,
            trigger: triggers::STAGE_CHANGED,
            idempotency_key: format!("stage:{stage_row_id}"),
            attachments: Vec::new(),
            extra_values: Default::default(),
            sender: None,
        },
    )
    .await?;
    Ok(())
}

/// Queues the "your car is ready for pickup" email on entering `completed`, in the same
/// transaction. Replaces the generic stage mail for that move: one letter, not two.
/// Idempotency key differs from the stage one, so a retry of the move cannot double-send.
pub async fn notify_ready_for_pickup(
    conn: &mut PgConnection,
    state: &AppState,
    order_id: i64,
    stage_row_id: i64,
) -> AppResult<()> {
    let settings = config::settings(&mut *conn).await?;
    if !settings.stage_change_notifications || !settings.automatic_email_enabled {
        return Ok(());
    }
    let Some(order) = orders::find(&mut *conn, order_id).await? else {
        return Ok(());
    };
    let Some(recipient) = customer_recipient(&mut *conn, &order).await? else {
        return Ok(());
    };
    queue_automatic(
        conn,
        &state.config,
        AutomaticEmail {
            template_key: "order_ready_for_pickup",
            about: About {
                order_id: Some(order_id),
                ..Default::default()
            },
            to: &recipient,
            trigger: triggers::READY_FOR_PICKUP,
            idempotency_key: format!("pickup:{stage_row_id}"),
            attachments: Vec::new(),
            extra_values: Default::default(),
            sender: None,
        },
    )
    .await?;
    Ok(())
}

pub async fn fetch_fx_rates(
    state: &AppState,
    from: NaiveDate,
    to: NaiveDate,
) -> anyhow::Result<usize> {
    let client = MnbClient::new(&state.config.mnb_endpoint)?;
    let currencies: Vec<&str> = Currency::ALL
        .iter()
        .filter(|c| **c != Currency::HUF)
        .map(|c| c.code())
        .collect();
    let mut stored = 0;
    // MNB handles long ranges, but chunk by year to keep responses small during backfills.
    let mut start = from;
    while start <= to {
        let end = (start + TimeDelta::days(365)).min(to);
        let rates = client.exchange_rates(start, end, &currencies).await?;
        let mut tx = state.db.begin().await?;
        for rate in &rates {
            fx::upsert(
                &mut *tx,
                rate.day,
                &rate.currency,
                "HUF",
                rate.huf_per_unit,
                "MNB",
            )
            .await?;
        }
        tx.commit().await?;
        stored += rates.len();
        start = end + TimeDelta::days(1);
    }
    tracing::info!(%from, %to, stored, "MNB exchange rates stored");
    Ok(stored)
}

pub async fn process_image(state: &AppState, image_id: i64) -> anyhow::Result<()> {
    let Some(image) = images::find(&state.db, image_id).await? else {
        return Ok(());
    };
    if image.deleted_at.is_some()
        || (image.processed_at.is_some() && image.processing_error.is_none())
    {
        return Ok(());
    }
    let bytes = state.storage.get_bytes(&image.storage_key).await?;
    if Sha256::digest(&bytes).as_slice() != image.content_hash.as_slice() {
        tracing::error!(image_id, key = %image.storage_key, "stored original does not match its recorded hash");
        images::set_processing_error(
            &state.db,
            image_id,
            "stored original does not match the recorded sha256",
        )
        .await?;
        return Ok(());
    }
    // HEIC: the image library cannot decode HEVC, so an external converter makes a JPEG
    // copy to derive the previews from. The original stays exactly as uploaded.
    let bytes = if image.content_type == "image/heic" || image.content_type == "image/heif" {
        match crate::media::heic::to_jpeg(&state.config.heic_converter, &bytes).await {
            Ok(jpeg) => jpeg,
            Err(e) => {
                images::set_processing_error(&state.db, image_id, &format!("{e:#}")).await?;
                return Ok(());
            }
        }
    } else {
        bytes
    };

    let tz = state.config.business_tz;
    let processed = match tokio::task::spawn_blocking(move || pipeline::process(&bytes, tz)).await?
    {
        Ok(p) => p,
        Err(e) => {
            images::set_processing_error(&state.db, image_id, &e.to_string()).await?;
            return Ok(());
        }
    };
    let hash_hex = hex::encode(&image.content_hash);
    let display_key = image_derived_key(image.order_id, image.category, &hash_hex, "display");
    let thumb_key = image_derived_key(image.order_id, image.category, &hash_hex, "thumb");
    state
        .storage
        .put_bytes(&display_key, processed.display_jpeg, "image/jpeg")
        .await?;
    state
        .storage
        .put_bytes(&thumb_key, processed.thumb_jpeg, "image/jpeg")
        .await?;
    images::set_derived(
        &state.db,
        image_id,
        &images::Derived {
            display_key: &display_key,
            thumb_key: &thumb_key,
            width: i32::try_from(processed.width).unwrap_or(i32::MAX),
            height: i32::try_from(processed.height).unwrap_or(i32::MAX),
            captured_at: processed.captured_at,
        },
    )
    .await?;
    Ok(())
}

/// A thumbnail for a drawing or picture filed as a document (0048): DXF drawn, the preview
/// inside a DWG extracted, image files scaled. Anything that cannot be drawn records why.
pub async fn process_document(state: &AppState, document_id: i64) -> anyhow::Result<()> {
    use crate::domain::media::{ThumbSource, document_thumb_key, thumb_source};
    use crate::repo::documents;

    let Some(doc) = documents::find(&state.db, document_id).await? else {
        return Ok(());
    };
    if doc.deleted_at.is_some() || doc.thumb_key.is_some() {
        return Ok(());
    }
    let Some(source) = thumb_source(&doc.filename) else {
        return Ok(());
    };
    let bytes = state.storage.get_bytes(&doc.storage_key).await?;
    let made = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        match source {
            ThumbSource::Dxf => crate::media::cad::dxf_thumbnail(&bytes).map_err(|e| e.to_string()),
            ThumbSource::Dwg => crate::media::cad::dwg_thumbnail(&bytes).map_err(|e| e.to_string()),
            ThumbSource::Image => {
                let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let mut out = std::io::Cursor::new(Vec::new());
                img.thumbnail(
                    crate::media::cad::THUMB_WIDTH,
                    crate::media::cad::THUMB_HEIGHT,
                )
                .write_to(&mut out, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
                Ok(out.into_inner())
            }
        }
    })
    .await?;
    match made {
        Ok(png) => {
            let key = document_thumb_key(&doc.storage_key);
            state.storage.put_bytes(&key, png, "image/png").await?;
            documents::set_thumb(&state.db, document_id, Some(&key), None).await?;
        }
        Err(reason) => {
            documents::set_thumb(&state.db, document_id, None, Some(&reason)).await?;
        }
    }
    Ok(())
}

/// Has an evidence photo's sha256 stamped by the time-stamping authority (0048). A TSA
/// outage fails the job, which the queue retries with backoff.
pub async fn timestamp_image(state: &AppState, image_id: i64) -> anyhow::Result<()> {
    let Some(tsa) = &state.config.tsa else {
        return Ok(());
    };
    let Some(image) = images::find(&state.db, image_id).await? else {
        return Ok(());
    };
    if image.deleted_at.is_some() || images::has_timestamp(&state.db, image_id).await? {
        return Ok(());
    }
    let stamp = crate::integrations::tsa::stamp(tsa, &image.content_hash).await?;
    images::insert_timestamp(
        &state.db,
        image_id,
        &tsa.url,
        &stamp.response,
        stamp.gen_time,
        &stamp.serial,
    )
    .await?;
    tracing::info!(image_id, gen_time = %stamp.gen_time, "evidence photo timestamped");
    Ok(())
}

/// Daily: queues a stamp for every evidence photo still without one — those from before
/// the authority was configured, and those whose stamping ran out of retries.
pub async fn timestamp_backfill(state: &AppState) -> anyhow::Result<usize> {
    if state.config.tsa.is_none() {
        return Ok(0);
    }
    let ids = images::unstamped_evidence(&state.db, 500).await?;
    for id in &ids {
        crate::repo::jobs::enqueue(
            &state.db,
            "timestamp_image",
            json!({ "image_id": id }),
            None,
            Some(&format!("timestamp_image:{id}")),
        )
        .await?;
    }
    Ok(ids.len())
}
