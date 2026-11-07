//! Lead creation and lead → order conversion.

use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::lead_tag;
use crate::domain::media::DocumentKind;
use crate::domain::money::Currency;
use crate::domain::stage::{StageEntity, initial_stage, keys};
use crate::error::{AppError, AppResult};
use crate::repo::documents::{self, Owner};
use crate::repo::leads::{Lead, LeadInput};
use crate::repo::orders::{Order, OrderFields};
use crate::repo::{audit, config, contacts, lead_sources, lead_tags, leads, partners, stages};
use crate::service::auth::AuthUser;
use crate::service::business_today;
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

/// Checks the source against the list (0048). Unchanged on an update: an archived source
/// may stay on the leads that have it. A system source (`website`, `minicrm`) is never
/// picked by hand. Anything else that is not a key: a label typed by an older client
/// becomes its key, and free text becomes `other` with the text kept as the detail.
pub async fn normalize_source(
    conn: &mut PgConnection,
    current: Option<&str>,
    input: &mut LeadInput,
) -> AppResult<()> {
    let Some(raw) = input.source.clone() else {
        return Ok(());
    };
    if Some(raw.as_str()) == current {
        return Ok(());
    }
    match lead_sources::find(&mut *conn, &raw).await? {
        Some(s) if s.is_system => Err(AppError::validation(format!(
            "source '{}' is set by the system and cannot be chosen",
            s.key
        ))),
        Some(s) if s.archived_at.is_some() => Err(AppError::validation(format!(
            "source '{}' is archived",
            s.key
        ))),
        Some(_) => Ok(()),
        None => {
            let matched = lead_sources::matching(&mut *conn, &raw).await?;
            let usable = match &matched {
                Some(key) => lead_sources::find(&mut *conn, key)
                    .await?
                    .is_some_and(|s| !s.is_system),
                None => false,
            };
            if usable {
                input.source = matched;
            } else {
                if input.source_detail.is_none() {
                    input.source_detail = Some(raw);
                }
                input.source = Some("other".into());
            }
            Ok(())
        }
    }
}

/// How far back a published MNB rate still counts for a quote (MNB skips weekends and
/// holidays), the same window the order valuation uses.
const QUOTE_FX_WINDOW_DAYS: i64 = 10;

/// Freezes an EUR quote's MNB rate as of `today`, or clears it when the quote is not in
/// EUR. Without a rate in the window the snapshot is cleared rather than guessed.
pub async fn refresh_quote_fx(
    conn: &mut PgConnection,
    mut lead: Lead,
    today: NaiveDate,
) -> AppResult<Lead> {
    let wanted = if lead.currency.as_deref() == Some("EUR") && lead.quoted_value_minor.is_some() {
        crate::repo::fx::rate_on_or_before(&mut *conn, "EUR", today)
            .await?
            .filter(|r| (today - r.day).num_days() <= QUOTE_FX_WINDOW_DAYS)
            .map(|r| (r.rate, r.day))
    } else {
        None
    };
    let current = lead.quote_fx_rate.zip(lead.quote_fx_day);
    if wanted != current {
        leads::set_quote_fx(&mut *conn, lead.id, wanted).await?;
        lead.quote_fx_rate = wanted.map(|w| w.0);
        lead.quote_fx_day = wanted.map(|w| w.1);
    }
    Ok(lead)
}

/// A quotation the sales manager made and sent outside the CRM, recorded on the lead: the
/// PDF already filed as one of the lead's documents, the price and validity it states, and
/// the follow-up letters counted from the day it went out. The CRM never sends the
/// quotation itself: the customer gets it from a person's own mailbox.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct QuotationRecord {
    /// The quotation file: a document of this lead. Filed as `quote` if it was not already.
    pub document_id: i64,
    pub quoted_value_minor: Option<i64>,
    pub currency: Option<Currency>,
    pub quote_valid_until: Option<NaiveDate>,
    /// The day it went out. Today when omitted; never in the future.
    pub sent_on: Option<NaiveDate>,
    /// Follow-up letters to schedule (ids of `/followup-steps`), counted from `sent_on`.
    /// Omitted: every active default step. `[]`: none. Any still waiting from an earlier
    /// quotation of the lead are cancelled either way.
    #[serde(default)]
    pub followup_step_ids: Option<Vec<i64>>,
}

