use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::{mutations::failure, navigation::Route};

const MAX_RECORDS: usize = 20_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const FIELD_BYTES: usize = 256 * 1024;
const PAGE: usize = 32;

#[derive(Clone, Debug)]
pub struct Hit {
    pub route: Route,
    pub title: Arc<str>,
    pub excerpt: Arc<str>,
}

pub struct Results {
    pub hits: Arc<[Hit]>,
    pub limited: bool,
}

struct Entry {
    revision: String,
    hit: Hit,
    text: String,
}

#[derive(Clone, Default)]
pub struct SearchEngine {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
}

impl SearchEngine {
    pub fn query(
        &self,
        runtime: &RuntimeHandle,
        query: Arc<str>,
        viewer: Option<Arc<str>>,
        cancel: CancellationToken,
    ) -> Result<Reply<Results>> {
        let entries = self.entries.clone();
        runtime.read(cancel.clone(), move |services| async move {
            let mut cache = entries.lock().await;
            let terms = query.split_whitespace().take(32).map(str::to_lowercase).collect::<Vec<_>>();
            let mut next = HashMap::new();
            let mut bytes = 0;
            let mut hits = BTreeMap::new();
            let mut offset = 0;
            let mut limited = false;
            loop {
                if cancel.is_cancelled() { return Err(ServiceError::Cancelled); }
                let rows = services.executor.execute(
                    "WITH source AS (
                        SELECT 'session' AS kind,s.id,s.title,s.updated_at,
                            d.id AS document_id,d.updated_at AS revision,d.body AS content,d.body_format AS format,s.created_at AS date
                        FROM sessions s LEFT JOIN session_documents d ON d.session_id=s.id AND d.deleted_at IS NULL
                        WHERE s.deleted_at IS NULL AND s.locked=0
                        UNION ALL
                        SELECT 'session',s.id,s.title,s.updated_at,t.id,t.updated_at,t.words_json,'words',s.created_at
                        FROM sessions s JOIN transcripts t ON t.session_id=s.id AND t.deleted_at IS NULL WHERE s.deleted_at IS NULL AND s.locked=0
                        UNION ALL
                        SELECT 'shared',share_id,title,CAST(content_revision AS TEXT),share_id,CAST(content_revision AS TEXT),body_json,'prosemirror_json',published_at
                        FROM shared_session_cache c WHERE viewer_user_id=?1 AND NOT (manage_access=1 AND EXISTS(SELECT 1 FROM sessions s WHERE s.id=c.session_id AND s.deleted_at IS NULL))
                    ) SELECT kind,id,substr(title,1,4096) AS title,updated_at,document_id,revision,
                        substr(content,1,?2) AS content,format,date,length(CAST(content AS BLOB)) AS bytes
                    FROM source ORDER BY date DESC,kind,id,document_id LIMIT ?3 OFFSET ?4".into(),
                    vec![json!(viewer),json!(FIELD_BYTES),json!(PAGE),json!(offset)],
                ).await.map_err(failure)?;
                for row in &rows {
                    if cancel.is_cancelled() { return Err(ServiceError::Cancelled); }
                    let string = |key| row[key].as_str().unwrap_or("");
                    let key = format!("{}:{}:{}:{}",viewer.as_deref().unwrap_or(""),string("kind"),string("id"),string("document_id"));
                    let revision = format!("{}:{}:{}",string("updated_at"),string("revision"),string("title"));
                    let entry = if let Some(entry) = cache.remove(&key).filter(|entry| entry.revision == revision) { entry } else {
                        let content = string("content");
                        let plain = if matches!(string("format"), "prosemirror_json" | "words") {
                            serde_json::from_str::<Value>(content).map(|value| visible_text(&value)).unwrap_or_default()
                        } else { content.to_owned() };
                        let title = string("title");
                        Entry { revision, text: format!("{title}\n{plain}").to_lowercase(),
                            hit: Hit { route: if string("kind") == "shared" { Route::SharedSession(string("id").into()) } else { Route::Session(string("id").to_owned().into()) },
                                title: if title.is_empty() { "Untitled note".into() } else { title.into() },
                                excerpt: plain.chars().take(160).collect::<String>().into(),
                            }
                        }
                    };
                    bytes += entry.text.len() + entry.revision.len() + entry.hit.title.len() + entry.hit.excerpt.len() + key.len();
                    limited |= row["bytes"].as_u64().unwrap_or(0) > FIELD_BYTES as u64;
                    if terms.iter().all(|term| entry.text.contains(term)) && hits.len() < 201 {
                        hits.entry(format!("{}:{}",string("kind"),string("id"))).or_insert_with(|| entry.hit.clone());
                    }
                    next.insert(key, entry);
                    if next.len() >= MAX_RECORDS || bytes >= MAX_BYTES { limited = true; break; }
                }
                if rows.len() < PAGE || next.len() >= MAX_RECORDS || bytes >= MAX_BYTES { break; }
                offset += PAGE;
            }
            limited |= hits.len() > 200;
            *cache = next;
            Ok(Results { hits: hits.into_values().take(200).collect(), limited })
        })
    }
}

fn visible_text(value: &Value) -> String {
    let mut text = String::new();
    let mut queue = vec![value];
    while let Some(value) = queue.pop() {
        match value {
            Value::Array(items) => queue.extend(items.iter().rev()),
            Value::Object(object) => {
                if let Some(value) = object.get("text").and_then(Value::as_str) {
                    text.push_str(value);
                    text.push(' ');
                }
                if let Some(value) = object.get("content") {
                    queue.push(value);
                }
            }
            _ => {}
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn searchable_text_keeps_unicode_without_indexing_node_metadata() {
        assert_eq!(
            visible_text(
                &json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"안녕 café"}]},{"type":"unknown","attrs":{"secret":"metadata"},"content":[{"text":"世界"}]}]})
            ),
            "안녕 café 世界 "
        );
    }
}
