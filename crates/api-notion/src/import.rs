use std::collections::HashSet;

use anlg_meeting_import::{
    ImportedMeeting, TranscriptSegment, hhmmss_to_ms, meeting_file, meeting_has_content, nonempty,
};
use anlg_nango::OwnedNangoProxy;
use serde_json::{Value, json};

use crate::error::{NotionError, Result};

const MEETING_NOTES_VERSION: &str = "2026-03-11";
const MAX_CHILD_PAGES: usize = 20;

pub struct ImportFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

pub struct ImportResult {
    pub files: Vec<ImportFile>,
    pub warnings: Vec<String>,
}

pub async fn import_meetings(
    proxy: &OwnedNangoProxy,
    known_meeting_ids: &[String],
) -> Result<ImportResult> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let payload = notion_post(
        proxy,
        "/v1/blocks/meeting_notes/query",
        json!({
            "sort": [{ "property": "last_edited_time", "direction": "descending" }],
            "limit": 50
        }),
    )
    .await?;

    let results = payload["results"].as_array().cloned().unwrap_or_default();
    let mut files = Vec::new();
    let mut without_content = 0;
    let query_warning = results
        .is_empty()
        .then(|| payload["message"].as_str().map(str::to_string))
        .flatten();

    for result in results {
        let Some(id) = result["id"].as_str().map(str::to_string) else {
            continue;
        };
        if known.contains(&id) {
            continue;
        }
        let imported = meeting_from_block(proxy, &result).await?;
        if !meeting_has_content(&imported) {
            without_content += 1;
            continue;
        }
        let file = meeting_file("notion", &imported).map_err(NotionError::Internal)?;
        files.push(ImportFile {
            path: file.path,
            name: file.name,
            content: file.content,
        });
    }

    let mut warnings = Vec::new();
    if let Some(message) = query_warning {
        warnings.push(format!(
            "Notion meeting notes could not be listed: {message}"
        ));
    }
    if files.is_empty() && without_content == 0 && warnings.is_empty() {
        warnings
            .push("Notion did not return any AI meeting notes for the connected user.".to_string());
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Notion meeting notes did not include summary, notes, or a transcript."
        ));
    }
    Ok(ImportResult { files, warnings })
}

async fn meeting_from_block(proxy: &OwnedNangoProxy, block: &Value) -> Result<ImportedMeeting> {
    let id = block["id"]
        .as_str()
        .ok_or_else(|| NotionError::Internal("meeting note is missing an id".into()))?
        .to_string();
    let notes = &block["meeting_notes"];
    let title = rich_text(&notes["title"]).unwrap_or_else(|| "Notion meeting".to_string());
    let start_time = nonempty(notes["recording"]["start_time"].as_str())
        .or_else(|| nonempty(notes["calendar_event"]["start_time"].as_str()))
        .or_else(|| nonempty(block["created_time"].as_str()));
    let children = &notes["children"];
    let summary = block_markdown(proxy, children["summary_block_id"].as_str()).await?;
    let notes_text = block_markdown(proxy, children["notes_block_id"].as_str()).await?;
    let transcript = block_transcript(proxy, children["transcript_block_id"].as_str()).await?;
    let url = format!("https://www.notion.so/{}", id.replace('-', ""));
    Ok(ImportedMeeting {
        id,
        title,
        start_time,
        url: Some(url),
        summary,
        notes: notes_text,
        transcript,
        action_items: Vec::new(),
    })
}

async fn block_markdown(proxy: &OwnedNangoProxy, block_id: Option<&str>) -> Result<Option<String>> {
    let Some(block_id) = block_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let children = list_children(proxy, block_id).await?;
    Ok(nonempty(Some(&blocks_to_markdown(&children))))
}

async fn block_transcript(
    proxy: &OwnedNangoProxy,
    block_id: Option<&str>,
) -> Result<Vec<TranscriptSegment>> {
    let Some(block_id) = block_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };
    let children = list_children(proxy, block_id).await?;
    Ok(blocks_to_transcript(&children))
}