/// Records a quotation (see [`QuotationRecord`]). Returns the lead as it now stands.
pub async fn record_quotation(
    state: &AppState,
    user: &AuthUser,
    lead_id: i64,
    req: &QuotationRecord,
) -> AppResult<Lead> {
    if req.quoted_value_minor.is_some_and(|v| v < 0) {
        return Err(AppError::validation(
            "quoted_value_minor must not be negative",
        ));
    }
    if req.quoted_value_minor.is_some() && req.currency.is_none() {
        return Err(AppError::validation("a quoted value needs a currency"));
    }
    let today = business_today(state.config.business_tz);
    let sent_on = req.sent_on.unwrap_or(today);
    if sent_on > today {
        return Err(AppError::validation("sent_on cannot be in the future"));
    }

    let mut tx = state.db.begin().await?;
    let lead = leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let document = documents::find(&mut *tx, req.document_id)
        .await?
        .filter(|d| Owner::Lead(lead_id).owns(d) && d.deleted_at.is_none())
        .ok_or_else(|| AppError::validation("the quotation must be a document of this lead"))?;
    if document.kind != DocumentKind::Quote {
        documents::set_kind(&mut *tx, document.id, DocumentKind::Quote).await?;
    }

    let currency = req.currency.map(|c| c.code());
    leads::set_quote(
        &mut *tx,
        lead_id,
        req.quoted_value_minor,
        currency,
        req.quote_valid_until,
    )
    .await?;

    // Follow-ups go to the lead's contact, else the partner's mailbox. Scheduling letters
    // that can only be skipped later would look like they went out, so refuse up front.
    let wants_followups = req
        .followup_step_ids
        .as_ref()
        .is_none_or(|ids| !ids.is_empty());
    if wants_followups && !has_followup_address(&mut tx, &lead).await? {
        return Err(AppError::validation(format!(
            "lead #{lead_id} has no email address for the follow-ups: add a contact email \
             or untick the follow-ups"
        )));
    }
    // Sent today: count from now. Sent earlier: from that morning, as the office would.
    let sent_at = if sent_on == today {
        Utc::now()
    } else {
        sent_on
            .and_hms_opt(9, 0, 0)
            .and_then(|t| t.and_local_timezone(state.config.business_tz).earliest())
            .map(|t| t.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    };
    let scheduled = crate::service::followups::schedule_for_quotation(
        &mut tx,
        lead_id,
        req.followup_step_ids.as_deref(),
        sent_at,
        user.user_id,
    )
    .await?;

    let lead = leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    // Quoting is the moment the rate counts, as it was when the CRM sent the letter.
    let lead = refresh_quote_fx(&mut tx, lead, sent_on).await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead_id,
        "quote_recorded",
        json!({
            "document_id": document.id,
            "filename": document.filename,
            "sent_on": sent_on,
            "quoted_value_minor": lead.quoted_value_minor,
            "currency": lead.currency,
            "quote_valid_until": lead.quote_valid_until,
            "followups": scheduled,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(lead)
}

async fn has_followup_address(conn: &mut PgConnection, lead: &Lead) -> AppResult<bool> {
    if lead
        .contact_email
        .as_deref()
        .and_then(normalize_address)
        .is_some()
    {
        return Ok(true);
    }
    let Some(partner_id) = lead.partner_id else {
        return Ok(false);
    };
    Ok(partners::find(&mut *conn, partner_id)
        .await?
        .and_then(|p| p.email)
        .and_then(|e| normalize_address(&e))
        .is_some())
}

/// `tag_ids` are the tags the person picked; tags claiming a domain the source names are
/// added on top.
pub async fn create(
    db: &PgPool,
    user: &AuthUser,
    mut input: LeadInput,
    tag_ids: &[i64],
    today: NaiveDate,
) -> AppResult<Lead> {
    let mut tx = db.begin().await?;
    normalize_source(&mut tx, None, &mut input).await?;
    check_relations(&mut tx, input.partner_id, input.contact_id).await?;
    check_new_tags(&mut tx, tag_ids).await?;
    let lead = leads::insert(&mut *tx, &input, user.user_id).await?;
    let lead = refresh_quote_fx(&mut tx, lead, today).await?;
    let definitions = config::stage_definitions(&mut *tx, StageEntity::Lead).await?;
    let initial = initial_stage(&definitions)
        .ok_or_else(|| AppError::internal("no active initial lead stage is configured"))?;
    stages::insert_lead_stage(&mut *tx, lead.id, &initial.key, Some(user.user_id), None).await?;
    for &tag_id in tag_ids {
        lead_tags::link(&mut *tx, lead.id, tag_id, None, Some(user.user_id)).await?;
    }
    // A source detail typed as a domain (hutoautok.hu) is recognised like a website lead's.
    auto_tag(
        &mut tx,
        lead.id,
        &lead_tag::hosts_of([input.source_detail.as_deref(), input.source.as_deref()]),
    )
    .await?;
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
    /// Questionnaire answers given in the convert dialog, laid over the lead's own (0065).
    /// A null answer clears the lead's.
    pub answers: Option<crate::domain::questionnaire::Answers>,
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

    // The questionnaire (0065): what the lead was asked, with what the dialog added, has to
    // answer every required question asked before there is an order. The lead keeps the
    // completed answers too, so a second order from the same enquiry starts from them.
    let (lead_type, lead_answers) = crate::repo::questionnaire::lead_spec(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let project_type_id = conversion.fields.project_type_id.or(lead_type);
    conversion.fields.project_type_id = project_type_id;
    let mut given = lead_answers.clone();
    if let Some(answers) = conversion.answers.take() {
        given.extend(answers);
    }
    let qn = crate::repo::questionnaire::load(&mut tx).await?;
    let answers =
        crate::service::questionnaire::clean(&qn, project_type_id, &given, &lead_answers)?;
    crate::service::questionnaire::require_complete(&qn, project_type_id, &answers)?;
    crate::repo::questionnaire::set_lead_spec(&mut *tx, lead_id, project_type_id, &answers).await?;

    let order = create_in_tx(
        &mut tx,
        user.user_id,
        today,
        conversion.fields,
        conversion.items,
        Some(lead_id),
    )
    .await?;
    crate::repo::questionnaire::set_order_answers(&mut *tx, order.id, &answers).await?;

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
    // The enquiry's quotation and files join the project's folder on the share.
    crate::repo::jobs::enqueue(
        &mut *tx,
        crate::jobs::kinds::STORAGE_TIDY,
        json!({ "lead_id": lead_id }),
        None,
        Some(&format!("storage_tidy:lead:{lead_id}:{}", order.id)),
    )
    .await?;
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
