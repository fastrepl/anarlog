use std::path::Path;

use anlg_db_app::{LegacyImportBatch, LegacyImportItem, LegacyImportRow};
use sqlx::SqlitePool;

use self::discovery::{SourceFile, SourceKind, discover_sources};
use self::documents::{DocumentVariants, promote_canonical_summary};
use self::entities::{
    parse_chat, parse_daily_notes, parse_human, parse_organization, parse_settings, parse_tasks,
};
use self::sessions::{
    parse_attachment, parse_session_document, parse_session_meta, parse_transcript,
};
use self::source_format::{sha256, stable_id};

#[cfg(test)]
use self::documents::RECOVERED_DUPLICATE_DOCUMENT_ID;
#[cfg(test)]
use self::sessions::RECOVERED_MISSING_SESSION_METADATA;

mod discovery;
mod documents;
mod entities;
mod sessions;
mod source_format;

static IMPORT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn import_legacy_vault(
    pool: &SqlitePool,
    vault_base: &Path,
    dry_run: bool,
) -> crate::Result<String> {
    let _guard = IMPORT_LOCK.lock().await;
    let run_id = uuid::Uuid::new_v4().to_string();
    let source_root = vault_base.to_string_lossy();
    anlg_db_app::begin_legacy_import_run(pool, &run_id, &source_root, dry_run).await?;

    let discovery = match discover_sources(vault_base) {
        Ok(discovery) => discovery,
        Err(error) => {
            anlg_db_app::fail_legacy_import_run(pool, &run_id, &error.to_string()).await?;
            return Err(error.into());
        }
    };
    let mut document_variants = DocumentVariants::default();

    for source in discovery.files {
        let bytes = match std::fs::read(&source.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let item_id = stable_id(&format!("{run_id}:{}", source.relative_path));
                anlg_db_app::record_legacy_import_error(
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
            && anlg_db_app::legacy_source_already_imported(
                pool,
                &source.relative_path,
                &source_sha256,
            )
            .await?
        {
            anlg_db_app::record_legacy_import_unchanged(
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
                    document_variants.recover(document, &source.relative_path, &source_sha256);
                }
                anlg_db_app::apply_legacy_import_item(pool, item, &batch, dry_run).await?;
            }
            Err(error) => {
                anlg_db_app::record_legacy_import_error(pool, item, &error).await?;
            }
        }
    }

    anlg_db_app::finish_legacy_import_run(pool, &run_id).await?;
    Ok(run_id)
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

#[cfg(test)]
mod tests;
