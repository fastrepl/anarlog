//! Markdown export of a meeting (`export_meeting_markdown`, the
//! `automation_markdown_export_*` automation) and the cloud snapshot shape.

use std::io::Write;
use std::path::Path;

use sqlx::SqlitePool;

use crate::MarkdownExportOptions;

const MAX_CLOUD_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;
static MARKDOWN_EXPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `exportMeetingMarkdown(meetingId, directory, options)`
pub async fn export_meeting_markdown(
    pool: &SqlitePool,
    meeting_id: String,
    directory: &str,
    options: Option<&MarkdownExportOptions>,
) -> Result<String, String> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err("export directory is not set".to_string());
    }
    let export = anlg_agent_access::get_meeting_export(pool, meeting_id)
        .await
        .map_err(|error| error.to_string())?;
    write_markdown_export_with_options(Path::new(directory), &export, options)
        .map(|path| path.to_string_lossy().into_owned())
}

// The markdown export automation first runs on meeting.completed, before
// auto-enhance has generated the summary. The note.enhanced dispatch is the
// signal that the summary is persisted, so re-export here to rewrite the file
// with the summary included.
pub async fn run_markdown_export_automation(pool: &SqlitePool, meeting_id: &str) {
    let enabled = load_setting(pool, "automation_markdown_export_enabled")
        .await
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let directory = load_setting(pool, "automation_markdown_export_directory")
        .await
        .and_then(|value| value.as_str().map(|value| value.trim().to_string()))
        .unwrap_or_default();
    if !enabled || directory.is_empty() {
        return;
    }

    let result = match anlg_agent_access::get_meeting_export(pool, meeting_id.to_string()).await {
        Ok(export) => write_markdown_export(std::path::Path::new(&directory), &export),
        Err(error) => Err(error.to_string()),
    };
    let (status, detail) = match result {
        Ok(path) => ("success", path.to_string_lossy().into_owned()),
        Err(error) => {
            tracing::warn!("[local-api] markdown re-export failed: {error}");
            ("error", error)
        }
    };
    let at: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await
        .unwrap_or_default();
    let record =
        crate::sorted_json(&serde_json::json!({ "at": at, "status": status, "detail": detail }))
            .to_string();
    // The settings layer stores this value as a JSON-encoded string, so the
    // record is double-encoded to stay readable by the desktop app.
    let value_json = serde_json::Value::String(record).to_string();
    if let Err(error) = sqlx::query(
        "INSERT INTO app_settings (id, value_json, updated_at) \
         VALUES ('automation_markdown_export_last_run', ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(id) DO UPDATE SET \
           value_json = excluded.value_json, \
           updated_at = excluded.updated_at",
    )
    .bind(value_json)
    .execute(pool)
    .await
    {
        tracing::warn!("[local-api] could not record the markdown export run: {error}");
    }
}

pub async fn load_setting(pool: &SqlitePool, id: &str) -> Option<serde_json::Value> {
    let raw: Option<String> =
        match sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
        {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("[local-api] could not load setting '{id}': {error}");
                None
            }
        };
    raw.and_then(|value| serde_json::from_str(&value).ok())
}

pub fn markdown_export_filename(meeting: &anlg_agent_access::Meeting) -> String {
    configured_markdown_filename(meeting, &MarkdownExportOptions::default())
}

pub fn configured_markdown_filename(
    meeting: &anlg_agent_access::Meeting,
    options: &MarkdownExportOptions,
) -> String {
    let title = meeting.title.trim();
    let title = if title.is_empty() {
        "Untitled meeting"
    } else {
        title
    };
    let occurred_at = if meeting.started_at.is_empty() {
        &meeting.created_at
    } else {
        &meeting.started_at
    };
    let date = occurred_at.get(..10).unwrap_or("");
    let custom = options.filename.trim();
    let base = if custom.is_empty() {
        format!("{date} {title}").trim().to_string()
    } else {
        custom
            .split("{title}")
            .map(|part| part.replace("{date}", date))
            .collect::<Vec<_>>()
            .join(title)
    };
    let base = base.trim();
    let base = if !custom.is_empty() && base.to_ascii_lowercase().ends_with(".md") {
        &base[..base.len() - 3]
    } else {
        base
    };
    let mut sanitized = sanitize_filename_part(base);
    if sanitized.is_empty() {
        sanitized = "Untitled meeting".to_string();
    }
    let stem = sanitized
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        sanitized.insert(0, '_');
    }
    let suffix = if options.include_id_suffix {
        let prefix = meeting.id.chars().take(8).collect::<String>();
        format!(" [{}]", sanitize_filename_part(&prefix))
    } else {
        String::new()
    };
    format!("{sanitized}{suffix}.md")
}

