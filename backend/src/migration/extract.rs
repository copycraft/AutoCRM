//! Phase M1: pull everything from MiniCRM and write it to disk, unmodified.
//!
//! Never transform here. The transform will be re-run many times; the API should be hit
//! once. Every file is written atomically and skipped if present, so an interrupted run
//! resumes where it stopped.
//!
//! The endpoint set follows MiniCRM's Integrations Manual (2025-02-03). What it says that
//! shapes this file:
//!
//! - Searches return 100 results a page, `Page` counted from 0, the total in `Count`.
//! - `Project?CategoryId=…` lists only cards outside the trash; the trash is its own search
//!   (`&Deleted=1`). Trashed cards are not migrated.
//! - `ToDoList/{project}` filters by `Status` (Open / Closed / All); asked for `All`, or the
//!   closed ones — most of a project's history — could be left behind. A list entry is a
//!   summary: the comments under a to-do (`Notes`), its attachments and its creation time
//!   are on `ToDo/{id}` only.
//! - `AddressList/{contact}` gives each address as a one-line label; the postal code, city
//!   and street are on `Address/{id}`.
//! - `ProjectHistory/{project}?Type=StatusHistory` is the card's status path with times
//!   (not available for trashed cards).
//! - `ListProjectConnection?FromProjectId=…` is the cards linked to a card: an enquiry and
//!   the order it became.
//! - `Project?…&UpdatedSince=…` and `Contact?UpdatedSince=…` (Hungarian time) list what
//!   changed, which is what `--since` uses: the full run can happen weeks before cutover,
//!   and on the day only the changes are fetched again.
//!
//! Anything the account doesn't expose (404) is logged and skipped rather than aborting the
//! run — compare the summary with what the MiniCRM UI shows before trusting it. A refused
//! API key is also a 404 at MiniCRM; the client tells the two apart and stops.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::client::MiniCrmClient;

