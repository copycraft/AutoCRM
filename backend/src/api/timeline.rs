//! `GET /timeline/{entity}/{id}`: a record's whole history in one list (see
//! repo::timeline). The employee timeline needs HR access like the employee itself.

use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::Value;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::AppResult;
use crate::repo::timeline::{self, TimelineEntity, TimelineEvent};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(history))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct HistoryQuery {
    /// Newest N events. Defaults to 300, at most 1000.
    limit: Option<i64>,
}

/// Fields whose values are ids, and what they point at.
const USER_FIELDS: &[&str] = &["assigned_to", "created_by", "user_id"];
const PARTNER_FIELDS: &[&str] = &["partner_id"];
const CONTACT_FIELDS: &[&str] = &["contact_id"];

fn ids_in(events: &[TimelineEvent], fields: &[&str]) -> Vec<i64> {
    let mut out = Vec::new();
    for e in events {
        let Value::Object(map) = &e.changes else { continue };
        for f in fields {
            if let Some(Value::Array(pair)) = map.get(*f) {
                out.extend(pair.iter().filter_map(Value::as_i64));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[utoipa::path(
    get, path = "/timeline/{entity}/{id}", tag = "timeline",
    params(
        ("entity" = String, Path, description = "lead | order | partner | employee | incoming_invoice"),
        ("id" = i64, Path),
        HistoryQuery,
    ),
    responses((status = 200, body = Items<TimelineEvent>))
)]
async fn history(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((entity, id)): ApiPath<(TimelineEntity, i64)>,
    ApiQuery(q): ApiQuery<HistoryQuery>,
) -> AppResult<Json<Items<TimelineEvent>>> {
    if entity == TimelineEntity::Employee {
        me.require(Capability::AccessHr)?;
    }
    let limit = q.limit.unwrap_or(300).clamp(1, 1000);
    let mut events = timeline::list(&state.db, entity, id, limit).await?;

    let users = ids_in(&events, USER_FIELDS);
    let partners = ids_in(&events, PARTNER_FIELDS);
    let contacts = ids_in(&events, CONTACT_FIELDS);
    if !users.is_empty() || !partners.is_empty() || !contacts.is_empty() {
        let names: HashMap<(String, i64), String> =
            timeline::names(&state.db, &users, &partners, &contacts)
                .await?
                .into_iter()
                .map(|(kind, id, name)| ((kind, id), name))
                .collect();
        for e in &mut events {
            let Value::Object(map) = &mut e.changes else { continue };
            for (field, value) in map.iter_mut() {
                let kind = if USER_FIELDS.contains(&field.as_str()) {
                    "user"
                } else if PARTNER_FIELDS.contains(&field.as_str()) {
                    "partner"
                } else if CONTACT_FIELDS.contains(&field.as_str()) {
                    "contact"
                } else {
                    continue;
                };
                if let Value::Array(pair) = value {
                    for v in pair.iter_mut() {
                        if let Some(id) = v.as_i64()
                            && let Some(name) = names.get(&(kind.to_string(), id))
                        {
                            *v = Value::String(name.clone());
                        }
                    }
                }
            }
        }
    }
    Ok(Items::new(events))
}
