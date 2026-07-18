use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use hypr_db_app::{
    LegacyActionItem, LegacyAppSetting, LegacyAttachment, LegacyChatGroup, LegacyChatMessage,
    LegacyDailyNote, LegacyDocument, LegacyHuman, LegacyImportBatch, LegacyImportItem,
    LegacyImportRow, LegacyOrganization, LegacyParticipant, LegacySession, LegacySessionTag,
    LegacyTag, LegacyTranscript,
};
use hypr_fs_sync_core::frontmatter::ParsedDocument;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

static IMPORT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const RECOVERED_MISSING_SESSION_METADATA: &str = "recovered_missing_session_metadata";
const RECOVERED_DUPLICATE_DOCUMENT_ID: &str = "recovered_duplicate_document_id";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceKind {
    Calendar,
    Event,
    Template,
    SessionMeta,
    SessionDocument,
    Transcript,
    Attachment,
    Human,
    Organization,
    Tasks,
    DailyNotes,
    Chat,
    Settings,
}

impl SourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Calendar => "calendar",
            Self::Event => "event",
            Self::Template => "template",
            Self::SessionMeta => "session_meta",
            Self::SessionDocument => "session_document",
            Self::Transcript => "transcript",
            Self::Attachment => "attachment",
            Self::Human => "human",
            Self::Organization => "organization",
            Self::Tasks => "tasks",
            Self::DailyNotes => "daily_notes",
            Self::Chat => "chat",
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug)]
struct SourceFile {
    path: PathBuf,
    relative_path: String,
    kind: SourceKind,
}

impl SourceFile {
    fn import_sort_key(&self) -> (&str, u8, &str) {
        let group = match self.kind {
            SourceKind::Attachment => self
                .relative_path
                .split_once("/attachments/")
                .map_or(self.relative_path.as_str(), |(directory, _)| directory),
            SourceKind::SessionMeta | SourceKind::SessionDocument | SourceKind::Transcript => self
                .relative_path
                .rsplit_once('/')
                .map_or(self.relative_path.as_str(), |(directory, _)| directory),
            _ => self.relative_path.as_str(),
        };
        let dependency_order = match self.kind {
            SourceKind::SessionMeta => 0,
            SourceKind::SessionDocument if self.relative_path.ends_with("/.md") => 2,
            _ => 1,
        };

        (group, dependency_order, self.relative_path.as_str())
    }
}

#[derive(Debug)]
struct SourceDiscovery {
    files: Vec<SourceFile>,
    summary_pairs: Vec<SummaryPair>,
}

#[derive(Debug)]
struct SummaryPair {
    hidden_relative_path: String,
    canonical_relative_path: String,
    hidden_document: LegacyDocument,
    hide_hidden_source: bool,
}

#[derive(Debug)]
struct DocumentVariant {
    document: LegacyDocument,
    source_path: String,
    target_id: String,
}

pub async fn import_legacy_vault(
    pool: &SqlitePool,
    vault_base: &Path,
    dry_run: bool,
) -> crate::Result<String> {
    let _guard = IMPORT_LOCK.lock().await;
    let run_id = uuid::Uuid::new_v4().to_string();
    let source_root = vault_base.to_string_lossy();
    hypr_db_app::begin_legacy_import_run(pool, &run_id, &source_root, dry_run).await?;

    let discovery = match discover_sources(vault_base) {
        Ok(discovery) => discovery,
        Err(error) => {
            hypr_db_app::fail_legacy_import_run(pool, &run_id, &error.to_string()).await?;
            return Err(error.into());
        }
    };
    let mut document_variants = HashMap::<String, Vec<DocumentVariant>>::new();

    for source in discovery.files {
        let bytes = match std::fs::read(&source.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let item_id = stable_id(&format!("{run_id}:{}", source.relative_path));
                hypr_db_app::record_legacy_import_error(
                    pool,
                    LegacyImportItem {
                        id: &item_id,
                        run_id: &run_id,
                        source_path: &source.relative_path,
                        source_kind: source.kind.as_str(),
                        source_sha256: "",
                    },
                    &error.to_string(),
                )
                .await?;
                continue;
            }
        };
        let source_sha256 = sha256(&bytes);
        let item_id = stable_id(&format!("{run_id}:{}", source.relative_path));
        let recheck_summary_pair = discovery.summary_pairs.iter().any(|summary| {
            summary.canonical_relative_path == source.relative_path
                || (!summary.hide_hidden_source
                    && summary.hidden_relative_path == source.relative_path)
        });

        if !dry_run
            && !recheck_summary_pair
            && source.kind != SourceKind::SessionDocument
            && hypr_db_app::legacy_source_already_imported(
                pool,
                &source.relative_path,
                &source_sha256,
            )
            .await?
        {
            hypr_db_app::record_legacy_import_unchanged(
                pool,
                LegacyImportItem {
                    id: &item_id,
                    run_id: &run_id,
                    source_path: &source.relative_path,
                    source_kind: source.kind.as_str(),
                    source_sha256: &source_sha256,
                },
            )
            .await?;
            continue;
        }

        let item = LegacyImportItem {
            id: &item_id,
            run_id: &run_id,
            source_path: &source.relative_path,
            source_kind: source.kind.as_str(),
            source_sha256: &source_sha256,
        };

        match parse_source(vault_base, &source, &bytes, &source_sha256) {
            Ok(mut batch) => {
                if !dry_run
                    && let Some(summary_pair) = discovery
                        .summary_pairs
                        .iter()
                        .find(|summary| summary.canonical_relative_path == source.relative_path)
                    && let Some(LegacyImportRow::Document(document)) = batch.rows.first()
                {
                    promote_canonical_summary(pool, summary_pair, document).await?;
                }
                if let Some(LegacyImportRow::Document(document)) = batch.rows.first_mut() {
                    recover_duplicate_document_id(
                        document,
                        &source.relative_path,
                        &source_sha256,
                        &mut document_variants,
                    );
                }
                hypr_db_app::apply_legacy_import_item(pool, item, &batch, dry_run).await?;
            }
            Err(error) => {
                hypr_db_app::record_legacy_import_error(pool, item, &error).await?;
            }
        }
    }

    hypr_db_app::finish_legacy_import_run(pool, &run_id).await?;
    Ok(run_id)
}

fn discover_sources(vault_base: &Path) -> std::io::Result<SourceDiscovery> {
    if !vault_base.exists() {
        return Ok(SourceDiscovery {
            files: Vec::new(),
            summary_pairs: Vec::new(),
        });
    }

    let mut files = Vec::new();
    let mut summary_pairs = Vec::new();
    collect_files(vault_base, vault_base, &mut files, &mut summary_pairs)?;
    files.sort_by(|left, right| left.import_sort_key().cmp(&right.import_sort_key()));
    summary_pairs.sort_by(|left, right| left.hidden_relative_path.cmp(&right.hidden_relative_path));
    Ok(SourceDiscovery {
        files,
        summary_pairs,
    })
}

fn collect_files(
    vault_base: &Path,
    directory: &Path,
    files: &mut Vec<SourceFile>,
    summary_pairs: &mut Vec<SummaryPair>,
) -> std::io::Result<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(vault_base, &path, files, summary_pairs)?;
            continue;
        }

        let relative_path = normalized_relative_path(vault_base, &path);
        if let Some(summary_pair) = summary_pair(vault_base, &path, &relative_path) {
            let hide_hidden_source = summary_pair.hide_hidden_source;
            summary_pairs.push(summary_pair);
            if hide_hidden_source {
                continue;
            }
        }
        if let Some(kind) = classify_source(&relative_path) {
            files.push(SourceFile {
                path,
                relative_path,
                kind,
            });
        }
    }

    Ok(())
}

