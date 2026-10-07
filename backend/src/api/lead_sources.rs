//! Where leads come from (0048): the list the source select offers. The office edits it
//! like the lost reasons; `website` and `minicrm` belong to the system and stay as they are.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::role::Capability;
use crate::domain::search_terms::fold;
use crate::error::{AppError, AppResult};
use crate::repo::lead_sources::{self, LeadSource};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_sources, create_source))
        .routes(routes!(update_source))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    #[serde(default)]
    include_archived: bool,
}

#[utoipa::path(
    get, path = "/lead-sources", tag = "leads",
    params(ListQuery),
    responses((status = 200, body = Items<LeadSource>))
)]
async fn list_sources(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<LeadSource>>> {
    Ok(Items::new(
        lead_sources::list(&state.db, q.include_archived).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct SourceBody {
    label: Option<String>,
    position: Option<i32>,
    archived: Option<bool>,
}

/// A key from a label: folded to ASCII, words joined by `_`, a letter first.
fn key_from(label: &str) -> String {
    let words: Vec<String> = label
        .split(|c: char| !c.is_alphanumeric())
        .map(fold)
        .filter(|w| !w.is_empty())
        .collect();
    let mut key = words.join("_");
    if !key.starts_with(|c: char| c.is_ascii_lowercase()) {
        key = format!("s_{key}");
    }
    key.chars().take(40).collect()
}

fn duplicate(e: sqlx::Error) -> AppError {
    let text = e.to_string();
    if text.contains("lead_sources_label_idx") || text.contains("lead_sources_pkey") {
        AppError::conflict("duplicate", "a source with this name already exists")
    } else {
        AppError::Database(e)
    }
}

#[utoipa::path(
    post, path = "/lead-sources", tag = "leads",
    request_body = SourceBody,
    responses((status = 201, body = LeadSource))
)]
async fn create_source(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<SourceBody>,
) -> AppResult<(StatusCode, Json<LeadSource>)> {
    me.require(Capability::EditLeads)?;
    let label = super::required("label", b.label.as_deref().unwrap_or(""))?;
    let mut key = key_from(&label);
    // Two labels can fold to one key; the second gets a number.
    let mut n = 2;
    while lead_sources::find(&state.db, &key).await?.is_some() {
        key = format!("{}_{n}", key_from(&label));
        n += 1;
    }
    let position = match b.position {
        Some(p) => p,
        None => {
            lead_sources::list(&state.db, true)
                .await?
                .iter()
                .filter(|s| !s.is_system)
                .map(|s| s.position)
                .max()
                .unwrap_or(0)
                + 5
        }
    };
    let created = lead_sources::insert(&state.db, &key, &label, position)
        .await
        .map_err(duplicate)?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch, path = "/lead-sources/{key}", tag = "leads",
    params(("key" = String, Path)),
    request_body = SourceBody,
    responses((status = 200, body = LeadSource))
)]
async fn update_source(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(key): ApiPath<String>,
    ApiJson(b): ApiJson<SourceBody>,
) -> AppResult<Json<LeadSource>> {
    me.require(Capability::EditLeads)?;
    let current = lead_sources::find(&state.db, &key)
        .await?
        .ok_or(AppError::NotFound("lead source"))?;
    if current.is_system && b.archived == Some(true) {
        return Err(AppError::validation(
            "a system source cannot be archived; rename it instead",
        ));
    }
    let label = match b.label {
        Some(l) => super::required("label", &l)?,
        None => current.label,
    };
    let updated = lead_sources::update(
        &state.db,
        &key,
        &label,
        b.position.unwrap_or(current.position),
        b.archived.unwrap_or(current.archived_at.is_some()),
    )
    .await
    .map_err(duplicate)?
    .ok_or(AppError::NotFound("lead source"))?;
    Ok(Json(updated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_folded_from_labels() {
        assert_eq!(key_from("Kiállítás, vásár"), "kiallitas_vasar");
        assert_eq!(key_from("2026 Hungexpo"), "s_2026_hungexpo");
        assert_eq!(key_from("Ügyfél ajánlása"), "ugyfel_ajanlasa");
    }
}
