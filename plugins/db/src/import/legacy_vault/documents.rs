use std::collections::HashMap;
use std::path::Path;

use anlg_db_app::{LegacyDocument, LegacyImportRow};
use sqlx::SqlitePool;

use super::sessions::parse_session_document;
use super::source_format::{normalized_relative_path, sha256, stable_id};

pub(super) const RECOVERED_DUPLICATE_DOCUMENT_ID: &str = "recovered_duplicate_document_id";

#[derive(Debug)]
pub(super) struct SummaryPair {
    pub(super) hidden_relative_path: String,
    pub(super) canonical_relative_path: String,
    pub(super) hidden_document: LegacyDocument,
    pub(super) hide_hidden_source: bool,
}

#[derive(Debug)]
struct DocumentVariant {
    document: LegacyDocument,
    source_path: String,
    target_id: String,
}

#[derive(Default)]
pub(super) struct DocumentVariants {
    by_id: HashMap<String, Vec<DocumentVariant>>,
}

impl DocumentVariants {
    pub(super) fn recover(
        &mut self,
        document: &mut LegacyDocument,
        source_path: &str,
        source_sha256: &str,
    ) {
        recover_duplicate_document_id(document, source_path, source_sha256, &mut self.by_id);
    }
}

pub(super) fn detect_summary_pair(
    vault_base: &Path,
    path: &Path,
    relative_path: &str,
) -> Option<SummaryPair> {
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
    let hide_hidden_source = (hidden_empty
        || (!canonical_empty && hidden_document.body == canonical_document.body))
        && (hidden_title_empty
            || (!canonical_title_empty && hidden_document.title == canonical_document.title));

    Some(SummaryPair {
        hidden_relative_path: relative_path.to_string(),
        canonical_relative_path: normalized_relative_path(vault_base, &canonical_path),
        hidden_document,
        hide_hidden_source,
    })
}

pub(super) async fn promote_canonical_summary(
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