fn summary_pair(vault_base: &Path, path: &Path, relative_path: &str) -> Option<SummaryPair> {
    if path.file_name()?.to_str()? != ".md" {
        return None;
    }

    let canonical_path = path.with_file_name("_summary.md");
    let hidden_bytes = std::fs::read(path).ok()?;
    let canonical_bytes = std::fs::read(&canonical_path).ok()?;
    let hidden_content = std::str::from_utf8(&hidden_bytes).ok()?;
    let canonical_content = std::str::from_utf8(&canonical_bytes).ok()?;
    let hidden_hash = sha256(&hidden_bytes);
    let canonical_hash = sha256(&canonical_bytes);
    let hidden = parse_session_document(vault_base, path, hidden_content, &hidden_hash).ok()?;
    let canonical = parse_session_document(
        vault_base,
        &canonical_path,
        canonical_content,
        &canonical_hash,
    )
    .ok()?;
    let Some(LegacyImportRow::Document(hidden_document)) = hidden.rows.into_iter().next() else {
        return None;
    };
    let Some(LegacyImportRow::Document(canonical_document)) = canonical.rows.into_iter().next()
    else {
        return None;
    };

    if hidden_document.id != canonical_document.id
        || hidden_document.session_id != canonical_document.session_id
    {
        return None;
    }
    let hidden_empty = legacy_document_body_is_empty(&hidden_document.body);
    let canonical_empty = legacy_document_body_is_empty(&canonical_document.body);
    let hidden_title_empty = hidden_document.title.trim().is_empty();
    let canonical_title_empty = canonical_document.title.trim().is_empty();
    let hide_hidden_source = !((!hidden_empty && canonical_empty)
        || (!hidden_empty && !canonical_empty && hidden_document.body != canonical_document.body)
        || (!hidden_title_empty && canonical_title_empty)
        || (!hidden_title_empty
            && !canonical_title_empty
            && hidden_document.title != canonical_document.title));

    Some(SummaryPair {
        hidden_relative_path: relative_path.to_string(),
        canonical_relative_path: normalized_relative_path(vault_base, &canonical_path),
        hidden_document,
        hide_hidden_source,
    })
}

async fn promote_canonical_summary(
    pool: &SqlitePool,
    summary_pair: &SummaryPair,
    canonical: &LegacyDocument,
) -> Result<(), sqlx::Error> {
    let hidden = &summary_pair.hidden_document;
    if legacy_document_body_is_empty(&canonical.body) {
        return Ok(());
    }
    sqlx::query(
        "UPDATE session_documents
         SET session_id = ?, kind = ?, template_id = ?,
             title = CASE WHEN ? THEN title ELSE ? END,
             body_format = ?, body = ?,
             source_hash = ?, sort_order = ?, created_by = ?, updated_by = ?, created_at = ?,
             updated_at = ?
         WHERE id = ?
           AND session_id IS ?
           AND kind IS ?
           AND template_id IS ?
           AND title IS ?
           AND body_format IS ?
           AND body IS ?
           AND source_hash IS ?
           AND sort_order IS ?
           AND created_by IS ?
           AND created_at IS ?
           AND updated_at IS ?
           AND deleted_at IS NULL
           AND EXISTS (
             SELECT 1 FROM storage_migration_state
             WHERE id = 'legacy_v1' AND parity_verified = 0
           )
           AND EXISTS (
             SELECT 1
             FROM migration_import_targets AS target
             JOIN migration_import_items AS item ON item.id = target.item_id
             JOIN migration_import_runs AS run ON run.id = target.run_id
             WHERE target.table_name = 'session_documents'
               AND target.target_id = session_documents.id
               AND target.source_path = ?
               AND target.status = 'inserted'
               AND item.source_sha256 = ?
               AND run.dry_run = 0
           )",
    )
    .bind(&canonical.session_id)
    .bind(&canonical.kind)
    .bind(&canonical.template_id)
    .bind(canonical.title.trim().is_empty())
    .bind(&canonical.title)
    .bind(&canonical.body_format)
    .bind(&canonical.body)
    .bind(&canonical.source_hash)
    .bind(canonical.sort_order)
    .bind(&canonical.created_by)
    .bind(&canonical.created_by)
    .bind(&canonical.created_at)
    .bind(&canonical.updated_at)
    .bind(&hidden.id)
    .bind(&hidden.session_id)
    .bind(&hidden.kind)
    .bind(&hidden.template_id)
    .bind(&hidden.title)
    .bind(&hidden.body_format)
    .bind(&hidden.body)
    .bind(&hidden.source_hash)
    .bind(hidden.sort_order)
    .bind(&hidden.created_by)
    .bind(&hidden.created_at)
    .bind(&hidden.updated_at)
    .bind(&summary_pair.hidden_relative_path)
    .bind(&hidden.source_hash)
    .execute(pool)
    .await?;

    Ok(())
}

fn legacy_document_body_is_empty(body: &str) -> bool {
    let body = body.trim();
    body.is_empty() || body == "&nbsp;"
}

fn recover_duplicate_document_id(
    document: &mut LegacyDocument,
    source_path: &str,
    source_sha256: &str,
    variants_by_id: &mut HashMap<String, Vec<DocumentVariant>>,
) {
    let original_id = document.id.clone();
    let variants = variants_by_id.entry(original_id.clone()).or_default();

    if let Some(variant) = variants
        .iter()
        .find(|variant| documents_share_variant(&variant.document, document))
    {
        document.id.clone_from(&variant.target_id);
        return;
    }

    if let Some(conflicting_source) = variants.first() {
        let recovered_id = stable_id(&format!(
            "legacy-recovered-document:{original_id}:{source_path}:{source_sha256}"
        ));
        document.id.clone_from(&recovered_id);
        document.generation_metadata_json = serde_json::json!({
            "legacy_recovery": {
                "reason": "duplicate_document_id",
                "original_id": original_id,
                "source_path": source_path,
                "source_sha256": source_sha256,
                "conflicting_source_path": conflicting_source.source_path,
            }
        })
        .to_string();
        document.recovery_status = Some(RECOVERED_DUPLICATE_DOCUMENT_ID.to_string());
        variants.push(DocumentVariant {
            document: document.clone(),
            source_path: source_path.to_string(),
            target_id: recovered_id,
        });
        return;
    }

    variants.push(DocumentVariant {
        document: document.clone(),
        source_path: source_path.to_string(),
        target_id: original_id,
    });
}

fn documents_share_variant(left: &LegacyDocument, right: &LegacyDocument) -> bool {
    left.session_id == right.session_id
        && left.kind == right.kind
        && left.template_id == right.template_id
        && left.body_format == right.body_format
        && (left.body == right.body
            || legacy_document_body_is_empty(&left.body)
            || legacy_document_body_is_empty(&right.body))
        && (left.title == right.title
            || left.title.trim().is_empty()
            || right.title.trim().is_empty())
}

fn classify_source(relative_path: &str) -> Option<SourceKind> {
    let parts = relative_path.split('/').collect::<Vec<_>>();
    let filename = parts.last().copied()?;

    match parts.as_slice() {
        ["calendars.json"] => return Some(SourceKind::Calendar),
        ["events.json"] => return Some(SourceKind::Event),
        ["templates.json"] => return Some(SourceKind::Template),
        ["tasks.json"] => return Some(SourceKind::Tasks),
        ["daily_notes.json"] => return Some(SourceKind::DailyNotes),
        ["settings.json"] => return Some(SourceKind::Settings),
        _ => {}
    }

    match parts.first().copied() {
        Some("humans") if filename.ends_with(".md") => Some(SourceKind::Human),
        Some("organizations") if filename.ends_with(".md") => Some(SourceKind::Organization),
        Some("chats") if filename == "messages.json" => Some(SourceKind::Chat),
        Some("sessions") if parts.contains(&"attachments") => Some(SourceKind::Attachment),
        Some("sessions") if filename == "_meta.json" => Some(SourceKind::SessionMeta),
        Some("sessions") if filename == "transcript.json" => Some(SourceKind::Transcript),
        Some("sessions") if filename.ends_with(".md") => Some(SourceKind::SessionDocument),
        _ => None,
    }
}

