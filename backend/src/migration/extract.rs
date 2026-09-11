//! Phase M1: pull everything from MiniCRM and write it to disk, unmodified.
//!
//! Never transform here. The transform will be re-run many times; the API should be hit
//! once. Every file is written atomically and skipped if present, so an interrupted run
//! resumes where it stopped.
//!
//! The endpoint set follows the MiniCRM R3 API as documented. Anything the account doesn't
//! expose (404) is logged and skipped rather than aborting the run — compare the summary
//! with what the MiniCRM UI shows before trusting it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Serialize;
use serde_json::Value;

use super::client::MiniCrmClient;

#[derive(Debug, Default, Serialize)]
pub struct ExtractSummary {
    pub categories: usize,
    pub project_ids: usize,
    pub projects_fetched: usize,
    pub contacts_fetched: usize,
    pub missing: Vec<String>,
}

pub struct Extractor<'a> {
    client: &'a MiniCrmClient,
    out: PathBuf,
    include_todos: bool,
}

impl<'a> Extractor<'a> {
    pub fn new(client: &'a MiniCrmClient, out: &Path, include_todos: bool) -> Self {
        Extractor {
            client,
            out: out.to_path_buf(),
            include_todos,
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.out.join(rel)
    }

    /// Fetch `api_path` into `rel` unless already on disk. Returns the JSON either way.
    async fn fetch(
        &self,
        api_path: &str,
        rel: &str,
        summary: &mut ExtractSummary,
    ) -> anyhow::Result<Option<Value>> {
        let file = self.path(rel);
        if file.exists() {
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

    pub async fn run(&self) -> anyhow::Result<ExtractSummary> {
        let mut summary = ExtractSummary::default();

        let categories = self
            .fetch("Api/R3/Category", "categories.json", &mut summary)
            .await?
            .unwrap_or(Value::Null);
        let category_ids = object_or_array_ids(&categories);
        summary.categories = category_ids.len();
        tracing::info!(count = category_ids.len(), "categories");

        let mut project_ids = BTreeSet::new();
        for category in &category_ids {
            self.fetch(
                &format!("Api/R3/Schema/Project/{category}"),
                &format!("schema/project-{category}.json"),
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
                let total = list.get("Count").and_then(Value::as_u64).unwrap_or(0) as usize;
                page += 1;
                // R3 pages hold 100 results.
                if page * 100 >= total {
                    break;
                }
            }
            tracing::info!(
                category,
                projects_so_far = project_ids.len(),
                "project list done"
            );
        }
        summary.project_ids = project_ids.len();

        let mut contact_ids = BTreeSet::new();
        for (i, id) in project_ids.iter().enumerate() {
            if let Some(project) = self
                .fetch(
                    &format!("Api/R3/Project/{id}"),
                    &format!("projects/{id}.json"),
                    &mut summary,
                )
                .await?
            {
                summary.projects_fetched += 1;
                if let Some(cid) = project.get("ContactId").and_then(as_i64) {
                    contact_ids.insert(cid);
                }
                if self.include_todos {
                    self.fetch(
                        &format!("Api/R3/ToDoList/{id}"),
                        &format!("todos/{id}.json"),
                        &mut summary,
                    )
                    .await?;
                }
            }
            if i % 100 == 0 {
                tracing::info!(done = i, total = project_ids.len(), "projects");
            }
        }

        // Contacts referenced by projects, plus the businesses people belong to.
        let mut queue: Vec<i64> = contact_ids.iter().copied().collect();
        let mut seen = BTreeSet::new();
        while let Some(cid) = queue.pop() {
            if !seen.insert(cid) {
                continue;
            }
            if let Some(contact) = self
                .fetch(
                    &format!("Api/R3/Contact/{cid}"),
                    &format!("contacts/{cid}.json"),
                    &mut summary,
                )
                .await?
            {
                summary.contacts_fetched += 1;
                if let Some(business) = contact
                    .get("BusinessId")
                    .and_then(as_i64)
                    .filter(|b| *b > 0)
                {
                    queue.push(business);
                }
                self.fetch(
                    &format!("Api/R3/AddressList/{cid}"),
                    &format!("addresses/{cid}.json"),
                    &mut summary,
                )
                .await?;
            }
        }

        write_atomic(
            &self.path("extract-summary.json"),
            &serde_json::to_vec_pretty(&summary)?,
        )
        .await?;
        Ok(summary)
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
}
