//! Lead creation and lead → order conversion.

use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;

use crate::domain::stage::{StageEntity, initial_stage, keys};
use crate::error::{AppError, AppResult};
use crate::repo::leads::{Lead, LeadInput};
use crate::repo::orders::{Order, OrderFields};
use crate::domain::lead_tag;
use crate::repo::{audit, config, contacts, lead_tags, leads, partners, stages};
use crate::service::auth::AuthUser;
use crate::service::orders::{NewItem, create_in_tx};
use sqlx::PgConnection;

/// The relations create enforces, checked against the merged pair on update too (N1):
/// the partner exists and is not archived (an archived partner takes no new work,
/// like on orders), the contact exists, and it belongs to the lead's partner.
pub async fn check_relations(
    conn: &mut PgConnection,
    partner_id: Option<i64>,
    contact_id: Option<i64>,
) -> AppResult<()> {
    if let Some(partner_id) = partner_id {
        let partner = partners::find(&mut *conn, partner_id)
            .await?
            .ok_or_else(|| AppError::validation("partner does not exist"))?;
        if partner.archived_at.is_some() {
            return Err(AppError::validation("partner is archived"));
        }
    }
    if let Some(contact_id) = contact_id {
        let contact = contacts::find(&mut *conn, contact_id)
            .await?
            .ok_or_else(|| AppError::validation("contact does not exist"))?;
        if Some(contact.partner_id) != partner_id {
            return Err(AppError::validation(
                "contact belongs to a different partner",
            ));
        }
    }
    Ok(())
}

/// `tag_ids` are the tags the person picked; tags claiming a domain the source names are
/// added on top.
pub async fn create(
    db: &PgPool,
    user: &AuthUser,
    input: LeadInput,
    tag_ids: &[i64],
) -> AppResult<Lead> {
    let mut tx = db.begin().await?;
    check_relations(&mut tx, input.partner_id, input.contact_id).await?;
    check_new_tags(&mut tx, tag_ids).await?;
    let lead = leads::insert(&mut *tx, &input, user.user_id).await?;
    let definitions = config::stage_definitions(&mut *tx, StageEntity::Lead).await?;
    let initial = initial_stage(&definitions)
        .ok_or_else(|| AppError::internal("no active initial lead stage is configured"))?;
    stages::insert_lead_stage(&mut *tx, lead.id, &initial.key, Some(user.user_id), None).await?;
    for &tag_id in tag_ids {
        lead_tags::link(&mut *tx, lead.id, tag_id, None, Some(user.user_id)).await?;
    }
    // A source typed as a domain (hutoautok.hu) is recognised like a website lead's.
    auto_tag(&mut tx, lead.id, &lead_tag::hosts_of([input.source.as_deref()])).await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead.id,
        "create",
        json!({ "title": lead.title }),
    )
    .await?;
    tx.commit().await?;
    Ok(lead)
}