fn parse_source(
    vault_base: &Path,
    source: &SourceFile,
    bytes: &[u8],
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    match source.kind {
        SourceKind::Attachment => parse_attachment(vault_base, source, bytes, source_sha256),
        _ => {
            let content = std::str::from_utf8(bytes)
                .map_err(|error| format!("source is not valid UTF-8: {error}"))?;
            match source.kind {
                SourceKind::Calendar => super::calendars::parse_legacy_calendars(content),
                SourceKind::Event => super::events::parse_legacy_events(content),
                SourceKind::Template => super::templates::parse_legacy_templates(content),
                SourceKind::SessionMeta => parse_session_meta(vault_base, &source.path, content),
                SourceKind::SessionDocument => {
                    parse_session_document(vault_base, &source.path, content, source_sha256)
                }
                SourceKind::Transcript => {
                    parse_transcript(vault_base, &source.path, content, source_sha256)
                }
                SourceKind::Human => parse_human(&source.path, content),
                SourceKind::Organization => parse_organization(&source.path, content),
                SourceKind::Tasks => parse_tasks(content),
                SourceKind::DailyNotes => parse_daily_notes(content),
                SourceKind::Chat => parse_chat(&source.path, content),
                SourceKind::Settings => parse_settings(content),
                SourceKind::Attachment => unreachable!(),
            }
        }
    }
}

fn parse_session_meta(
    vault_base: &Path,
    path: &Path,
    content: &str,
) -> Result<LegacyImportBatch, String> {
    let meta = parse_json_object(content)?;
    let (session_id, folder_path) = infer_session_id_and_folder(vault_base, path)?;
    let mut batch = LegacyImportBatch::default();

    if let Some(meta_id) = value_string(meta.get("id"))
        && meta_id != session_id
    {
        append_warning(
            &mut batch,
            format!("metadata id {meta_id} differs from folder id {session_id}"),
        );
    }

    let event = meta.get("event").and_then(Value::as_object);
    let event_json = event
        .map(|event| Value::Object(event.clone()).to_string())
        .unwrap_or_default();
    let event_id = value_string(meta.get("event_id")).unwrap_or_default();
    let external_event_id = event
        .and_then(|event| value_string(event.get("tracking_id")))
        .unwrap_or_default();
    let started_at = event
        .and_then(|event| value_string(event.get("started_at")))
        .unwrap_or_default();
    let ended_at = event
        .and_then(|event| value_string(event.get("ended_at")))
        .unwrap_or_default();
    let series_id = event
        .and_then(|event| value_string(event.get("recurrence_series_id")))
        .unwrap_or_default();
    let stored_title = value_string(meta.get("title")).unwrap_or_default();
    let title = if stored_title.trim().is_empty() {
        recover_title_from_summary(path).unwrap_or(stored_title)
    } else {
        stored_title
    };

    batch.rows.push(LegacyImportRow::Session(LegacySession {
        id: session_id.clone(),
        owner_user_id: value_string(meta.get("user_id")).unwrap_or_default(),
        title,
        created_at: value_string(meta.get("created_at")).unwrap_or_default(),
        started_at,
        ended_at,
        event_id,
        external_event_id,
        external_provider: String::new(),
        series_id,
        event_json,
        metadata_json: "{}".to_string(),
        folder_path,
        recovery_status: None,
    }));

    if let Some(participants) = meta.get("participants").and_then(Value::as_array) {
        for (index, participant) in participants.iter().enumerate() {
            let Some(participant) = participant.as_object() else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("participant {index} is not an object"));
                continue;
            };
            let human_id = value_string(participant.get("human_id")).unwrap_or_default();
            if human_id.is_empty() {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("participant {index} has no human_id"));
                continue;
            }
            let id = value_string(participant.get("id")).unwrap_or_else(|| {
                stable_id(&format!("{session_id}:participant:{human_id}:{index}"))
            });
            batch
                .rows
                .push(LegacyImportRow::Participant(LegacyParticipant {
                    id,
                    owner_user_id: value_string(participant.get("user_id")).unwrap_or_default(),
                    session_id: session_id.clone(),
                    human_id,
                    source: value_string(participant.get("source")).unwrap_or_default(),
                }));
        }
    }

    if let Some(key_facts) = meta.get("key_facts").and_then(Value::as_object) {
        let body = value_string(key_facts.get("content")).unwrap_or_default();
        let source_hash = value_string(key_facts.get("source_hash")).unwrap_or_default();
        if !body.is_empty() || !source_hash.is_empty() {
            batch.rows.push(LegacyImportRow::Document(LegacyDocument {
                id: format!("{session_id}:key_facts"),
                session_id: session_id.clone(),
                kind: "key_facts".to_string(),
                template_id: String::new(),
                title: "Key facts".to_string(),
                body_format: "markdown".to_string(),
                body,
                source_hash,
                sort_order: 0,
                created_by: value_string(key_facts.get("user_id"))
                    .or_else(|| value_string(meta.get("user_id")))
                    .unwrap_or_default(),
                created_at: value_string(key_facts.get("created_at")).unwrap_or_default(),
                updated_at: value_string(key_facts.get("updated_at")).unwrap_or_default(),
                generation_metadata_json: "{}".to_string(),
                recovery_status: None,
            }));
        }
    }

    if let Some(tags) = meta.get("tags").and_then(Value::as_array) {
        let owner_user_id = value_string(meta.get("user_id")).unwrap_or_default();
        for (index, tag) in tags.iter().enumerate() {
            let Some(name) = value_string(Some(tag)) else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("tag {index} is not a string"));
                continue;
            };
            if name.is_empty() {
                continue;
            }
            batch.rows.push(LegacyImportRow::Tag(LegacyTag {
                id: name.clone(),
                owner_user_id: owner_user_id.clone(),
                name: name.clone(),
            }));
            batch
                .rows
                .push(LegacyImportRow::SessionTag(LegacySessionTag {
                    id: format!("{session_id}:{name}"),
                    owner_user_id: owner_user_id.clone(),
                    session_id: session_id.clone(),
                    tag_id: name,
                }));
        }
    }

    Ok(batch)
}

fn recover_title_from_summary(meta_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(meta_path.with_file_name("_summary.md")).ok()?;
    let document = ParsedDocument::from_str(&content).ok()?;
    let title = document
        .content
        .trim_start()
        .lines()
        .next()?
        .strip_prefix("# ")?
        .trim();

    if title.is_empty()
        || matches!(
            title.to_ascii_lowercase().as_str(),
            "summary" | "untitled" | "untitled note"
        )
    {
        return None;
    }

    Some(title.to_string())
}