/// What to download besides the cards and their contacts.
#[derive(Debug, Clone)]
pub struct ExtractOptions {
    /// Each project's to-do list (`raw/todos`).
    pub todos: bool,
    /// Each to-do in full: comments, attachments, creation time (`raw/todo-details`).
    pub todo_details: bool,
    /// Each card's status path (`raw/status-history`).
    pub status_history: bool,
    /// The cards linked to each card (`raw/connections`).
    pub connections: bool,
    /// Every person filed under a company the cards reach, not only the cards' own
    /// contacts.
    pub business_contacts: bool,
    /// Every contact in the account, also those with no card at all.
    pub all_contacts: bool,
    /// Refresh: fetch again what changed since then (`yyyy-mm-dd` or
    /// `yyyy-mm-dd hh:mm:ss`, Hungarian time), on top of an earlier full run.
    pub since: Option<String>,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        ExtractOptions {
            todos: true,
            todo_details: true,
            status_history: true,
            connections: true,
            business_contacts: true,
            all_contacts: false,
            since: None,
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ExtractSummary {
    pub categories: usize,
    pub project_ids: usize,
    pub projects_fetched: usize,
    pub contacts_fetched: usize,
    pub todo_details: usize,
    pub status_histories: usize,
    pub connections: usize,
    pub addresses: usize,
    /// In a `--since` run: the cards and contacts fetched again because they changed.
    pub refreshed_projects: usize,
    pub refreshed_contacts: usize,
    pub missing: Vec<String>,
}

pub struct Extractor<'a> {
    client: &'a MiniCrmClient,
    out: PathBuf,
    options: ExtractOptions,
}

/// `2026-11-06 10:30:00` as MiniCRM wants it in a query string.
fn query_time(since: &str) -> String {
    since.trim().replace(' ', "+")
}

impl<'a> Extractor<'a> {
    pub fn new(client: &'a MiniCrmClient, out: &Path, options: ExtractOptions) -> Self {
        Extractor {
            client,
            out: out.to_path_buf(),
            options,
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.out.join(rel)
    }

    /// Fetch `api_path` into `rel` unless already on disk — or, with `fresh`, always,
    /// replacing what is there. Returns the JSON either way.
    async fn fetch(
        &self,
        api_path: &str,
        rel: &str,
        fresh: bool,
        summary: &mut ExtractSummary,
    ) -> anyhow::Result<Option<Value>> {
        let file = self.path(rel);
        if !fresh && file.exists() {
            let text = tokio::fs::read_to_string(&file).await?;
            return Ok(Some(
                serde_json::from_str(&text)
                    .with_context(|| format!("corrupt {}", file.display()))?,
            ));
        }
        match self.client.get_json(api_path).await? {
            Some(value) => {
                write_atomic(&file, &serde_json::to_vec_pretty(&value)?).await?;
                Ok(Some(value))
            }
            None => {
                summary.missing.push(api_path.to_string());
                Ok(None)
            }
        }
    }

    /// A search's ids, kept on disk under `rel` so a rerun does not ask again (unless
    /// `fresh`).
    async fn search(&self, api_path: &str, rel: &str, fresh: bool) -> anyhow::Result<Vec<i64>> {
        let file = self.path(rel);
        if !fresh && file.exists() {
            let text = tokio::fs::read_to_string(&file).await?;
            return serde_json::from_str(&text)
                .with_context(|| format!("corrupt {}", file.display()));
        }
        let ids = self.client.search_ids(api_path).await?;
        write_atomic(&file, &serde_json::to_vec(&ids)?).await?;
        Ok(ids)
    }

    pub async fn run(&self) -> anyhow::Result<ExtractSummary> {
        let mut summary = ExtractSummary::default();
        let since = self.options.since.as_deref().map(query_time);

        // The first request doubles as the credential check: every account has categories,
        // so a 404 here means the API is not reachable with these details at all.
        let Some(categories) = self
            .fetch(
                "Api/R3/Category",
                "categories.json",
                since.is_some(),
                &mut summary,
            )
            .await?
        else {
            bail!(
                "MiniCRM answered 404 for Api/R3/Category: check MINICRM_BASE_URL, the SystemId \
                 and that the plan includes the REST API"
            );
        };
        let category_ids = object_or_array_ids(&categories);
        summary.categories = category_ids.len();
        tracing::info!(count = category_ids.len(), "categories");

        let mut project_ids = BTreeSet::new();
        let mut changed_projects = BTreeSet::new();
        for category in &category_ids {
            self.fetch(
                &format!("Api/R3/Schema/Project/{category}"),
                &format!("schema/project-{category}.json"),
                since.is_some(),
                &mut summary,
            )
            .await?;
            let mut page = 0;
            loop {
                let rel = format!("project-lists/{category}-{page:05}.json");
                let Some(list) = self
                    .fetch(
                        &format!("Api/R3/Project?CategoryId={category}&Page={page}"),
                        &rel,
                        // A refresh lists again: new cards appear only in a fresh list.
                        since.is_some(),
                        &mut summary,
                    )
                    .await?
                else {
                    break;
                };
                let ids = object_or_array_ids(list.get("Results").unwrap_or(&Value::Null));
                if ids.is_empty() {
                    break;
                }
                project_ids.extend(ids);
                let total = list.get("Count").and_then(as_i64).unwrap_or(0) as usize;
                page += 1;
                if page * 100 >= total {
                    break;
                }
            }
            if let Some(since) = &since {
                // Changed since the full run, and moved to the trash since (their details
                // are fetched again so the load sees `Deleted: 1` and skips them).
                changed_projects.extend(
                    self.client
                        .search_ids(&format!(
                            "Api/R3/Project?CategoryId={category}&UpdatedSince={since}"
                        ))
                        .await?,
                );
                let trashed = self
                    .client
                    .search_ids(&format!(
                        "Api/R3/Project?CategoryId={category}&Deleted=1&UpdatedSince={since}"
                    ))
                    .await?;
                changed_projects.extend(trashed.iter().copied());
                project_ids.extend(trashed);
            }
            tracing::info!(
                category,
                projects_so_far = project_ids.len(),
                "project list done"
            );
        }
        summary.project_ids = project_ids.len();
        summary.refreshed_projects = changed_projects.len();

        let mut contact_ids = BTreeSet::new();
        for (i, id) in project_ids.iter().enumerate() {
            let fresh = changed_projects.contains(id);
            if let Some(project) = self
                .fetch(
                    &format!("Api/R3/Project/{id}"),
                    &format!("projects/{id}.json"),
                    fresh,
                    &mut summary,
                )
                .await?
            {
                summary.projects_fetched += 1;
                if let Some(cid) = project.get("ContactId").and_then(as_i64) {
                    contact_ids.insert(cid);
                }
                if let Some(bid) = project
                    .get("BusinessId")
                    .and_then(as_i64)
                    .filter(|b| *b > 0)
                {
                    contact_ids.insert(bid);
                }
                let trashed = project
                    .get("Deleted")
                    .and_then(as_i64)
                    .is_some_and(|d| d != 0);
                self.project_extras(*id, trashed, fresh, &mut summary)
                    .await?;
            }
            if i % 100 == 0 {
                tracing::info!(done = i, total = project_ids.len(), "projects");
            }
        }

        // Contacts: the cards' own, the people of the companies they reach, optionally
        // everyone, and in a refresh whoever changed.
        let mut changed_contacts = BTreeSet::new();
        if self.options.all_contacts {
            contact_ids.extend(
                self.search(
                    "Api/R3/Contact?UpdatedSince=2000-01-01",
                    "contact-index.json",
                    since.is_some(),
                )
                .await?,
            );
        }
        if let Some(since) = &since {
            let changed = self
                .client
                .search_ids(&format!("Api/R3/Contact?UpdatedSince={since}"))
                .await?;
            // Only those the migration already covers, or would cover anyway: a refresh
            // must not quietly widen the run to the whole address book.
            for cid in changed {
                if contact_ids.contains(&cid) || self.path(&format!("contacts/{cid}.json")).exists()
                {
                    changed_contacts.insert(cid);
                    contact_ids.insert(cid);
                }
            }
        }
        summary.refreshed_contacts = changed_contacts.len();

        let mut queue: Vec<i64> = contact_ids.iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(cid) = queue.pop() {
            if !seen.insert(cid) {
                continue;
            }
            let fresh = changed_contacts.contains(&cid);
            let Some(contact) = self
                .fetch(
                    &format!("Api/R3/Contact/{cid}"),
                    &format!("contacts/{cid}.json"),
                    fresh,
                    &mut summary,
                )
                .await?
            else {
                continue;
            };
            summary.contacts_fetched += 1;
            if let Some(business) = contact
                .get("BusinessId")
                .and_then(as_i64)
                .filter(|b| *b > 0)
            {
                queue.push(business);
            }
            let is_business = contact.get("Type").and_then(Value::as_str) == Some("Business");
            if is_business && self.options.business_contacts {
                // `MainContactId` with a company's id lists the people filed under it.
                queue.extend(
                    self.search(
                        &format!("Api/R3/Contact?MainContactId={cid}"),
                        &format!("business-contacts/{cid}.json"),
                        fresh,
                    )
                    .await?,
                );
            }
            self.addresses(cid, fresh, &mut summary).await?;
        }

        write_atomic(
            &self.path(if since.is_some() {
                "extract-refresh-summary.json"
            } else {
                "extract-summary.json"
            }),
            &serde_json::to_vec_pretty(&summary)?,
        )
        .await?;
        Ok(summary)
    }

    /// A card's to-dos (and each in full), status path and links to other cards.
    async fn project_extras(
        &self,
        id: i64,
        trashed: bool,
        fresh: bool,
        summary: &mut ExtractSummary,
    ) -> anyhow::Result<()> {
        if self.options.todos
            && let Some(list) = self
                .fetch(
                    &format!("Api/R3/ToDoList/{id}?Status=All"),
                    &format!("todos/{id}.json"),
                    fresh,
                    summary,
                )
                .await?
            && self.options.todo_details
        {
            let body = list.get("Results").unwrap_or(&list);
            for tid in object_or_array_ids(body) {
                if self
                    .fetch(
                        &format!("Api/R3/ToDo/{tid}"),
                        &format!("todo-details/{tid}.json"),
                        fresh,
                        summary,
                    )
                    .await?
                    .is_some()
                {
                    summary.todo_details += 1;
                }
            }
        }
        // MiniCRM keeps no status history for trashed cards.
        if self.options.status_history
            && !trashed
            && self
                .fetch(
                    &format!("Api/R3/ProjectHistory/{id}?Type=StatusHistory"),
                    &format!("status-history/{id}.json"),
                    fresh,
                    summary,
                )
                .await?
                .is_some()
        {
            summary.status_histories += 1;
        }
        if self.options.connections
            && self
                .fetch(
                    &format!("Api/R3/ListProjectConnection?FromProjectId={id}"),
                    &format!("connections/{id}.json"),
                    fresh,
                    summary,
                )
                .await?
                .is_some()
        {
            summary.connections += 1;
        }
        Ok(())
    }

    /// The contact's addresses in full, as `raw/addresses/{contact}.json` =
    /// `{"Count": n, "Results": {"<address id>": {…Address/{id}…}}}`. The list endpoint
    /// alone gives one-line labels with no postal code or city.
    async fn addresses(
        &self,
        cid: i64,
        fresh: bool,
        summary: &mut ExtractSummary,
    ) -> anyhow::Result<()> {
        let combined = format!("addresses/{cid}.json");
        if !fresh && self.path(&combined).exists() {
            return Ok(());
        }
        let Some(list) = self
            .fetch(
                &format!("Api/R3/AddressList/{cid}"),
                &format!("address-lists/{cid}.json"),
                fresh,
                summary,
            )
            .await?
        else {
            return Ok(());
        };
        let mut results = Map::new();
        for aid in object_or_array_ids(list.get("Results").unwrap_or(&Value::Null)) {
            if let Some(address) = self
                .fetch(
                    &format!("Api/R3/Address/{aid}"),
                    &format!("address-details/{aid}.json"),
                    fresh,
                    summary,
                )
                .await?
            {
                summary.addresses += 1;
                results.insert(aid.to_string(), address);
            }
        }
        let body = json!({ "Count": results.len(), "Results": results });
        write_atomic(&self.path(&combined), &serde_json::to_vec_pretty(&body)?).await
    }
}

pub fn as_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// MiniCRM returns collections either as `{"123": {...}}` or `[{"Id": 123, ...}]`.
pub fn object_or_array_ids(v: &Value) -> Vec<i64> {
    match v {
        Value::Object(map) => map.keys().filter_map(|k| k.parse().ok()).collect(),
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i.get("Id").and_then(as_i64))
            .collect(),
        _ => Vec::new(),
    }
}

pub async fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ids_from_both_collection_shapes() {
        assert_eq!(
            object_or_array_ids(&json!({"12": "Megrendelések", "15": "Érdeklődők"})),
            vec![12, 15]
        );
        assert_eq!(
            object_or_array_ids(&json!([{"Id": 3}, {"Id": "4"}, {"Name": "x"}])),
            vec![3, 4]
        );
        assert!(object_or_array_ids(&json!(null)).is_empty());
    }

    #[test]
    fn address_labels_and_todo_arrays_give_their_ids() {
        // AddressList: id → one-line label. ToDoList: an array of summaries.
        assert_eq!(
            object_or_array_ids(&json!({"307": "NoName (John Doe)", "310": "Office"})),
            vec![307, 310]
        );
        assert_eq!(
            object_or_array_ids(&json!([{"Id": "1111", "Status": "Open"}])),
            vec![1111]
        );
    }

    #[test]
    fn refresh_times_go_into_the_query_with_a_plus() {
        assert_eq!(query_time("2026-11-06 10:30:00"), "2026-11-06+10:30:00");
        assert_eq!(query_time(" 2026-11-06 "), "2026-11-06");
    }
}
