//! Shipping's `apply_session_correction` (`chat/tools/session-correction.ts`) and
//! `move_meeting_contents` (`session/move-contents.ts`) tools.

use std::collections::HashSet;

use desktop_runtime::{CancellationToken, Result, RuntimeHandle};
use serde_json::{Value, json};

use super::{context::markdown, model::failure};

pub struct Request {
    pub explicit_session: bool,
    pub session: Option<String>,
    pub target: String,
    pub summary: Option<String>,
    pub active_summary: Option<String>,
    pub old_text: String,
    pub new_text: String,
    pub dictionary_terms: Vec<String>,
}

fn replace_exact(value: &str, old: &str, new: &str) -> (String, usize) {
    if old.is_empty() {
        return (value.to_owned(), 0);
    }
    (value.replace(old, new), value.matches(old).count())
}

fn word_char(character: char) -> bool {
    character.is_alphanumeric()
}

fn same_ignoring_case(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// `replaceBoundedExact`: case-insensitive whole-word replacement.
fn replace_bounded(value: &str, old: &str, new: &str) -> (String, usize) {
    let chars = value.chars().collect::<Vec<_>>();
    let needle = old.chars().collect::<Vec<_>>();
    if needle.is_empty() {
        return (value.to_owned(), 0);
    }
    let mut out = String::new();
    let mut count = 0;
    let mut index = 0;
    while index < chars.len() {
        let bounded_start = index == 0 || !word_char(chars[index - 1]);
        let end = index + needle.len();
        if bounded_start
            && end <= chars.len()
            && (end == chars.len() || !word_char(chars[end]))
            && chars[index..end]
                .iter()
                .zip(&needle)
                .all(|(a, b)| same_ignoring_case(*a, *b))
        {
            out.push_str(new);
            count += 1;
            index = end;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    (out, count)
}

fn trim_token(value: &str) -> &str {
    let trimmed = value.trim_matches(|c: char| !word_char(c));
    if trimmed.is_empty() { value } else { trimmed }
}

fn comparable(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| word_char(*c))
        .collect()
}

fn replacement_tokens(value: &str) -> Vec<&str> {
    value
        .split_whitespace()
        .map(trim_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn comparable_tokens(value: &str) -> Vec<String> {
    replacement_tokens(value)
        .into_iter()
        .map(comparable)
        .filter(|token| !token.is_empty())
        .collect()
}

/// `buildReplacementWords`: the new tokens share the replaced words' time span.
fn replacement_words(original: &[Value], new: &str) -> Vec<Value> {
    let tokens = replacement_tokens(new);
    let first = original.first().cloned().unwrap_or_else(|| json!({}));
    let last = original.last().cloned().unwrap_or_else(|| first.clone());
    let start = first["start_ms"].as_f64();
    let end = last["end_ms"].as_f64().or(start);
    let step = match (start, end) {
        (Some(start), Some(end)) => (end - start).max(0.) / tokens.len().max(1) as f64,
        _ => 0.,
    };
    let base = first["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .unwrap_or("word")
        .to_owned();
    tokens
        .iter()
        .enumerate()
        .map(|(index, token)| {
            let mut word = first.clone();
            if index > 0 {
                word["id"] = json!(format!("{base}:correction:{index}"));
            }
            word["text"] = json!(token);
            if let Some(start) = start {
                word["start_ms"] = json!((start + step * index as f64).round() as i64);
                word["end_ms"] = json!((start + step * (index + 1) as f64).round() as i64);
            }
            word
        })
        .collect()
}

fn replace_words(words: &[Value], old: &str, new: &str) -> (Vec<Value>, usize) {
    let target = comparable_tokens(old);
    let tokens = replacement_tokens(new);
    if target.is_empty() || tokens.is_empty() || target.len() > words.len() {
        return (words.to_vec(), 0);
    }
    let text = |word: &Value| word["text"].as_str().unwrap_or_default().to_owned();
    let mut next = Vec::new();
    let mut count = 0;
    let mut index = 0;
    while index < words.len() {
        if target.len() == 1 && tokens.len() == 1 {
            let (replaced, found) = replace_bounded(&text(&words[index]), old, new);
            if found > 0 {
                let mut word = words[index].clone();
                word["text"] = json!(replaced);
                next.push(word);
                count += found;
                index += 1;
                continue;
            }
        }
        let end = index + target.len();
        if end <= words.len()
            && words[index..end]
                .iter()
                .zip(&target)
                .all(|(word, target)| comparable(&text(word)) == *target)
        {
            next.extend(replacement_words(&words[index..end], new));
            index = end;
            count += 1;
            continue;
        }
        next.push(words[index].clone());
        index += 1;
    }
    if count == 0 {
        (words.to_vec(), 0)
    } else {
        (next, count)
    }
}

/// Shipping's `TEXT_TOKEN_PATTERN`: letters and digits, joined by `'`, `’`, `_` or `-`.
fn text_tokens(value: &str) -> Vec<(String, usize, usize)> {
    let chars = value.char_indices().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !word_char(chars[index].1) {
            index += 1;
            continue;
        }
        let start = chars[index].0;
        let mut end = index;
        loop {
            while end < chars.len() && word_char(chars[end].1) {
                end += 1;
            }
            if end + 1 < chars.len()
                && matches!(chars[end].1, '\'' | '’' | '_' | '-')
                && word_char(chars[end + 1].1)
            {
                end += 1;
                continue;
            }
            break;
        }
        let stop = chars.get(end).map_or(value.len(), |(offset, _)| *offset);
        let token = comparable(&value[start..stop]);
        if !token.is_empty() {
            tokens.push((token, start, stop));
        }
        index = end;
    }
    tokens
}

/// `replaceLoosePhrase`: matches word tokens regardless of case and punctuation.
fn replace_loose(value: &str, old: &str, new: &str) -> (String, usize) {
    let target = comparable_tokens(old);
    let tokens = text_tokens(value);
    if target.is_empty() || target.len() > tokens.len() {
        return (value.to_owned(), 0);
    }
    let mut out = String::new();
    let mut cursor = 0;
    let mut count = 0;
    let mut index = 0;
    while index + target.len() <= tokens.len() {
        if tokens[index..index + target.len()]
            .iter()
            .zip(&target)
            .all(|((token, _, _), target)| token == target)
        {
            out.push_str(&value[cursor..tokens[index].1]);
            out.push_str(new);
            cursor = tokens[index + target.len() - 1].2;
            count += 1;
            index += target.len();
        } else {
            index += 1;
        }
    }
    if count == 0 {
        return (value.to_owned(), 0);
    }
    out.push_str(&value[cursor..]);
    (out, count)
}

fn replace_text(value: &str, old: &str, new: &str) -> (String, usize) {
    let (replaced, count) =
        if comparable_tokens(old).len() == 1 && replacement_tokens(new).len() == 1 {
            replace_bounded(value, old, new)
        } else {
            replace_exact(value, old, new)
        };
    if count > 0 {
        (replaced, count)
    } else {
        replace_loose(value, old, new)
    }
}

/// `normalizeKeywordList`.
fn keywords(terms: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    terms
        .into_iter()
        .map(|term| term.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|term| term.chars().count() >= 2 && seen.insert(term.to_lowercase()))
        .collect()
}

async fn rows(
    runtime: &RuntimeHandle,
    sql: &'static str,
    params: Vec<Value>,
) -> Result<Vec<Value>> {
    runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(sql.into(), params)
                .await
                .map_err(failure)
        })?
        .receive()
        .await
}

fn text(row: &Value, key: &str) -> String {
    row[key].as_str().unwrap_or_default().to_owned()
}

/// Adds new dictionary terms to the synced preference, returning the ones added.
async fn save_terms(runtime: &RuntimeHandle, terms: Vec<String>) -> Result<Vec<String>> {
    const KEY: &str = "personalization_dictionary_terms";
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let stored = rows(
        runtime,
        "SELECT id, value_json, 0 AS oversized, 0 AS source_rank FROM app_settings WHERE id IN (?1, 'cloudsync_workspace_binding', 'legacy_settings_document', 'legacy_main_values_document') UNION ALL SELECT id, value_json, 0, 1 FROM synced_preferences WHERE id = ?1 ORDER BY id, source_rank",
        vec![json!(KEY)],
    )
    .await?;
    let base = crate::product::settings::decode(&stored)?
        .remove(KEY)
        .ok_or_else(|| failure("Dictionary setting is unavailable"))?;
    let current = keywords(
        base.value
            .as_str()
            .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
            .unwrap_or_default(),
    );
    let known = current
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<HashSet<_>>();
    let added = keywords(terms)
        .into_iter()
        .filter(|term| !known.contains(&term.to_lowercase()))
        .collect::<Vec<_>>();
    if !added.is_empty() {
        let all = keywords(current.into_iter().chain(added.iter().cloned()));
        crate::product::settings::save(runtime, KEY.into(), base, json!(all))?
            .receive()
            .await?;
    }
    Ok(added)
}

pub async fn apply(runtime: &RuntimeHandle, request: Request) -> Result<Value> {
    let Some(session) = request.session else {
        return Ok(
            json!({"status": "error", "message": "No active session selected. Provide sessionId explicitly when calling apply_session_correction."}),
        );
    };
    let new = request.new_text.trim().to_owned();
    if new.is_empty() {
        return Ok(
            json!({"status": "error", "message": "Replacement text cannot be blank.", "sessionId": session}),
        );
    }
    let old = request.old_text;
    let summary_id = request.summary.or_else(|| {
        (!request.explicit_session)
            .then_some(request.active_summary)
            .flatten()
    });
    let title_row = rows(
        runtime,
        "SELECT title FROM sessions WHERE id = ? AND deleted_at IS NULL",
        vec![json!(session)],
    )
    .await?;
    let Some(title_row) = title_row.first() else {
        return Ok(
            json!({"status": "error", "message": "The target session could not be loaded.", "sessionId": session}),
        );
    };
    let title = text(title_row, "title");
    let summaries = rows(
        runtime,
        "SELECT id, title, body, body_format FROM session_documents WHERE session_id = ? AND kind IN ('summary', 'template_output') AND deleted_at IS NULL ORDER BY sort_order, id",
        vec![json!(session)],
    )
    .await?;
    let transcripts = rows(
        runtime,
        "SELECT id, words_json, memo FROM transcripts WHERE session_id = ? AND deleted_at IS NULL ORDER BY created_at, id",
        vec![json!(session)],
    )
    .await?;
    let target = request.target.as_str();
    let mut edit_summary = matches!(target, "summary" | "summary_and_transcript");
    let edit_transcript = matches!(target, "transcript" | "summary_and_transcript");
    if edit_summary
        && let Some(id) = &summary_id
        && !summaries.iter().any(|row| row["id"] == id.as_str())
    {
        if target == "summary" {
            return Ok(
                json!({"status": "error", "message": "The requested summary does not belong to the target session.", "sessionId": session}),
            );
        }
        edit_summary = false;
    }

    let mut summary_changes = Vec::new();
    let mut summary_updates = Vec::new();
    if edit_summary {
        for row in &summaries {
            if summary_id
                .as_ref()
                .is_some_and(|id| row["id"] != id.as_str())
            {
                continue;
            }
            let current =
                markdown(&text(row, "body"), &text(row, "body_format")).unwrap_or_default();
            let (replaced, count) = replace_exact(&current, &old, &new);
            if count == 0 {
                continue;
            }
            let next = anlg_tiptap::md_to_tiptap_json(&replaced)
                .map_err(failure)?
                .to_string();
            summary_updates.push((
                text(row, "id"),
                text(row, "body"),
                text(row, "body_format"),
                next,
            ));
            summary_changes.push(json!({
                "enhancedNoteId": row["id"],
                "title": Some(text(row, "title").trim().to_owned()).filter(|t| !t.is_empty()).unwrap_or_else(|| "Summary".into()),
                "replacements": count,
            }));
        }
    }
    let mut transcript_changes = Vec::new();
    let mut transcript_updates = Vec::new();
    if edit_transcript && !replacement_tokens(&new).is_empty() {
        for row in &transcripts {
            let words_json = text(row, "words_json");
            let memo = text(row, "memo");
            let words = serde_json::from_str::<Vec<Value>>(&words_json).unwrap_or_default();
            let (next_words, word_count) = replace_words(&words, &old, &new);
            let (next_memo, memo_count) = replace_text(&memo, &old, &new);
            if word_count == 0 && memo_count == 0 {
                continue;
            }
            if !words.is_empty() && !memo.is_empty() && (word_count == 0 || memo_count == 0) {
                continue;
            }
            transcript_updates.push((
                text(row, "id"),
                words_json.clone(),
                memo.clone(),
                if word_count > 0 {
                    serde_json::to_string(&next_words).map_err(failure)?
                } else {
                    words_json
                },
                if memo_count > 0 { next_memo } else { memo },
            ));
            transcript_changes.push(json!({
                "transcriptId": row["id"],
                "wordReplacements": word_count,
                "memoReplacements": memo_count,
            }));
        }
    }
    let title_change = edit_summary
        .then(|| replace_exact(&title, &old, &new))
        .filter(|(next, count)| *count > 0 && *next != title);

    if summary_changes.is_empty() && transcript_changes.is_empty() && title_change.is_none() {
        return Ok(json!({
            "status": "not_found",
            "message": "No exact match found. Read the note and call apply_session_correction with the exact current text.",
            "sessionId": session,
        }));
    }

    let write_session = session.clone();
    let next_title = title_change.as_ref().map(|(next, _)| next.clone());
    let committed = runtime
        .submit(move |services| async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let mut transaction = services
                .db
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(failure)?;
            let mut affected = Vec::new();
            if let Some(next) = &next_title {
                affected.push(
                    anlg_db_app::update_session_title(
                        &mut transaction,
                        &write_session,
                        &title,
                        next,
                        &now,
                    )
                    .await
                    .map_err(failure)?,
                );
            }
            for (id, body, format, next) in &summary_updates {
                affected.push(
                    anlg_db_app::update_summary_document(
                        &mut transaction,
                        anlg_db_app::SessionDocumentBodyUpdate {
                            id,
                            session_id: &write_session,
                            current_body: body,
                            current_body_format: format,
                            next_body: next,
                            updated_at: &now,
                        },
                    )
                    .await
                    .map_err(failure)?,
                );
            }
            for (id, words, memo, next_words, next_memo) in &transcript_updates {
                affected.push(
                    anlg_db_app::update_session_transcript(
                        &mut transaction,
                        anlg_db_app::TranscriptContentUpdate {
                            id,
                            session_id: &write_session,
                            current_words_json: words,
                            current_memo: memo,
                            next_words_json: next_words,
                            next_memo,
                            updated_at: &now,
                        },
                    )
                    .await
                    .map_err(failure)?,
                );
            }
            if affected.iter().any(|rows| *rows != 1) {
                transaction.rollback().await.map_err(failure)?;
                return Err(failure(
                    "The note changed before the correction was committed.",
                ));
            }
            transaction.commit().await.map_err(failure)
        })?
        .receive()
        .await;
    if committed.is_err() {
        return Ok(json!({
            "status": "error",
            "message": "The note changed before the correction could be committed. Read the note and retry.",
            "sessionId": session,
        }));
    }

    let (added, dictionary_failed) = match save_terms(runtime, request.dictionary_terms).await {
        Ok(added) => (added, false),
        Err(_) => (Vec::new(), true),
    };
    let mut missing = Vec::new();
    if edit_summary && summary_changes.is_empty() && title_change.is_none() {
        missing.push("summary");
    }
    if edit_transcript && transcript_changes.is_empty() {
        missing.push("transcript");
    }
    let mut messages = Vec::new();
    if !missing.is_empty() {
        messages.push(format!(
            "Applied correction where matched, but no matching {} text was found.",
            missing.join(" or ")
        ));
    }
    if dictionary_failed {
        messages.push(
            "The correction was applied, but dictionary terms could not be saved.".to_owned(),
        );
    }
    let mut result = json!({
        "status": if missing.is_empty() { "applied" } else { "partial" },
        "sessionId": session,
        "summaryChanges": summary_changes,
        "transcriptChanges": transcript_changes,
        "titleChange": title_change.map(|(next, count)| json!({"replacements": count, "nextTitle": next})),
        "dictionaryChanges": {"addedTerms": added},
    });
    if !messages.is_empty() {
        result["message"] = json!(messages.join(" "));
    }
    Ok(result)
}

fn note_has_content(markdown: &str) -> bool {
    let trimmed = markdown.trim();
    !trimmed.is_empty() && trimmed != "&nbsp;"
}

/// Shipping's `moveSessionContents`, for meetings without a recording file: transcripts,
/// summaries, notes and action items move onto an empty target meeting.
pub async fn move_contents(
    runtime: &RuntimeHandle,
    source: String,
    target: String,
) -> Result<Value> {
    let ids = json!({"sourceMeetingId": source, "targetMeetingId": target});
    let error = |message: &str| {
        let mut value = ids.clone();
        value["status"] = json!("error");
        value["message"] = json!(message);
        Ok(value)
    };
    if source.is_empty() || target.is_empty() {
        return error("Both the source and target meetings are required.");
    }
    if source == target {
        return error("Choose two different meetings.");
    }
    let load = |id: String| async move {
        super::tools::load_meeting(runtime, &id, &CancellationToken::new()).await
    };
    let Some(source_meeting) = load(source.clone()).await? else {
        return error("The source meeting could not be loaded.");
    };
    let Some(target_meeting) = load(target.clone()).await? else {
        return error("The target meeting could not be loaded.");
    };
    let counts = rows(
        runtime,
        "SELECT (SELECT COUNT(*) FROM transcripts WHERE session_id = ?1 AND deleted_at IS NULL) AS source_transcripts, (SELECT COUNT(*) FROM transcripts WHERE session_id = ?2 AND deleted_at IS NULL) AS target_transcripts, (SELECT COUNT(*) FROM action_items WHERE session_id = ?1 AND deleted_at IS NULL) AS action_items, EXISTS (SELECT 1 FROM session_attachments WHERE session_id = ?1 AND id = 'session-audio:' || ?1 AND deleted_at IS NULL) AS source_audio, EXISTS (SELECT 1 FROM session_attachments WHERE session_id = ?2 AND id = 'session-audio:' || ?2 AND deleted_at IS NULL) AS target_audio",
        vec![json!(source), json!(target)],
    )
    .await?;
    let count = |key: &str| {
        counts
            .first()
            .and_then(|row| row[key].as_i64())
            .unwrap_or(0)
    };
    let source_markdown = source_meeting
        .note
        .as_ref()
        .map(|note| note.markdown.clone())
        .unwrap_or_default();
    let source_notes = note_has_content(&source_markdown);
    let anything = count("source_audio") > 0
        || count("source_transcripts") > 0
        || !source_meeting.summaries.is_empty()
        || source_notes
        || count("action_items") > 0;
    if !anything {
        let mut value = ids.clone();
        value["status"] = json!("nothing_to_move");
        value["message"] =
            json!("The source meeting has no recording, transcript, or notes to move.");
        return Ok(value);
    }
    if count("target_audio") > 0 || count("target_transcripts") > 0 {
        return error(
            "The target meeting already has a recording or transcript. Move into an empty meeting, or delete the existing recording first.",
        );
    }
    if count("source_audio") > 0 {
        return error("Moving a recording file is not supported in this build yet.");
    }
    let target_markdown = target_meeting
        .note
        .as_ref()
        .map(|note| note.markdown.clone())
        .unwrap_or_default();
    let next_note = source_notes
        .then(|| {
            let merged = if note_has_content(&target_markdown) {
                format!("{}\n\n{}", target_markdown.trim(), source_markdown.trim())
            } else {
                source_markdown.trim().to_owned()
            };
            anlg_tiptap::md_to_tiptap_json(&merged).map(|doc| doc.to_string())
        })
        .transpose()
        .map_err(failure)?;
    let empty_note = anlg_tiptap::md_to_tiptap_json("")
        .map_err(failure)?
        .to_string();
    let (from, to) = (source.clone(), target.clone());
    let source_note_id = source_meeting.note.as_ref().map(|note| note.id.clone());
    let target_note_id = target_meeting.note.as_ref().map(|note| note.id.clone());
    let moved = runtime
        .submit(move |services| async move {
            let now = chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string();
            let (source_audio, target_audio) = (
                format!("session-audio:{from}"),
                format!("session-audio:{to}"),
            );
            let request = anlg_db_app::SessionAudioMove {
                source_session_id: &from,
                target_session_id: &to,
                rewrite_audio_ids: false,
                source_audio_id: &source_audio,
                target_audio_id: &target_audio,
                now: &now,
            };
            let mut transaction = services
                .db
                .pool()
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(failure)?;
            let transcripts = anlg_db_app::move_session_transcripts(&mut transaction, &request)
                .await
                .map_err(failure)?;
            let summaries =
                anlg_db_app::move_session_summary_documents(&mut transaction, &from, &to, &now)
                    .await
                    .map_err(failure)?;
            let action_items =
                anlg_db_app::move_session_action_items(&mut transaction, &from, &to, &now)
                    .await
                    .map_err(failure)?;
            anlg_db_app::move_session_voiceprint_exemplars(&mut transaction, &request)
                .await
                .map_err(failure)?;
            anlg_db_app::move_session_voiceprint_candidates(&mut transaction, &request)
                .await
                .map_err(failure)?;
            if let Some(next) = &next_note {
                let (Some(target_note), Some(source_note)) = (&target_note_id, &source_note_id)
                else {
                    transaction.rollback().await.map_err(failure)?;
                    return Err(failure("The target meeting has no note to merge into."));
                };
                let target_rows = anlg_db_app::rewrite_session_note_body(
                    &mut transaction,
                    target_note,
                    &to,
                    next,
                    &now,
                )
                .await
                .map_err(failure)?;
                let source_rows = anlg_db_app::rewrite_session_note_body(
                    &mut transaction,
                    source_note,
                    &from,
                    &empty_note,
                    &now,
                )
                .await
                .map_err(failure)?;
                if target_rows != 1 || source_rows != 1 {
                    transaction.rollback().await.map_err(failure)?;
                    return Err(failure("The notes changed while moving."));
                }
            }
            transaction.commit().await.map_err(failure)?;
            Ok((transcripts, summaries, action_items))
        })?
        .receive()
        .await;
    match moved {
        Ok((transcripts, summaries, action_items)) => Ok(json!({
            "status": "moved",
            "sourceMeetingId": source,
            "targetMeetingId": target,
            "sourceTitle": source_meeting.title,
            "targetTitle": target_meeting.title,
            "moved": {
                "recording": false,
                "transcripts": transcripts,
                "summaries": summaries,
                "notes": source_notes,
                "actionItems": action_items,
            },
        })),
        Err(failure) => error(&failure.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_match_whole_words_then_loose_phrases_like_shipping() {
        assert_eq!(
            replace_bounded("Ask anna and Annabel", "Anna", "Ana"),
            ("Ask Ana and Annabel".into(), 1)
        );
        assert_eq!(
            replace_text("We met JON, snow today", "jon snow", "Jon Stark"),
            ("We met Jon Stark today".into(), 1)
        );
        let words = vec![
            json!({"id": "w1", "text": "Open", "start_ms": 0, "end_ms": 100}),
            json!({"id": "w2", "text": "A.I.,", "start_ms": 100, "end_ms": 200}),
            json!({"id": "w3", "text": "rocks", "start_ms": 200, "end_ms": 300}),
        ];
        let (next, count) = replace_words(&words, "open ai", "OpenAI");
        assert_eq!(count, 1);
        assert_eq!(next[0]["text"], "OpenAI");
        assert_eq!(next[0]["end_ms"], 200);
        assert_eq!(next[1]["text"], "rocks");
        // A hyphenated name is one token, as in shipping's pattern.
        assert_eq!(replace_loose("Jon-Snow", "jon snow", "x").1, 0);
        assert_eq!(
            keywords(["  Anarlog ".into(), "anarlog".into(), "x".into()]),
            vec!["Anarlog".to_owned()]
        );
    }
}