/// A website enquiry becomes a lead in the first stage, with no staff user behind it.
/// Unassigned: the office picks it up from the list. `hosts` is every host known about the
/// visit, strongest evidence first; tags claiming one of them are attached.
pub async fn create_from_website(
    db: &PgPool,
    input: LeadInput,
    attribution: &crate::repo::attribution::Attribution,
    hosts: &[String],
) -> AppResult<Lead> {
    let mut tx = db.begin().await?;
    let lead = leads::insert_by(&mut *tx, &input, None).await?;
    crate::repo::attribution::insert(&mut *tx, lead.id, attribution).await?;
    auto_tag(&mut tx, lead.id, hosts).await?;
    let definitions = config::stage_definitions(&mut *tx, StageEntity::Lead).await?;
    let initial = initial_stage(&definitions)
        .ok_or_else(|| AppError::internal("no active initial lead stage is configured"))?;
    stages::insert_lead_stage(&mut *tx, lead.id, &initial.key, None, None).await?;
    audit::record(
        &mut *tx,
        None,
        "lead",
        lead.id,
        "create",
        json!({ "title": lead.title, "source": "website" }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(lead_id = lead.id, "website lead filed");
    Ok(lead)
}

/// Attaches every live tag claiming a domain one of `hosts` falls under, noting the domain.
pub async fn auto_tag(conn: &mut PgConnection, lead_id: i64, hosts: &[String]) -> AppResult<()> {
    if hosts.is_empty() {
        return Ok(());
    }
    for (tag_id, domains) in lead_tags::with_domains(&mut *conn).await? {
        if let Some(domain) = lead_tag::first_match(hosts, &domains) {
            lead_tags::link(&mut *conn, lead_id, tag_id, Some(domain), None).await?;
        }
    }
    Ok(())
}

/// Tags being put on a lead must exist and not be archived.
pub async fn check_new_tags(conn: &mut PgConnection, tag_ids: &[i64]) -> AppResult<()> {
    let mut ids = tag_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() && lead_tags::count_live(&mut *conn, &ids).await? != ids.len() as i64 {
        return Err(AppError::validation("a tag does not exist or is archived"));
    }
    Ok(())
}

/// Replaces the lead's tags with `tag_ids`. Kept tags keep how they got there (the
/// recognised domain); an archived tag the lead already has may stay.
pub async fn set_tags(
    db: &PgPool,
    user: &AuthUser,
    lead_id: i64,
    tag_ids: &[i64],
) -> AppResult<Vec<lead_tags::LeadTagRef>> {
    let mut tx = db.begin().await?;
    leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let before = lead_tags::for_lead(&mut *tx, lead_id).await?;
    let mut ids = tag_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    let new: Vec<i64> = ids
        .iter()
        .copied()
        .filter(|id| !before.iter().any(|t| t.id == *id))
        .collect();
    check_new_tags(&mut tx, &new).await?;
    lead_tags::unlink_others(&mut *tx, lead_id, &ids).await?;
    for &tag_id in &new {
        lead_tags::link(&mut *tx, lead_id, tag_id, None, Some(user.user_id)).await?;
    }
    let after = lead_tags::for_lead(&mut *tx, lead_id).await?;
    let labels = |tags: &[lead_tags::LeadTagRef]| {
        tags.iter()
            .map(|t| format!("{}/{}", t.market, t.label))
            .collect::<Vec<_>>()
    };
    if labels(&before) != labels(&after) {
        audit::record(
            &mut *tx,
            Some(user.user_id),
            "lead",
            lead_id,
            "update",
            json!({ "tags": { "from": labels(&before), "to": labels(&after) } }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(after)
}

/// What the office fills in when a lead becomes an order. Partner may be omitted if the
/// lead already has one.
pub struct Conversion {
    pub partner_id: Option<i64>,
    pub fields: OrderFields,
    pub items: Vec<NewItem>,
}

pub async fn convert(
    db: &PgPool,
    user: &AuthUser,
    lead_id: i64,
    today: NaiveDate,
    mut conversion: Conversion,
) -> AppResult<Order> {
    let mut tx = db.begin().await?;
    let lead = leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    // V2.7: one enquiry for three identical Sprinters becomes three orders, and each keeps
    // its origin. Refusing the second conversion left the other two as orphans with no
    // record of where they came from.

    let partner_id = conversion.partner_id.or(lead.partner_id).ok_or_else(|| {
        AppError::validation("choose or create a partner before converting the lead")
    })?;
    conversion.fields.partner_id = partner_id;
    if conversion.fields.contact_id.is_none() && lead.partner_id == Some(partner_id) {
        conversion.fields.contact_id = lead.contact_id;
    }

    let order = create_in_tx(
        &mut tx,
        user.user_id,
        today,
        conversion.fields,
        conversion.items,
        Some(lead_id),
    )
    .await?;

    if lead.partner_id.is_none() {
        leads::set_partner(&mut *tx, lead_id, partner_id).await?;
    }
    let note = format!("Megrendelés: {}", order.number);
    stages::insert_lead_stage(
        &mut *tx,
        lead_id,
        keys::LEAD_WON,
        Some(user.user_id),
        Some(&note),
    )
    .await?;
    crate::repo::followups::cancel_for_lead(&mut *tx, lead_id, "a lead megrendelés lett").await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead_id,
        "convert",
        json!({ "order_id": order.id, "order_number": order.number }),
    )
    .await?;
    tx.commit().await?;

    tracing::info!(lead_id, order_id = order.id, number = %order.number, "lead converted");
    Ok(order)
}