fn sanitize_filename_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>();
    // Leave room for the suffix and extension on filesystems with a 255-byte limit.
    let mut end = sanitized.len().min(180);
    while !sanitized.is_char_boundary(end) {
        end -= 1;
    }
    sanitized[..end].trim_matches([' ', '.']).to_string()
}

pub fn write_markdown_export(
    directory: &std::path::Path,
    export: &anlg_agent_access::MeetingExport,
) -> Result<std::path::PathBuf, String> {
    write_markdown_export_with_options(directory, export, None)
}

pub fn write_markdown_export_with_options(
    directory: &std::path::Path,
    export: &anlg_agent_access::MeetingExport,
    options: Option<&MarkdownExportOptions>,
) -> Result<std::path::PathBuf, String> {
    // Native dispatch and desktop workflows can export the same meeting concurrently.
    let _guard = MARKDOWN_EXPORT_LOCK
        .lock()
        .map_err(|error| error.to_string())?;
    let defaults = MarkdownExportOptions::default();
    let selected = options.unwrap_or(&defaults);
    if !(selected.include_memo
        || selected.include_summary
        || selected.include_transcript
        || selected.include_action_items)
    {
        return Err("choose at least one element to export".to_string());
    }
    let mut filtered = export.clone();
    if !selected.include_memo {
        filtered.meeting.note = None;
    }
    if !selected.include_summary {
        filtered.meeting.summaries.clear();
    }
    if !selected.include_transcript {
        filtered.transcripts.clear();
    }
    if !selected.include_action_items {
        filtered.meeting.action_items.clear();
    }

    std::fs::create_dir_all(directory)
        .map_err(|error| format!("could not create export directory: {error}"))?;
    let filename = if options.is_none() {
        markdown_export_filename(&export.meeting)
    } else {
        configured_markdown_filename(&export.meeting, selected)
    };
    let path = directory.join(&filename);
    let mut markdown = filtered.to_markdown();
    markdown.push('\n');
    let legacy_prefix = legacy_export_prefix(&export.meeting.id);
    if options.is_none() {
        markdown.insert_str(0, &legacy_prefix);
    }
    let existing = match std::fs::read_to_string(&path) {
        Ok(content) => {
            let marker = format!("- ID: `{}`", export.meeting.id);
            let mut lines = content.lines();
            let existing_id = lines.find(|line| line.starts_with("- ID: `"));
            let has_export_date = lines
                .next()
                .is_some_and(|line| line.starts_with("- Date: "));
            if existing_id != Some(marker.as_str()) || !has_export_date {
                return Err(format!(
                    "{filename} already exists for another file; choose a different filename or include the meeting ID suffix"
                ));
            }
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(format!("could not read existing export: {error}")),
    };
    persist_markdown_export(&path, existing, |file| file.write_all(markdown.as_bytes()))
        .map_err(|error| format!("could not write markdown export: {error}"))?;
    // Configured actions can export different content for the same meeting to
    // the same folder. Only the legacy export owns its old filename cleanup.
    if options.is_none() {
        remove_stale_exports(directory, &export.meeting.id, &filename);
    }
    Ok(path)
}

fn persist_markdown_export(
    path: &std::path::Path,
    replace_existing: bool,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let directory = path
        .parent()
        .ok_or_else(|| std::io::Error::other("export has no parent folder"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".anlg-export-")
        .tempfile_in(directory)?;
    write(temporary.as_file_mut())?;
    if replace_existing {
        let metadata = std::fs::metadata(path)?;
        // Atomic replacement keeps the temporary file's ownership. Copying a
        // foreign owner would require privileges even in a writable shared folder.
        temporary
            .as_file()
            .set_permissions(metadata.permissions())?;
    }
    temporary.as_file_mut().flush()?;
    temporary.as_file().sync_all()?;
    if replace_existing {
        temporary.persist(path).map_err(|error| error.error)?;
    } else {
        temporary
            .persist_noclobber(path)
            .map_err(|error| error.error)?;
    }
    Ok(())
}

fn legacy_export_prefix(meeting_id: &str) -> String {
    format!("<!-- anarlog:legacy-markdown-export {meeting_id:?} -->\n\n")
}

// Only remove files explicitly owned by the legacy exporter. Unmarked files
// may belong to users or configured actions, even when their ID suffix matches.
fn remove_stale_exports(directory: &std::path::Path, meeting_id: &str, keep_filename: &str) {
    let id_prefix = meeting_id.chars().take(8).collect::<String>();
    if id_prefix.is_empty() {
        return;
    }
    let marker = format!(" [{}].md", sanitize_filename_part(&id_prefix));
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let owner = legacy_export_prefix(meeting_id);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name != keep_filename
            && name.ends_with(&marker)
            && std::fs::read_to_string(entry.path())
                .is_ok_and(|content| content.starts_with(&owner))
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// `getCloudSnapshot`: the export, words and hints dropped when it is over
/// the size limit.
pub fn prepare_cloud_snapshot(
    mut export: anlg_agent_access::MeetingExport,
) -> Result<serde_json::Value, String> {
    let snapshot = serde_json::to_value(&export).map_err(|error| error.to_string())?;
    if cloud_snapshot_jsonb_len(&snapshot)? <= MAX_CLOUD_SNAPSHOT_BYTES {
        return Ok(snapshot);
    }
    drop(snapshot);
    for transcript in &mut export.transcripts {
        transcript.words.clear();
        transcript.speaker_hints.clear();
    }
    let snapshot = serde_json::to_value(export).map_err(|error| error.to_string())?;
    if cloud_snapshot_jsonb_len(&snapshot)? > MAX_CLOUD_SNAPSHOT_BYTES {
        return Err(format!(
            "meeting snapshot exceeds the {MAX_CLOUD_SNAPSHOT_BYTES}-byte limit"
        ));
    }
    Ok(snapshot)
}

// The hosted constraint measures jsonb::text, which includes separator spaces
// and expands exponent notation, rather than the compact JSON sent over HTTP.
pub fn cloud_snapshot_jsonb_len(value: &serde_json::Value) -> Result<usize, String> {
    match value {
        serde_json::Value::Array(values) => values
            .iter()
            .try_fold(2 + values.len().saturating_sub(1) * 2, |len, value| {
                Ok(len + cloud_snapshot_jsonb_len(value)?)
            }),
        serde_json::Value::Object(values) => values.iter().try_fold(
            2 + values.len().saturating_sub(1) * 2,
            |len, (key, value)| {
                let key_len = serde_json::to_vec(key)
                    .map_err(|error| error.to_string())?
                    .len();
                Ok(len + key_len + 2 + cloud_snapshot_jsonb_len(value)?)
            },
        ),
        serde_json::Value::Number(value) => {
            let number = value.to_string();
            let Some((mantissa, exponent)) = number.split_once('e') else {
                return Ok(number.len());
            };
            let exponent = exponent.parse::<i32>().map_err(|error| error.to_string())?;
            let sign = usize::from(mantissa.starts_with('-'));
            let mantissa = mantissa.trim_start_matches('-');
            let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
            let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
            Ok(sign
                + if decimal <= 0 {
                    2 + (-decimal) as usize + digits
                } else if decimal as usize >= digits {
                    decimal as usize
                } else {
                    digits + 1
                })
        }
        _ => serde_json::to_vec(value)
            .map(|serialized| serialized.len())
            .map_err(|error| error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use crate::MarkdownExportOptions;
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    async fn seeded_pool() -> sqlx::SqlitePool {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at, series_id) \
             VALUES ('meeting-1', 'Planning', '2026-07-13', 'series-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, body_format, body, title) \
             VALUES ('note-1', 'meeting-1', 'note', 'markdown', 'Launch decision', 'Notes')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, started_at_ms, words_json) \
             VALUES ('transcript-1', 'meeting-1', 0, '[{\"text\":\"hello\"},{\"text\":\"world\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().clone()
    }

    #[tokio::test]
    async fn markdown_export_writes_stable_file_into_directory() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();

        assert_eq!(
            super::markdown_export_filename(&export.meeting),
            "2026-07-13 Planning [meeting-].md"
        );

        let untitled = anlg_agent_access::Meeting {
            title: "  ".to_string(),
            started_at: String::new(),
            created_at: "bad".to_string(),
            ..export.meeting.clone()
        };
        assert_eq!(
            super::markdown_export_filename(&untitled),
            "Untitled meeting [meeting-].md"
        );

        let hostile = anlg_agent_access::Meeting {
            title: "a/b:c*d?".to_string(),
            ..export.meeting.clone()
        };
        assert_eq!(
            super::markdown_export_filename(&hostile),
            "2026-07-13 a_b_c_d_ [meeting-].md"
        );

        let directory =
            std::env::temp_dir().join(format!("anlg-md-export-{}", uuid::Uuid::new_v4()));
        let path = super::write_markdown_export(&directory, &export).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "2026-07-13 Planning [meeting-].md"
        );
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("# Planning"));
        assert!(written.contains("hello world"));

        let other_meeting_file = directory.join("2026-07-13 Other [meeting2].md");
        std::fs::write(&other_meeting_file, "other").unwrap();
        let mut retitled = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        retitled.meeting.title = "Planning follow-up".to_string();
        let renamed = super::write_markdown_export(&directory, &retitled).unwrap();
        assert_eq!(
            renamed.file_name().unwrap().to_string_lossy(),
            "2026-07-13 Planning follow-up [meeting-].md"
        );
        assert!(!path.exists(), "stale export under the old title remains");
        assert!(other_meeting_file.exists());
        std::fs::remove_dir_all(&directory).ok();
    }

    #[tokio::test]
    async fn note_enhanced_reexports_markdown_and_records_the_run() {
        let pool = seeded_pool().await;
        let directory = std::env::temp_dir().join(format!("anlg-md-auto-{}", uuid::Uuid::new_v4()));
        for (id, value) in [
            (
                "automation_markdown_export_enabled",
                serde_json::json!(true),
            ),
            (
                "automation_markdown_export_directory",
                serde_json::json!(directory.to_string_lossy()),
            ),
        ] {
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
                .bind(id)
                .bind(value.to_string())
                .execute(&pool)
                .await
                .unwrap();
        }

        super::run_markdown_export_automation(&pool, "meeting-1").await;

        let exported = directory.join("2026-07-13 Planning [meeting-].md");
        assert!(exported.exists());
        let last_run: String = sqlx::query_scalar(
            "SELECT value_json FROM app_settings \
             WHERE id = 'automation_markdown_export_last_run'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        // Stored the way the desktop settings layer writes string settings:
        // a JSON-encoded string containing the record JSON.
        let record: String = serde_json::from_str(&last_run).unwrap();
        let record: serde_json::Value = serde_json::from_str(&record).unwrap();
        assert_eq!(record["status"], "success");
        assert_eq!(record["detail"], exported.to_string_lossy().into_owned());
        assert!(record["at"].as_str().is_some_and(|at| at.ends_with('Z')));
        std::fs::remove_dir_all(&directory).ok();
    }

    #[tokio::test]
    async fn configured_markdown_export_filters_every_content_combination() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let mut summary = export.meeting.note.clone().unwrap();
        summary.title = "Summary".to_string();
        summary.markdown = "Summary-only text".to_string();
        export.meeting.summaries.push(summary);
        export
            .meeting
            .action_items
            .push(anlg_agent_access::ActionItem {
                id: "action-1".to_string(),
                assignee_human_id: String::new(),
                status: "open".to_string(),
                text: "Action-only text".to_string(),
                due_at: String::new(),
                completed_at: None,
            });
        let directory =
            std::env::temp_dir().join(format!("anlg-md-options-{}", uuid::Uuid::new_v4()));
        for bits in 0..16 {
            let options = MarkdownExportOptions {
                include_memo: bits & 1 != 0,
                include_summary: bits & 2 != 0,
                include_transcript: bits & 4 != 0,
                include_action_items: bits & 8 != 0,
                filename: format!("selection-{bits}"),
                include_id_suffix: false,
            };
            let result =
                super::write_markdown_export_with_options(&directory, &export, Some(&options));
            if bits == 0 {
                assert!(result.unwrap_err().contains("at least one"));
                assert!(!directory.exists());
                continue;
            }
            let path = result.unwrap();
            let markdown = std::fs::read_to_string(path).unwrap();
            assert!(markdown.starts_with("# Planning\n"));
            assert_eq!(markdown.contains("Launch decision"), options.include_memo);
            assert_eq!(
                markdown.contains("Summary-only text"),
                options.include_summary
            );
            assert_eq!(markdown.contains("hello world"), options.include_transcript);
            assert_eq!(
                markdown.contains("Action-only text"),
                options.include_action_items
            );
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 15);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn configured_markdown_filenames_are_safe_and_support_patterns() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        for (name, expected) in [
            ("Recap.md", "Recap.md"),
            ("{date} {title} recap", "2026-07-13 Planning recap.md"),
            ("../outside/report", "_outside_report.md"),
            ("CON", "_CON.md"),
            ("...", "Untitled meeting.md"),
            ("  ", "2026-07-13 Planning.md"),
            ("recap.MD", "recap.md"),
            ("a\nb", "a_b.md"),
        ] {
            let options = MarkdownExportOptions {
                filename: name.to_string(),
                include_id_suffix: false,
                ..Default::default()
            };
            assert_eq!(
                super::configured_markdown_filename(&export.meeting, &options),
                expected
            );
        }
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            ..Default::default()
        };
        assert_eq!(
            super::configured_markdown_filename(&export.meeting, &options),
            "Recap [meeting-].md"
        );
        let options = MarkdownExportOptions {
            filename: "会".repeat(300),
            ..Default::default()
        };
        let name = super::configured_markdown_filename(&export.meeting, &options);
        assert!(name.len() < 255);
        assert_eq!(std::path::Path::new(&name).components().count(), 1);
        let defaults: MarkdownExportOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults, MarkdownExportOptions::default());

        let literal_title = anlg_agent_access::Meeting {
            title: "Planning {date} and {title}".to_string(),
            ..export.meeting.clone()
        };
        let options = MarkdownExportOptions {
            filename: "{date} {title} recap {title}".to_string(),
            include_id_suffix: false,
            ..Default::default()
        };
        assert_eq!(
            super::configured_markdown_filename(&literal_title, &options),
            "2026-07-13 Planning {date} and {title} recap Planning {date} and {title}.md"
        );
    }

    #[tokio::test]
    async fn configured_actions_keep_separate_exports_for_the_same_meeting() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("anlg-md-actions-{}", uuid::Uuid::new_v4()));
        let memo_options = MarkdownExportOptions {
            filename: "Memo".to_string(),
            include_transcript: false,
            ..Default::default()
        };
        let transcript_options = MarkdownExportOptions {
            filename: "Transcript".to_string(),
            include_memo: false,
            ..Default::default()
        };
        let memo =
            super::write_markdown_export_with_options(&directory, &export, Some(&memo_options))
                .unwrap();
        let transcript = super::write_markdown_export_with_options(
            &directory,
            &export,
            Some(&transcript_options),
        )
        .unwrap();
        assert!(memo.exists());
        assert!(transcript.exists());
        assert!(
            !std::fs::read_to_string(memo)
                .unwrap()
                .contains("hello world")
        );
        assert!(
            !std::fs::read_to_string(transcript)
                .unwrap()
                .contains("Launch decision")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn failed_markdown_writes_leave_no_partial_export_and_can_be_retried() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            include_id_suffix: false,
            ..Default::default()
        };
        for replace_existing in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("Recap.md");
            if replace_existing {
                std::fs::write(&path, format!("{}\n", export.to_markdown())).unwrap();
            }
            let before = std::fs::read(&path).ok();
            let error = super::persist_markdown_export(&path, replace_existing, |file| {
                file.write_all(b"# Partial")?;
                Err(std::io::Error::other("simulated write failure"))
            })
            .unwrap_err();
            assert_eq!(error.to_string(), "simulated write failure");
            assert_eq!(std::fs::read(&path).ok(), before);
            assert_eq!(
                std::fs::read_dir(directory.path()).unwrap().count(),
                usize::from(replace_existing)
            );
            super::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
            assert!(
                std::fs::read_to_string(&path)
                    .unwrap()
                    .contains("hello world")
            );
        }
    }

    #[tokio::test]
    async fn simultaneous_markdown_exports_complete_without_false_collisions() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.meeting.note.as_mut().unwrap().markdown = "Memo content. ".repeat(20_000);
        let directory = tempfile::tempdir().unwrap();
        let barrier = std::sync::Barrier::new(8);
        let options = MarkdownExportOptions::default();
        std::thread::scope(|scope| {
            let handles = (0..8)
                .map(|index| {
                    let export = &export;
                    let directory = directory.path();
                    let barrier = &barrier;
                    let options = &options;
                    scope.spawn(move || {
                        barrier.wait();
                        for _ in 0..4 {
                            super::write_markdown_export_with_options(
                                directory,
                                export,
                                (index % 2 == 0).then_some(options),
                            )
                            .unwrap();
                        }
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().unwrap();
            }
        });
        let path = directory
            .path()
            .join(super::markdown_export_filename(&export.meeting));
        let written = std::fs::read_to_string(path).unwrap();
        let content = written
            .strip_prefix("<!-- anarlog:legacy-markdown-export \"meeting-1\" -->\n\n")
            .unwrap_or(&written);
        assert_eq!(content, format!("{}\n", export.to_markdown()));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn markdown_replacement_keeps_existing_permission_bits() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let options = MarkdownExportOptions::default();
        let path =
            super::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        super::write_markdown_export_with_options(directory.path(), &export, Some(&options))
            .unwrap();
        let updated = std::fs::metadata(path).unwrap();
        assert_eq!(updated.permissions().mode() & 0o777, 0o640);
    }

    #[tokio::test]
    async fn legacy_cleanup_keeps_configured_and_unmarked_exports() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let legacy = super::write_markdown_export(directory.path(), &export).unwrap();
        let options = MarkdownExportOptions {
            filename: "Memo".to_string(),
            ..Default::default()
        };
        let configured =
            super::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
        let unmarked = directory.path().join("Older export [meeting-].md");
        std::fs::write(&unmarked, format!("{}\n", export.to_markdown())).unwrap();
        let mut other = export.clone();
        other.meeting.id = "meeting-2".to_string();
        other.meeting.title = "Another meeting".to_string();
        let same_prefix = super::write_markdown_export(directory.path(), &other).unwrap();

        export.meeting.title = "Planning updated".to_string();
        super::write_markdown_export(directory.path(), &export).unwrap();
        assert!(!legacy.exists());
        assert!(configured.exists());
        assert!(unmarked.exists());
        assert!(same_prefix.exists());
    }

    #[tokio::test]
    async fn configured_export_takes_ownership_of_a_shared_legacy_filename() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = super::write_markdown_export(directory.path(), &export).unwrap();
        super::write_markdown_export_with_options(
            directory.path(),
            &export,
            Some(&MarkdownExportOptions::default()),
        )
        .unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("<!-- anarlog:legacy-markdown-export")
        );
        export.meeting.title = "Planning updated".to_string();
        super::write_markdown_export(directory.path(), &export).unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn configured_markdown_export_updates_its_own_file_but_rejects_collisions() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("anlg-md-collision-{}", uuid::Uuid::new_v4()));
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            include_id_suffix: false,
            ..Default::default()
        };
        let path =
            super::write_markdown_export_with_options(&directory, &export, Some(&options)).unwrap();
        export.meeting.note.as_mut().unwrap().markdown = "Updated memo".to_string();
        assert_eq!(
            super::write_markdown_export_with_options(&directory, &export, Some(&options)).unwrap(),
            path
        );
        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("Updated memo"));
        export.meeting.id = "another-meeting".to_string();
        let error = super::write_markdown_export_with_options(&directory, &export, Some(&options))
            .unwrap_err();
        assert!(error.contains("already exists"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), updated);
        std::fs::write(&path, "My unrelated notes").unwrap();
        assert!(
            super::write_markdown_export_with_options(&directory, &export, Some(&options)).is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "My unrelated notes"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn markdown_reexports_multiline_titles_and_windows_line_endings() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.meeting.title = "Planning\n\nfollow-up".to_string();
        let directory = tempfile::tempdir().unwrap();
        let options = MarkdownExportOptions::default();
        for selected in [None, Some(&options)] {
            let path =
                super::write_markdown_export_with_options(directory.path(), &export, selected)
                    .unwrap();
            let content = std::fs::read_to_string(&path)
                .unwrap()
                .replace('\n', "\r\n");
            std::fs::write(&path, content).unwrap();
            export.meeting.note.as_mut().unwrap().markdown = "Updated memo".to_string();
            super::write_markdown_export_with_options(directory.path(), &export, selected).unwrap();
            assert!(
                std::fs::read_to_string(path)
                    .unwrap()
                    .contains("Updated memo")
            );
        }
    }

    #[tokio::test]
    async fn markdown_collision_checks_the_owner_before_ids_in_the_body() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join(super::markdown_export_filename(&export.meeting));
        for other in [
            "# Another meeting\n\n- ID: `meeting-2`\n- Date: 2026-07-13\n\n## Notes\n\n- ID: `meeting-1`\n",
            "# My notes\n\nThis meeting needs a follow-up:\n\n- ID: `meeting-1`\n",
        ] {
            std::fs::write(&path, other).unwrap();
            assert!(super::write_markdown_export(directory.path(), &export).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), other);
        }
    }

    #[tokio::test]
    async fn legacy_cleanup_matches_sanitized_id_suffixes() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.meeting.id = "meeting/1".to_string();
        let directory = tempfile::tempdir().unwrap();
        let original = super::write_markdown_export(directory.path(), &export).unwrap();
        export.meeting.title = "Planning updated".to_string();
        let updated = super::write_markdown_export(directory.path(), &export).unwrap();
        assert!(!original.exists());
        assert!(updated.exists());
    }

    #[tokio::test]
    async fn note_enhanced_export_skips_silently_without_configuration() {
        let pool = seeded_pool().await;

        super::run_markdown_export_automation(&pool, "meeting-1").await;

        let row: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM app_settings \
             WHERE id = 'automation_markdown_export_last_run'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn oversized_cloud_snapshot_keeps_text_and_drops_word_payloads() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.transcripts[0].words =
            vec![serde_json::json!({ "text": "x".repeat(2 * 1024 * 1024) })];
        export.transcripts[0].speaker_hints = vec![serde_json::json!({ "name": "x".repeat(1024) })];

        let snapshot = super::prepare_cloud_snapshot(export).unwrap();

        assert_eq!(snapshot["id"], "meeting-1");
        assert!(snapshot.get("meeting").is_none());
        assert_eq!(snapshot["transcripts"][0]["text"], "hello world");
        assert_eq!(snapshot["transcripts"][0]["words"], serde_json::json!([]));
        assert_eq!(
            snapshot["transcripts"][0]["speaker_hints"],
            serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn cloud_snapshot_accounts_for_jsonb_spacing_at_the_size_limit() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.transcripts[0].words = vec![serde_json::json!({ "text": "word" }); 110_000];
        let compact_len = serde_json::to_vec(&export).unwrap().len();
        assert!(compact_len < 2 * 1024 * 1024);
        let padding = 2 * 1024 * 1024 - compact_len - 1;
        export.transcripts[0].words[0]["text"] =
            serde_json::json!("word".to_string() + &"x".repeat(padding));
        assert_eq!(
            serde_json::to_vec(&export).unwrap().len(),
            2 * 1024 * 1024 - 1
        );

        let snapshot = super::prepare_cloud_snapshot(export).unwrap();

        assert_eq!(snapshot["transcripts"][0]["text"], "hello world");
        assert!(
            snapshot["transcripts"][0]["words"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn cloud_snapshot_size_matches_jsonb_separators_and_expanded_numbers() {
        for (value, expected) in [
            (serde_json::json!({"a": [1, true, "한글"]}), 26),
            (serde_json::json!({"empty": [], "object": {}}), 27),
            (serde_json::json!(1e20), 21),
            (serde_json::json!(1.23e-20), 24),
            (serde_json::json!(-1.23e20), 22),
            (serde_json::json!(1.0), 3),
            (serde_json::json!(1e-308), 310),
            (serde_json::json!(1e308), 309),
        ] {
            assert_eq!(
                super::cloud_snapshot_jsonb_len(&value).unwrap(),
                expected,
                "{value}"
            );
        }
    }

    #[tokio::test]
    async fn cloud_snapshot_within_the_jsonb_limit_preserves_word_metadata() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let expected = serde_json::to_value(&export).unwrap();

        assert_eq!(super::prepare_cloud_snapshot(export).unwrap(), expected);
    }
}