fn parse_session_document(
    vault_base: &Path,
    path: &Path,
    content: &str,
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    let inferred_session_id = infer_session_id_and_folder(vault_base, path)?.0;
    let session_id = value_string(document.frontmatter.get("session_id"))
        .filter(|id| !id.is_empty())
        .unwrap_or(inferred_session_id);
    if session_id.is_empty() {
        return Err("document has no session id".to_string());
    }

    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let template_id = value_string(document.frontmatter.get("template_id")).unwrap_or_default();
    let kind = if filename == "_memo.md" {
        "note"
    } else if filename == "_summary.md" {
        "summary"
    } else if !template_id.is_empty() {
        "template_output"
    } else {
        "summary"
    };
    let id = value_string(document.frontmatter.get("id"))
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| stable_id(&format!("{session_id}:document:{filename}")));

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Document(LegacyDocument {
            id,
            session_id,
            kind: kind.to_string(),
            template_id,
            title: value_string(document.frontmatter.get("title")).unwrap_or_default(),
            body_format: "markdown".to_string(),
            body: document.content,
            source_hash: source_sha256.to_string(),
            sort_order: value_i64(document.frontmatter.get("position")).unwrap_or(0),
            created_by: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            generation_metadata_json: "{}".to_string(),
            recovery_status: None,
        })],
        ..Default::default()
    })
}

fn parse_transcript(
    vault_base: &Path,
    path: &Path,
    content: &str,
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let root = parse_json_object(content)?;
    let transcripts = root
        .get("transcripts")
        .and_then(Value::as_array)
        .ok_or_else(|| "transcript file has no transcripts array".to_string())?;
    let (inferred_session_id, folder_path) = infer_session_id_and_folder(vault_base, path)?;
    let source_path = normalized_relative_path(vault_base, path);
    let missing_session_metadata = !path.with_file_name("_meta.json").is_file();
    let recovery_metadata_json = missing_session_metadata.then(|| {
        serde_json::json!({
            "legacy_recovery": {
                "reason": "missing_session_metadata",
                "source_path": source_path,
                "source_sha256": source_sha256,
            }
        })
        .to_string()
    });
    let mut batch = LegacyImportBatch::default();
    let mut recovered_sessions = HashMap::<String, (String, String)>::new();

    for (index, transcript) in transcripts.iter().enumerate() {
        let Some(transcript) = transcript.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("transcript {index} is not an object"));
            continue;
        };
        let session_id = value_string(transcript.get("session_id"))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| inferred_session_id.clone());
        if session_id.is_empty() {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("transcript {index} has no session id"));
            continue;
        }
        let id = value_string(transcript.get("id"))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| stable_id(&format!("{session_id}:transcript:{index}")));
        let mut words = transcript
            .get("words")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (word_index, word) in words.iter_mut().enumerate() {
            let Some(word) = word.as_object_mut() else {
                append_warning(
                    &mut batch,
                    format!("transcript {index} word {word_index} is not an object"),
                );
                continue;
            };
            let has_id = word
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty());
            if !has_id {
                word.insert(
                    "id".to_string(),
                    Value::String(format!("{id}:word:{word_index}")),
                );
            }
        }

        let speaker_hints = transcript
            .get("speaker_hints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let owner_user_id = value_string(transcript.get("user_id")).unwrap_or_default();
        let created_at = value_string(transcript.get("created_at")).unwrap_or_default();
        if missing_session_metadata {
            let recovered = recovered_sessions
                .entry(session_id.clone())
                .or_insert_with(|| (owner_user_id.clone(), created_at.clone()));
            if recovered.0.is_empty() && !owner_user_id.is_empty() {
                recovered.0.clone_from(&owner_user_id);
            }
            if !created_at.is_empty() && (recovered.1.is_empty() || created_at < recovered.1) {
                recovered.1.clone_from(&created_at);
            }
        }
        batch
            .rows
            .push(LegacyImportRow::Transcript(LegacyTranscript {
                id,
                owner_user_id,
                session_id: session_id.clone(),
                started_at_ms: value_i64(transcript.get("started_at")).unwrap_or(0),
                ended_at_ms: value_i64(transcript.get("ended_at")),
                memo: value_string(transcript.get("memo_md")).unwrap_or_default(),
                words_json: Value::Array(words).to_string(),
                speaker_hints_json: Value::Array(speaker_hints).to_string(),
                created_at,
                metadata_json: recovery_metadata_json
                    .clone()
                    .unwrap_or_else(|| "{}".to_string()),
                recovery_status: missing_session_metadata
                    .then(|| RECOVERED_MISSING_SESSION_METADATA.to_string()),
            }));
    }

    if let Some(metadata_json) = recovery_metadata_json {
        let mut sessions = recovered_sessions.into_iter().collect::<Vec<_>>();
        sessions.sort_by(|left, right| left.0.cmp(&right.0));
        let mut rows = sessions
            .into_iter()
            .map(|(session_id, (owner_user_id, created_at))| {
                LegacyImportRow::Session(LegacySession {
                    id: session_id.clone(),
                    owner_user_id,
                    title: recovered_transcript_title(&session_id, &created_at),
                    created_at,
                    started_at: String::new(),
                    ended_at: String::new(),
                    event_id: String::new(),
                    external_event_id: String::new(),
                    external_provider: String::new(),
                    series_id: String::new(),
                    event_json: String::new(),
                    metadata_json: metadata_json.clone(),
                    folder_path: folder_path.clone(),
                    recovery_status: Some(RECOVERED_MISSING_SESSION_METADATA.to_string()),
                })
            })
            .collect::<Vec<_>>();
        rows.append(&mut batch.rows);
        batch.rows = rows;
    }

    Ok(batch)
}

fn recovered_transcript_title(session_id: &str, created_at: &str) -> String {
    let date = created_at.get(..10).filter(|date| {
        date.as_bytes().iter().enumerate().all(|(index, byte)| {
            matches!(index, 4 | 7) && *byte == b'-'
                || !matches!(index, 4 | 7) && byte.is_ascii_digit()
        })
    });
    match date {
        Some(date) => format!("Recovered transcript — {date}"),
        None => format!("Recovered transcript — {session_id}"),
    }
}

fn parse_attachment(
    vault_base: &Path,
    source: &SourceFile,
    bytes: &[u8],
    source_sha256: &str,
) -> Result<LegacyImportBatch, String> {
    let parts = source.relative_path.split('/').collect::<Vec<_>>();
    let attachment_index = parts
        .iter()
        .position(|part| *part == "attachments")
        .ok_or_else(|| "attachment path has no attachments directory".to_string())?;
    if attachment_index < 2 {
        return Err("attachment path has no session directory".to_string());
    }
    let session_id = parts[attachment_index - 1].to_string();
    let filename = source
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "attachment filename is not valid UTF-8".to_string())?
        .to_string();
    let relative_path = normalized_relative_path(vault_base, &source.path);

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Attachment(LegacyAttachment {
            id: stable_id(&format!("{session_id}:attachment:{relative_path}")),
            session_id,
            filename: filename.clone(),
            relative_path,
            content_type: content_type_for_path(&source.path).to_string(),
            size_bytes: i64::try_from(bytes.len()).unwrap_or(i64::MAX),
            sha256: source_sha256.to_string(),
            source_id: filename,
        })],
        ..Default::default()
    })
}

fn parse_human(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    let id = file_stem(path)?;
    let emails = match document.frontmatter.get("emails") {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        _ => value_string(document.frontmatter.get("email")).unwrap_or_default(),
    };

    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Human(LegacyHuman {
            id,
            owner_user_id: value_string(document.frontmatter.get("user_id")).unwrap_or_default(),
            organization_id: value_string(document.frontmatter.get("org_id")).unwrap_or_default(),
            name: value_string(document.frontmatter.get("name")).unwrap_or_default(),
            email: emails,
            phone: value_string(document.frontmatter.get("phone")).unwrap_or_default(),
            job_title: value_string(document.frontmatter.get("job_title")).unwrap_or_default(),
            linkedin_username: value_string(document.frontmatter.get("linkedin_username"))
                .unwrap_or_default(),
            memo: document.content.trim().to_string(),
            pinned: value_bool(document.frontmatter.get("pinned")),
            pin_order: value_i64(document.frontmatter.get("pin_order")),
            created_at: value_string(document.frontmatter.get("created_at")).unwrap_or_default(),
        })],
        ..Default::default()
    })
}