async fn list_children(proxy: &OwnedNangoProxy, block_id: &str) -> Result<Vec<Value>> {
    let mut children = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_CHILD_PAGES {
        let mut path = format!("/v1/blocks/{block_id}/children?page_size=100");
        if let Some(cursor) = cursor.as_deref() {
            path.push_str("&start_cursor=");
            path.push_str(cursor);
        }
        let payload = notion_get(proxy, &path).await?;
        if let Some(results) = payload["results"].as_array() {
            children.extend(results.iter().cloned());
        }
        if payload["has_more"].as_bool() != Some(true) {
            break;
        }
        let next = payload["next_cursor"]
            .as_str()
            .map(str::to_string)
            .filter(|value| !value.is_empty());
        if next == cursor {
            break;
        }
        match next {
            Some(value) => cursor = Some(value),
            None => break,
        }
    }
    Ok(children)
}

async fn notion_post(proxy: &OwnedNangoProxy, path: &str, body: Value) -> Result<Value> {
    notion_send(
        proxy
            .post(
                path,
                serde_json::to_vec(&body).map_err(|e| NotionError::Notion(e.to_string()))?,
                "application/json",
            )
            .map_err(|e| NotionError::Notion(e.to_string()))?,
    )
    .await
}

async fn notion_get(proxy: &OwnedNangoProxy, path: &str) -> Result<Value> {
    notion_send(
        proxy
            .get(path)
            .map_err(|e| NotionError::Notion(e.to_string()))?,
    )
    .await
}

async fn notion_send(builder: reqwest::RequestBuilder) -> Result<Value> {
    let response = builder
        .header("Notion-Version", MEETING_NOTES_VERSION)
        .send()
        .await
        .map_err(|e| NotionError::Notion(e.to_string()))?;
    let status = response.status();
    let payload: Value = response
        .json()
        .await
        .map_err(|e| NotionError::Notion(e.to_string()))?;
    if !status.is_success() {
        let message = payload["message"]
            .as_str()
            .unwrap_or("Notion request failed");
        return Err(NotionError::Notion(format!("{status}: {message}")));
    }
    Ok(payload)
}

fn blocks_to_markdown(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(block_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn blocks_to_transcript(blocks: &[Value]) -> Vec<TranscriptSegment> {
    blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let text = block_line(block)?;
            let (speaker, spoken) = if let Some((speaker, spoken)) = text.split_once(": ")
                && speaker.len() <= 60
            {
                (speaker.trim().to_string(), spoken.trim().to_string())
            } else {
                (String::new(), text)
            };
            let start_ms = timestamp_prefix(&spoken)
                .or_else(|| timestamp_prefix(block_plain_text(block).as_deref().unwrap_or("")))
                .unwrap_or((index as u64).saturating_mul(1_000));
            Some(TranscriptSegment {
                speaker,
                text: spoken,
                start_ms,
                end_ms: start_ms,
            })
        })
        .collect()
}

fn timestamp_prefix(text: &str) -> Option<u64> {
    let stamp = text.split_whitespace().next()?;
    if stamp.contains(':') && stamp.chars().all(|c| c.is_ascii_digit() || c == ':') {
        Some(hhmmss_to_ms(stamp))
    } else {
        None
    }
}

fn block_line(block: &Value) -> Option<String> {
    block_plain_text(block).and_then(|text| nonempty(Some(&text)))
}

fn block_plain_text(block: &Value) -> Option<String> {
    let block_type = block["type"].as_str()?;
    rich_text(&block[block_type]["rich_text"])
}

fn rich_text(value: &Value) -> Option<String> {
    let fragments = value.as_array()?;
    let text = fragments
        .iter()
        .filter_map(|fragment| fragment["plain_text"].as_str())
        .collect::<String>();
    nonempty(Some(&text))
}