fn parse_organization(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let document = ParsedDocument::from_str(content).map_err(|error| error.to_string())?;
    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::Organization(LegacyOrganization {
            id: file_stem(path)?,
            owner_user_id: value_string(document.frontmatter.get("user_id")).unwrap_or_default(),
            name: value_string(document.frontmatter.get("name")).unwrap_or_default(),
            memo: document.content.trim().to_string(),
            pinned: value_bool(document.frontmatter.get("pinned")),
            pin_order: value_i64(document.frontmatter.get("pin_order")),
            created_at: value_string(document.frontmatter.get("created_at")).unwrap_or_default(),
        })],
        ..Default::default()
    })
}

fn parse_tasks(content: &str) -> Result<LegacyImportBatch, String> {
    let table = parse_json_object(content)?;
    let mut batch = LegacyImportBatch::default();
    for (row_id, row) in table {
        let Some(row) = row.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("task {row_id} is not an object"));
            continue;
        };
        let id = value_string(row.get("task_id"))
            .filter(|id| !id.is_empty())
            .unwrap_or(row_id);
        let source_type = value_string(row.get("source_type")).unwrap_or_default();
        let source_id = value_string(row.get("source_id")).unwrap_or_default();
        let body = row
            .get("body")
            .cloned()
            .or_else(|| row.get("body_json").cloned())
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let body_json = match body {
            Value::String(value) if serde_json::from_str::<Value>(&value).is_ok() => value,
            value => value.to_string(),
        };
        batch
            .rows
            .push(LegacyImportRow::ActionItem(LegacyActionItem {
                id,
                owner_user_id: value_string(row.get("user_id")).unwrap_or_default(),
                session_id: if source_type == "session" {
                    source_id.clone()
                } else {
                    String::new()
                },
                source_type,
                source_id,
                source_order: value_i64(row.get("source_order"))
                    .or_else(|| value_i64(row.get("order")))
                    .unwrap_or(0),
                status: value_string(row.get("status")).unwrap_or_else(|| "todo".to_string()),
                text: value_string(row.get("text_preview"))
                    .or_else(|| value_string(row.get("text")))
                    .unwrap_or_default(),
                body_json,
                due_at: value_string(row.get("due_date")).unwrap_or_default(),
            }));
    }
    Ok(batch)
}

fn parse_daily_notes(content: &str) -> Result<LegacyImportBatch, String> {
    let table = parse_json_object(content)?;
    let mut batch = LegacyImportBatch::default();
    for (id, row) in table {
        let Some(row) = row.as_object() else {
            batch.skipped_count += 1;
            append_warning(&mut batch, format!("daily note {id} is not an object"));
            continue;
        };
        batch.rows.push(LegacyImportRow::DailyNote(LegacyDailyNote {
            id,
            owner_user_id: value_string(row.get("user_id")).unwrap_or_default(),
            note_date: value_string(row.get("date")).unwrap_or_default(),
            body_format: "prosemirror_json".to_string(),
            body: value_string(row.get("content")).unwrap_or_default(),
        }));
    }
    Ok(batch)
}

fn parse_chat(path: &Path, content: &str) -> Result<LegacyImportBatch, String> {
    let root = parse_json_object(content)?;
    let group = root
        .get("chat_group")
        .and_then(Value::as_object)
        .ok_or_else(|| "chat file has no chat_group object".to_string())?;
    let inferred_group_id = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let group_id = value_string(group.get("id"))
        .filter(|id| !id.is_empty())
        .unwrap_or(inferred_group_id);
    if group_id.is_empty() {
        return Err("chat group has no id".to_string());
    }

    let mut batch = LegacyImportBatch::default();
    batch.rows.push(LegacyImportRow::ChatGroup(LegacyChatGroup {
        id: group_id.clone(),
        owner_user_id: value_string(group.get("user_id")).unwrap_or_default(),
        title: value_string(group.get("title")).unwrap_or_default(),
        created_at: value_string(group.get("created_at")).unwrap_or_default(),
    }));

    if let Some(messages) = root.get("messages").and_then(Value::as_array) {
        for (index, message) in messages.iter().enumerate() {
            let Some(message) = message.as_object() else {
                batch.skipped_count += 1;
                append_warning(&mut batch, format!("chat message {index} is not an object"));
                continue;
            };
            let id = value_string(message.get("id"))
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| stable_id(&format!("{group_id}:message:{index}")));
            batch
                .rows
                .push(LegacyImportRow::ChatMessage(LegacyChatMessage {
                    id,
                    chat_group_id: value_string(message.get("chat_group_id"))
                        .filter(|id| !id.is_empty())
                        .unwrap_or_else(|| group_id.clone()),
                    owner_user_id: value_string(message.get("user_id")).unwrap_or_default(),
                    role: value_string(message.get("role")).unwrap_or_default(),
                    content: value_string(message.get("content")).unwrap_or_default(),
                    metadata_json: json_field(message.get("metadata"), "{}"),
                    parts_json: json_field(message.get("parts"), "[]"),
                    status: value_string(message.get("status"))
                        .unwrap_or_else(|| "ready".to_string()),
                    created_at: value_string(message.get("created_at")).unwrap_or_default(),
                }));
        }
    }

    Ok(batch)
}

fn parse_settings(content: &str) -> Result<LegacyImportBatch, String> {
    serde_json::from_str::<Value>(content).map_err(|error| error.to_string())?;
    Ok(LegacyImportBatch {
        rows: vec![LegacyImportRow::AppSetting(LegacyAppSetting {
            id: "legacy_settings_document".to_string(),
            value_json: content.to_string(),
        })],
        ..Default::default()
    })
}

fn parse_json_object(content: &str) -> Result<Map<String, Value>, String> {
    serde_json::from_str::<Value>(content)
        .map_err(|error| error.to_string())?
        .as_object()
        .cloned()
        .ok_or_else(|| "JSON root is not an object".to_string())
}

fn infer_session_id_and_folder(vault_base: &Path, path: &Path) -> Result<(String, String), String> {
    let relative = path
        .strip_prefix(vault_base)
        .map_err(|_| "session source is outside the vault".to_string())?;
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if parts.first().map(String::as_str) != Some("sessions") || parts.len() < 3 {
        return Err("invalid session source path".to_string());
    }
    let session_id = parts[parts.len() - 2].clone();
    let folder_path = parts[1..parts.len() - 2].join("/");
    Ok((session_id, folder_path))
}

fn normalized_relative_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn file_stem(path: &Path) -> Result<String, String> {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "source filename has no valid id".to_string())
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToString::to_string)
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|number| number.round() as i64))
}

fn value_bool(value: Option<&Value>) -> bool {
    value.and_then(Value::as_bool).unwrap_or(false)
}

fn json_field(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(value)) if serde_json::from_str::<Value>(value).is_ok() => value.clone(),
        Some(value) => value.to_string(),
        None => fallback.to_string(),
    }
}

fn append_warning(batch: &mut LegacyImportBatch, warning: String) {
    if !batch.warning.is_empty() {
        batch.warning.push_str("; ");
    }
    batch.warning.push_str(&warning);
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

fn stable_id(value: &str) -> String {
    sha256(value.as_bytes())
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("aac") => "audio/aac",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("webm") => "audio/webm",
        Some("gif") => "image/gif",
        Some("jpeg" | "jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("md") => "text/markdown",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypr_db_core::Db;

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        hypr_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    async fn row_count(db: &Db, query: &'static str) -> i64 {
        sqlx::query_scalar::<_, i64>(query)
            .fetch_one(db.pool())
            .await
            .unwrap()
    }

    #[test]
    fn transcript_import_synthesizes_stable_word_ids_without_reordering() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sessions/session-1/transcript.json");
        let batch = parse_transcript(
            dir.path(),
            &path,
            r#"{
              "transcripts": [{
                "id": "transcript-1",
                "session_id": "session-1",
                "started_at": 10.4,
                "words": [
                  {"text":"hello","start_ms":10,"end_ms":20,"channel":0},
                  {"id":"word-existing","text":"world","start_ms":21,"end_ms":30,"channel":0}
                ],
                "speaker_hints": []
              }]
            }"#,
            "transcript-hash",
        )
        .unwrap();

        let transcript = batch
            .rows
            .iter()
            .find_map(|row| match row {
                LegacyImportRow::Transcript(transcript) => Some(transcript),
                _ => None,
            })
            .expect("expected transcript");
        assert_eq!(transcript.started_at_ms, 10);
        let words: Vec<Value> = serde_json::from_str(&transcript.words_json).unwrap();
        assert_eq!(words[0]["id"], "transcript-1:word:0");
        assert_eq!(words[1]["id"], "word-existing");
        assert_eq!(words[0]["text"], "hello");
        assert_eq!(words[1]["text"], "world");
    }

    #[tokio::test]
    async fn orphan_transcript_recovers_a_visible_session_losslessly_and_idempotently() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let transcript_path = session_dir.join("transcript.json");
        std::fs::write(
            &transcript_path,
            r#"{"transcripts":[{"id":"transcript-1","user_id":"user-1","session_id":"session-1","created_at":"2026-07-11T09:30:00Z","started_at":10,"ended_at":30,"memo_md":"Recovered memo","words":[{"text":"hello","start_ms":10,"end_ms":20,"channel":0},{"text":"again","start_ms":21,"end_ms":30,"channel":0}],"speaker_hints":[]}]}"#,
        )
        .unwrap();
        let source_before = std::fs::read(&transcript_path).unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let run: (String, i64, i64) = sqlx::query_as(
            "SELECT status, imported_count, conflict_count FROM migration_import_runs WHERE id = ?",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(run, ("completed".to_string(), 2, 0));
        let session: (String, String) =
            sqlx::query_as("SELECT title, metadata_json FROM sessions WHERE id = 'session-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(session.0, "Recovered transcript — 2026-07-11");
        assert_eq!(
            serde_json::from_str::<Value>(&session.1).unwrap()["legacy_recovery"]["reason"],
            "missing_session_metadata"
        );
        let transcript: (String, String, String) = sqlx::query_as(
            "SELECT memo, words_json, metadata_json FROM transcripts WHERE id = 'transcript-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(transcript.0, "Recovered memo");
        assert_eq!(
            serde_json::from_str::<Vec<Value>>(&transcript.1)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            serde_json::from_str::<Value>(&transcript.2).unwrap()["legacy_recovery"]["reason"],
            "missing_session_metadata"
        );
        let targets: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
        )
        .bind(&run_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            targets,
            vec![
                (
                    "sessions".to_string(),
                    RECOVERED_MISSING_SESSION_METADATA.to_string()
                ),
                (
                    "transcripts".to_string(),
                    RECOVERED_MISSING_SESSION_METADATA.to_string()
                ),
            ]
        );
        assert!(
            sqlx::query_scalar::<_, bool>(
                "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap()
        );

        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        assert_eq!(row_count(&db, "SELECT COUNT(*) FROM sessions").await, 1);
        assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 1);
        assert_eq!(std::fs::read(&transcript_path).unwrap(), source_before);
    }

    #[tokio::test]
    async fn divergent_duplicate_document_ids_are_recovered_with_provenance() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Planning"}"#,
        )
        .unwrap();
        let first = "---\nid: shared-document\nsession_id: session-1\n---\n\nFirst body";
        let second = "---\nid: shared-document\nsession_id: session-1\n---\n\nSecond body";
        std::fs::write(session_dir.join("a.md"), first).unwrap();
        std::fs::write(session_dir.join("b.md"), second).unwrap();
        let recovered_id = stable_id(&format!(
            "legacy-recovered-document:shared-document:sessions/session-1/b.md:{}",
            sha256(second.as_bytes())
        ));

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let documents: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, body, generation_metadata_json FROM session_documents ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(documents.len(), 2);
        assert_eq!(
            documents
                .iter()
                .find(|document| document.0 == "shared-document")
                .unwrap()
                .1,
            "First body"
        );
        let recovered = documents
            .iter()
            .find(|document| document.0 == recovered_id)
            .unwrap();
        assert_eq!(recovered.1, "Second body");
        let provenance = serde_json::from_str::<Value>(&recovered.2).unwrap();
        assert_eq!(
            provenance["legacy_recovery"]["reason"],
            "duplicate_document_id"
        );
        assert_eq!(
            provenance["legacy_recovery"]["original_id"],
            "shared-document"
        );
        let recovered_status: String = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets WHERE run_id = ? AND target_id = ?",
        )
        .bind(&run_id)
        .bind(&recovered_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(recovered_status, RECOVERED_DUPLICATE_DOCUMENT_ID);
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT status FROM migration_import_runs WHERE id = ?",
            )
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            "completed"
        );

        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        assert_eq!(
            row_count(&db, "SELECT COUNT(*) FROM session_documents").await,
            2
        );
    }

    #[tokio::test]
    async fn identical_duplicate_document_ids_do_not_fork() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Planning"}"#,
        )
        .unwrap();
        let document = "---\nid: shared-document\nsession_id: session-1\n---\n\nSame body";
        std::fs::write(session_dir.join("a.md"), document).unwrap();
        std::fs::write(session_dir.join("b.md"), document).unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        assert_eq!(
            row_count(&db, "SELECT COUNT(*) FROM session_documents").await,
            1
        );
        let run: (String, i64) =
            sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(run, ("completed".to_string(), 0));
    }

    #[tokio::test]
    async fn canonical_session_wins_when_an_orphan_transcript_is_recovered() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta_path = session_dir.join("_meta.json");
        std::fs::write(
            &meta_path,
            r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Canonical title"}"#,
        )
        .unwrap();
        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        std::fs::remove_file(meta_path).unwrap();
        std::fs::write(
            session_dir.join("transcript.json"),
            r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","created_at":"2026-07-12T09:30:00Z","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = 'session-1'")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            "Canonical title"
        );
        assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 1);
        let targets: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
        )
        .bind(&run_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            targets,
            vec![
                ("sessions".to_string(), "retained_existing".to_string()),
                (
                    "transcripts".to_string(),
                    RECOVERED_MISSING_SESSION_METADATA.to_string()
                ),
            ]
        );
    }

    #[tokio::test]
    async fn canonical_metadata_replaces_a_recovered_placeholder() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("transcript.json"),
            r#"{"transcripts":[{"id":"transcript-1","user_id":"transcript-user","session_id":"session-1","created_at":"2026-07-11T09:30:00Z","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
        )
        .unwrap();
        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","user_id":"canonical-user","created_at":"2026-07-10T08:00:00Z","title":"Canonical title"}"#,
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let session: (String, String, String, String) = sqlx::query_as(
            "SELECT owner_user_id, title, created_at, metadata_json FROM sessions WHERE id = 'session-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            session,
            (
                "canonical-user".to_string(),
                "Canonical title".to_string(),
                "2026-07-10T08:00:00Z".to_string(),
                "{}".to_string(),
            )
        );
        let target_status: String = sqlx::query_scalar(
            "SELECT status FROM migration_import_targets WHERE run_id = ? AND source_kind = 'session_meta'",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(target_status, "filled_from_legacy");
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT status FROM migration_import_runs WHERE id = ?",
            )
            .bind(&run_id)
            .fetch_one(db.pool())
            .await
            .unwrap(),
            "completed"
        );
    }

    #[tokio::test]
    async fn deleted_session_is_not_reused_for_an_orphan_transcript() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta_path = session_dir.join("_meta.json");
        std::fs::write(
            &meta_path,
            r#"{"id":"session-1","created_at":"2026-07-11T09:30:00Z","title":"Deleted"}"#,
        )
        .unwrap();
        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE sessions SET deleted_at = '2026-07-12T00:00:00Z' WHERE id = 'session-1'",
        )
        .execute(db.pool())
        .await
        .unwrap();
        std::fs::remove_file(meta_path).unwrap();
        let transcript_path = session_dir.join("transcript.json");
        std::fs::write(
            &transcript_path,
            r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","words":[{"text":"hello"}],"speaker_hints":[]}]}"#,
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        assert_eq!(row_count(&db, "SELECT COUNT(*) FROM transcripts").await, 0);
        assert!(transcript_path.is_file());
        let run: (String, i64) =
            sqlx::query_as("SELECT status, skipped_count FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(run, ("completed_with_issues".to_string(), 2));
        let target_statuses: Vec<(String, String)> = sqlx::query_as(
            "SELECT table_name, status FROM migration_import_targets WHERE run_id = ? ORDER BY table_name",
        )
        .bind(&run_id)
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            target_statuses,
            vec![
                ("sessions".to_string(), "missing_dependency".to_string()),
                ("transcripts".to_string(), "missing_dependency".to_string()),
            ]
        );
        assert!(
            !sqlx::query_scalar::<_, bool>(
                "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
            )
            .fetch_one(db.pool())
            .await
            .unwrap()
        );
    }

    #[tokio::test]
    async fn malformed_sources_are_reported_without_aborting_other_imports() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tasks.json"), "{not-json").unwrap();
        std::fs::write(
            dir.path().join("daily_notes.json"),
            r#"{"daily-1":{"user_id":"user-1","date":"2026-07-10","content":"{}"}}"#,
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let status: String =
            sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(status, "completed_with_issues");
        let error: String = sqlx::query_scalar(
            "SELECT error FROM migration_import_items \
             WHERE run_id = ? AND source_path = 'tasks.json'",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert!(!error.is_empty());
        assert_eq!(row_count(&db, "SELECT COUNT(*) FROM daily_notes").await, 1);
    }

    #[tokio::test]
    async fn nonempty_canonical_summary_shadows_empty_hidden_artifact() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
        )
        .unwrap();
        std::fs::write(
            session_dir.join(".md"),
            "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\n",
        )
        .unwrap();
        std::fs::write(
            session_dir.join("_summary.md"),
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCurrent summary",
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let status: String =
            sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let hidden_item_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM migration_import_items \
             WHERE run_id = ? AND source_path = 'sessions/session-1/.md'",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(status, "completed");
        assert_eq!(body, "Current summary");
        assert_eq!(hidden_item_count, 0);
    }

    #[tokio::test]
    async fn nonempty_hidden_summary_is_not_shadowed_by_empty_canonical_summary() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
        )
        .unwrap();
        std::fs::write(
            session_dir.join(".md"),
            "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\nKeep this summary",
        )
        .unwrap();
        std::fs::write(
            session_dir.join("_summary.md"),
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\n",
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let run: (String, i64) =
            sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let body: String =
            sqlx::query_scalar("SELECT body FROM session_documents WHERE id = 'summary-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        let hidden_item_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM migration_import_items
             WHERE run_id = ? AND source_path = 'sessions/session-1/.md'",
        )
        .bind(&run_id)
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(run, ("completed".to_string(), 0));
        assert_eq!(body, "Keep this summary");
        assert_eq!(hidden_item_count, 1);
    }

    #[tokio::test]
    async fn divergent_summaries_preserve_both_documents_without_conflict() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#,
        )
        .unwrap();
        std::fs::write(
            session_dir.join(".md"),
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nHidden recovery copy",
        )
        .unwrap();
        std::fs::write(
            session_dir.join("_summary.md"),
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCanonical summary",
        )
        .unwrap();

        let run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let run: (String, i64) =
            sqlx::query_as("SELECT status, conflict_count FROM migration_import_runs WHERE id = ?")
                .bind(&run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let bodies: Vec<String> =
            sqlx::query_scalar("SELECT body FROM session_documents ORDER BY body")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let targets: Vec<(String, String)> = sqlx::query_as(
            "SELECT source_path, status FROM migration_import_targets
             WHERE run_id = ? AND table_name = 'session_documents'
             ORDER BY source_path",
        )
        .bind(&run_id)
        .fetch_all(db.pool())
        .await
        .unwrap();

        assert_eq!(run, ("completed".to_string(), 0));
        assert_eq!(bodies, vec!["Canonical summary", "Hidden recovery copy"]);
        assert_eq!(
            targets,
            vec![
                (
                    "sessions/session-1/.md".to_string(),
                    RECOVERED_DUPLICATE_DOCUMENT_ID.to_string()
                ),
                (
                    "sessions/session-1/_summary.md".to_string(),
                    "inserted".to_string()
                ),
            ]
        );
    }

    #[tokio::test]
    async fn empty_session_title_is_recovered_from_summary_heading() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(
            session_dir.join("_meta.json"),
            r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":""}"#,
        )
        .unwrap();
        std::fs::write(
            session_dir.join("_summary.md"),
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\n# Transcript Test Utterances\n\nDetails",
        )
        .unwrap();

        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        let title: String = sqlx::query_scalar("SELECT title FROM sessions WHERE id = 'session-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(title, "Transcript Test Utterances");
    }

    #[tokio::test]
    async fn divergent_nonempty_summaries_recover_after_a_partial_import() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/session-1");
        std::fs::create_dir_all(&session_dir).unwrap();
        let meta = r#"{"id":"session-1","created_at":"2026-07-12T01:00:00Z","title":"Planning"}"#;
        let hidden = "---\nid: summary-1\nsession_id: session-1\ntemplate_id: ''\ntitle: Summary\n---\n\nStale summary";
        let canonical =
            "---\nid: summary-1\nsession_id: session-1\ntitle: Summary\n---\n\nCurrent summary";
        std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
        std::fs::write(session_dir.join(".md"), hidden).unwrap();
        std::fs::write(session_dir.join("_summary.md"), canonical).unwrap();

        let failed_run_id = "failed-run";
        hypr_db_app::begin_legacy_import_run(
            db.pool(),
            failed_run_id,
            &dir.path().to_string_lossy(),
            false,
        )
        .await
        .unwrap();
        for (item_id, source_path, source_kind, source_hash, batch) in [
            (
                "meta-item",
                "sessions/session-1/_meta.json",
                "session_meta",
                sha256(meta.as_bytes()),
                parse_session_meta(dir.path(), &session_dir.join("_meta.json"), meta).unwrap(),
            ),
            (
                "hidden-item",
                "sessions/session-1/.md",
                "session_document",
                sha256(hidden.as_bytes()),
                parse_session_document(
                    dir.path(),
                    &session_dir.join(".md"),
                    hidden,
                    &sha256(hidden.as_bytes()),
                )
                .unwrap(),
            ),
            (
                "canonical-item",
                "sessions/session-1/_summary.md",
                "session_document",
                sha256(canonical.as_bytes()),
                parse_session_document(
                    dir.path(),
                    &session_dir.join("_summary.md"),
                    canonical,
                    &sha256(canonical.as_bytes()),
                )
                .unwrap(),
            ),
        ] {
            hypr_db_app::apply_legacy_import_item(
                db.pool(),
                LegacyImportItem {
                    id: item_id,
                    run_id: failed_run_id,
                    source_path,
                    source_kind,
                    source_sha256: &source_hash,
                },
                &batch,
                false,
            )
            .await
            .unwrap();
        }
        assert_eq!(
            hypr_db_app::finish_legacy_import_run(db.pool(), failed_run_id)
                .await
                .unwrap(),
            "completed_with_conflicts"
        );
        sqlx::query(
            "UPDATE migration_import_runs
             SET status = 'completed_with_issues'
             WHERE id = ?",
        )
        .bind(failed_run_id)
        .execute(db.pool())
        .await
        .unwrap();

        let recovery_run_id = import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        let bodies: Vec<String> =
            sqlx::query_scalar("SELECT body FROM session_documents ORDER BY body")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM migration_import_runs WHERE id = ?")
                .bind(&recovery_run_id)
                .fetch_one(db.pool())
                .await
                .unwrap();
        let parity_verified: bool = sqlx::query_scalar(
            "SELECT parity_verified FROM storage_migration_state WHERE id = 'legacy_v1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();

        assert_eq!(bodies, vec!["Current summary", "Stale summary"]);
        assert_eq!(status, "completed");
        assert!(parity_verified);
    }

    #[tokio::test]
    async fn shadow_import_is_non_destructive_idempotent_and_audited() {
        let db = test_db().await;
        let dir = tempfile::tempdir().unwrap();
        let session_dir = dir.path().join("sessions/work/session-1");
        std::fs::create_dir_all(session_dir.join("attachments")).unwrap();
        std::fs::create_dir_all(dir.path().join("humans")).unwrap();
        std::fs::create_dir_all(dir.path().join("organizations")).unwrap();
        std::fs::create_dir_all(dir.path().join("chats/chat-1")).unwrap();

        let meta = r#"{
          "id":"session-1",
          "user_id":"user-1",
          "created_at":"2026-07-10T01:00:00Z",
          "title":"Planning",
          "participants":[{"id":"participant-1","user_id":"user-1","human_id":"human-1","source":"manual"}],
          "key_facts":{"content":"Fact","source_hash":"source-hash"},
          "tags":["work"]
        }"#;
        std::fs::write(session_dir.join("_meta.json"), meta).unwrap();
        std::fs::write(
            session_dir.join("_memo.md"),
            "---\nid: session-1\nsession_id: session-1\n---\n\nMeeting note",
        )
        .unwrap();
        std::fs::write(
            session_dir.join("transcript.json"),
            r#"{"transcripts":[{"id":"transcript-1","session_id":"session-1","started_at":0,"words":[{"text":"hello","start_ms":0,"end_ms":10,"channel":0}],"speaker_hints":[]}]}"#,
        )
        .unwrap();
        std::fs::write(session_dir.join("attachments/file.txt"), b"attachment").unwrap();
        std::fs::write(
            dir.path().join("humans/human-1.md"),
            "---\nuser_id: user-1\nname: Alice\nemails: [alice@example.com]\norg_id: org-1\n---\n\nMemo",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("organizations/org-1.md"),
            "---\nuser_id: user-1\nname: Acme\n---\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("tasks.json"),
            r#"{"task-1":{"user_id":"user-1","task_id":"task-1","source_id":"session-1","source_type":"session","source_order":0,"status":"todo","text_preview":"Follow up","body":[]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("daily_notes.json"),
            r#"{"daily-1":{"user_id":"user-1","date":"2026-07-10","content":"{}"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("chats/chat-1/messages.json"),
            r#"{"chat_group":{"id":"chat-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","title":"Chat"},"messages":[{"id":"message-1","user_id":"user-1","created_at":"2026-07-10T01:00:00Z","chat_group_id":"chat-1","role":"user","content":"Hi","metadata":{},"parts":[],"status":"ready"}]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"general":{"theme":"dark"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("calendars.json"),
            r##"{"calendar-1":{"tracking_id_calendar":"calendar-track-1","name":"Work","enabled":true,"provider":"google","source":"account","color":"#123456","connection_id":"connection-1"}}"##,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("events.json"),
            r#"{"event-1":{"tracking_id_event":"event-track-1","calendar_id":"calendar-1","title":"Planning","started_at":"2026-07-10T01:00:00Z","ended_at":"2026-07-10T02:00:00Z","provider":"google","participants":[]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("templates.json"),
            r#"{"template-custom":{"title":"Custom","description":"Legacy template","sections":[]}}"#,
        )
        .unwrap();

        let source_before = std::fs::read(session_dir.join("_meta.json")).unwrap();
        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();
        import_legacy_vault(db.pool(), dir.path(), false)
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(session_dir.join("_meta.json")).unwrap(),
            source_before
        );
        for (table, query, expected) in [
            ("sessions", "SELECT COUNT(*) FROM sessions", 1_i64),
            (
                "session_documents",
                "SELECT COUNT(*) FROM session_documents",
                2,
            ),
            ("transcripts", "SELECT COUNT(*) FROM transcripts", 1),
            (
                "session_participants",
                "SELECT COUNT(*) FROM session_participants",
                1,
            ),
            ("tags", "SELECT COUNT(*) FROM tags", 1),
            ("session_tags", "SELECT COUNT(*) FROM session_tags", 1),
            (
                "session_attachments",
                "SELECT COUNT(*) FROM session_attachments",
                1,
            ),
            ("humans", "SELECT COUNT(*) FROM humans", 1),
            ("organizations", "SELECT COUNT(*) FROM organizations", 1),
            ("action_items", "SELECT COUNT(*) FROM action_items", 1),
            ("daily_notes", "SELECT COUNT(*) FROM daily_notes", 1),
            ("chat_groups", "SELECT COUNT(*) FROM chat_groups", 1),
            ("chat_messages", "SELECT COUNT(*) FROM chat_messages", 1),
            ("app_settings", "SELECT COUNT(*) FROM app_settings", 2),
            ("calendars", "SELECT COUNT(*) FROM calendars", 1),
            ("events", "SELECT COUNT(*) FROM events", 1),
            ("templates", "SELECT COUNT(*) FROM templates", 18),
        ] {
            assert_eq!(row_count(&db, query).await, expected, "{table}");
        }
        assert_eq!(
            row_count(
                &db,
                "SELECT COUNT(*) FROM app_settings WHERE id = 'cloudsync_workspace_binding'",
            )
            .await,
            1,
        );
        assert_eq!(
            row_count(
                &db,
                "SELECT COUNT(*) FROM app_settings WHERE id = 'legacy_settings_document'",
            )
            .await,
            1,
        );

        let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_runs")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(run_count, 2);
        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM migration_import_items")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(item_count, 26);

        let unchanged_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM migration_import_items WHERE status = 'unchanged'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(unchanged_count, 12);

        let report = super::super::get_legacy_import_report(db.pool())
            .await
            .unwrap();
        assert_eq!(report.items.len(), 13);
        assert_eq!(
            report
                .items
                .iter()
                .filter(|item| item.status == "unchanged")
                .count(),
            12
        );
        assert_eq!(
            report
                .items
                .iter()
                .filter(|item| item.status == "complete")
                .count(),
            1
        );
        assert_eq!(report.targets.len(), 18);
        assert_eq!(
            report
                .targets
                .iter()
                .filter(|target| target.status == "unchanged")
                .count(),
            17
        );
        assert_eq!(
            report
                .targets
                .iter()
                .filter(|target| target.status == "matched")
                .count(),
            1
        );
        let latest_run = report.latest_run.unwrap();
        assert_eq!(latest_run.status, "completed");
        assert_eq!(latest_run.matched_count, 18);
    }
}
