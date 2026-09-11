//! Folder catalog data: `session/queries/folders.ts`, `session/folder-catalog.ts`,
//! `session/folder-icon.ts`, and `session/folder-attachments.ts`, with the vault
//! mirrored through `anlg-fs-sync-core` the way the fs-sync plugin does.

use std::path::PathBuf;

use anyhow::{Context as _, anyhow};
use sqlx::SqlitePool;

use crate::db::TemplateIcon;
use crate::timeline::normalize_folder_path;

/// `FOLDER_PATHS_SQL`
const FOLDER_PATHS_SQL: &str = "
  SELECT folder_path
  FROM (
    SELECT folder_path
    FROM sessions
    WHERE deleted_at IS NULL
      AND folder_path != ''
    UNION
    SELECT folder_path
    FROM folder_attachments
    WHERE deleted_at IS NULL
      AND folder_path != ''
    UNION
    SELECT path AS folder_path
    FROM folders
    WHERE deleted_at IS NULL
      AND path != ''
  )
";

const FOLDER_ICONS_SQL: &str = "
  SELECT path, icon_json
  FROM folders
  WHERE deleted_at IS NULL
    AND path != ''
";

const FOLDER_INSTRUCTIONS_SQL: &str = "
  SELECT instructions
  FROM folders
  WHERE path = ?
    AND deleted_at IS NULL
  LIMIT 1
";

const FOLDER_MATERIALS_SQL: &str = "
  SELECT id, filename, content_type, size_bytes, relative_path
  FROM folder_attachments
  WHERE folder_path = ?
    AND deleted_at IS NULL
  ORDER BY filename, id
";

/// `ensureFolderStatements`: revive the newest row for the path, or insert one
/// with the workspace of the newest session inside it.
const REVIVE_FOLDER_SQL: &str = "
  UPDATE folders
  SET
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = NULL
  WHERE id = (
    SELECT id
    FROM folders
    WHERE path = ?
    ORDER BY deleted_at IS NULL DESC,
      updated_at DESC,
      id
    LIMIT 1
  )
";

const INSERT_FOLDER_SQL: &str = "
  INSERT INTO folders (
    id,
    workspace_id,
    path
  )
  SELECT
    ?,
    COALESCE((
      SELECT session.workspace_id
      FROM sessions AS session
      WHERE session.deleted_at IS NULL
        AND (session.folder_path = ? OR session.folder_path LIKE ?)
      ORDER BY session.updated_at DESC, session.id
      LIMIT 1
    ), ''),
    ?
  WHERE NOT EXISTS (
    SELECT 1
    FROM folders
    WHERE path = ?
      AND deleted_at IS NULL
  )
";

const RENAME_FOLDER_SQL: &str = "
  UPDATE folders
  SET
    path = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = NULL
  WHERE path = ?
    AND deleted_at IS NULL
";

const REWRITE_ATTACHMENT_PATHS_SQL: &str = "
  UPDATE folder_attachments
  SET
    folder_path = CASE
      WHEN folder_path = ? THEN ?
      ELSE ? || substr(folder_path, length(?) + 1)
    END,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE deleted_at IS NULL
    AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
";

const REWRITE_SESSION_PATHS_SQL: &str = "
  UPDATE sessions
  SET
    folder_path = CASE
      WHEN folder_path = ? THEN ?
      ELSE ? || substr(folder_path, length(?) + 1)
    END,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE deleted_at IS NULL
    AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
";

const FOLDER_NAME_TAKEN_SQL: &str = "
  SELECT 1 AS present
  FROM (
    SELECT path AS folder_path
    FROM folders
    WHERE deleted_at IS NULL
      AND path = ?
    UNION
    SELECT folder_path
    FROM folder_attachments
    WHERE deleted_at IS NULL
      AND folder_path = ?
    UNION
    SELECT folder_path
    FROM sessions
    WHERE deleted_at IS NULL
      AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
  )
  LIMIT 1
";

const FOLDER_SESSIONS_SQL: &str = "
  SELECT id, folder_path
  FROM sessions
  WHERE deleted_at IS NULL
    AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
";

const DELETE_FOLDERS_SQL: &str = "
  UPDATE folders
  SET
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE deleted_at IS NULL
    AND (path = ? OR path LIKE ? OR path LIKE ?)
";

const DELETE_FOLDER_ATTACHMENTS_SQL: &str = "
  UPDATE folder_attachments
  SET
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE deleted_at IS NULL
    AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
";

const UNFILE_SESSIONS_SQL: &str = "
  UPDATE sessions
  SET
    folder_path = '',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE deleted_at IS NULL
    AND (folder_path = ? OR folder_path LIKE ? OR folder_path LIKE ?)
";

const UPDATE_INSTRUCTIONS_SQL: &str = "
  UPDATE folders
  SET
    instructions = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE path = ?
    AND deleted_at IS NULL
";

const UPDATE_ICON_SQL: &str = "
  UPDATE folders
  SET
    icon_json = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE path = ?
    AND deleted_at IS NULL
";

const REVIVE_MATERIAL_SQL: &str = "
  UPDATE folder_attachments
  SET
    filename = ?,
    content_type = ?,
    size_bytes = ?,
    sha256 = ?,
    source_type = 'folder_material',
    source_id = ?,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = NULL
  WHERE id = (
    SELECT id
    FROM folder_attachments
    WHERE folder_path = ?
      AND relative_path = ?
    ORDER BY deleted_at IS NULL DESC,
      updated_at DESC,
      id
    LIMIT 1
  )
";

const INSERT_MATERIAL_SQL: &str = "
  INSERT INTO folder_attachments (
    id,
    workspace_id,
    folder_path,
    filename,
    relative_path,
    content_type,
    size_bytes,
    sha256,
    storage_kind,
    cloud_object_key,
    source_type,
    source_id,
    metadata_json
  )
  SELECT
    ?,
    COALESCE((
      SELECT session.workspace_id
      FROM sessions AS session
      WHERE session.deleted_at IS NULL
        AND (session.folder_path = ? OR session.folder_path LIKE ?)
      ORDER BY session.updated_at DESC, session.id
      LIMIT 1
    ), ''),
    ?,
    ?,
    ?,
    ?,
    ?,
    ?,
    'local_file',
    '',
    'folder_material',
    ?,
    '{}'
  WHERE NOT EXISTS (
    SELECT 1
    FROM folder_attachments
    WHERE folder_path = ?
      AND relative_path = ?
      AND deleted_at IS NULL
  )
";

const DELETE_MATERIAL_SQL: &str = "
  UPDATE folder_attachments
  SET
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    deleted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  WHERE folder_path = ?
    AND relative_path = ?
    AND deleted_at IS NULL
";

/// `FolderMaterialRecord`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub relative_path: String,
}

impl Material {
    /// `diskAttachmentId`
    pub fn attachment_id(&self) -> &str {
        self.relative_path
            .rsplit('/')
            .next()
            .unwrap_or(&self.relative_path)
    }
}

/// The folder catalog snapshot the tab renders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// `useFolderPaths`: every ancestor of every referenced path, sorted.
    pub paths: Vec<String>,
    /// `useFolderIcons`
    pub icons: Vec<(String, TemplateIcon)>,
}

/// `collectFolderPaths`
pub fn collect_folder_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut collected = std::collections::BTreeSet::new();
    for path in paths {
        let Some(normalized) = normalize_folder_path(path) else {
            continue;
        };
        let mut acc = String::new();
        for segment in normalized.split('/') {
            if acc.is_empty() {
                acc = segment.to_string();
            } else {
                acc = format!("{acc}/{segment}");
            }
            collected.insert(acc.clone());
        }
    }
    collected.into_iter().collect()
}

/// `folderDisplayName`
pub fn display_name(path: &str) -> String {
    normalize_folder_path(path)
        .and_then(|normalized| normalized.rsplit('/').next().map(str::to_string))
        .unwrap_or_default()
}

/// `ancestorFolderPaths`: the path and every ancestor, shortest first.
fn ancestor_paths(path: &str) -> Vec<String> {
    let mut acc = String::new();
    let mut out = Vec::new();
    for segment in path.split('/') {
        if acc.is_empty() {
            acc = segment.to_string();
        } else {
            acc = format!("{acc}/{segment}");
        }
        out.push(acc.clone());
    }
    out
}

fn require_named(path: &str) -> anyhow::Result<String> {
    normalize_folder_path(path).ok_or_else(|| anyhow!("invalid folder path"))
}

fn like_params(path: &str) -> [String; 3] {
    [path.to_string(), format!("{path}/%"), format!("{path}\\%")]
}

/// `normalizeFolderIcon`: unwrap up to three JSON layers, accept explicit
/// template icons, otherwise the default folder icon.
pub fn normalize_folder_icon(value: &str) -> TemplateIcon {
    let mut current: serde_json::Value = serde_json::Value::String(value.to_string());
    for _ in 0..3 {
        let Some(text) = current.as_str() else {
            break;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return TemplateIcon::default_folder();
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(parsed) => current = parsed,
            Err(_) => return TemplateIcon::default_folder(),
        }
    }
    match TemplateIcon::from_json(&current) {
        Some(icon) if icon.is_explicit() => icon,
        _ => TemplateIcon::default_folder(),
    }
}

pub struct Vault {
    fs: anlg_fs_sync_core::FsSyncCore,
}

impl Vault {
    /// The vault is the folder holding `app.db`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            fs: anlg_fs_sync_core::FsSyncCore::new(base_dir),
        }
    }
}

async fn ensure_catalog(pool: &SqlitePool, path: &str) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    for ancestor in ancestor_paths(path) {
        sqlx::query(REVIVE_FOLDER_SQL)
            .bind(&ancestor)
            .execute(&mut *tx)
            .await?;
        sqlx::query(INSERT_FOLDER_SQL)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&ancestor)
            .bind(format!("{ancestor}/%"))
            .bind(&ancestor)
            .bind(&ancestor)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// `useFolderPaths` + `useFolderIcons`
pub async fn load_catalog(pool: &SqlitePool) -> anyhow::Result<Catalog> {
    let rows: Vec<(String,)> = sqlx::query_as(FOLDER_PATHS_SQL).fetch_all(pool).await?;
    let paths = collect_folder_paths(rows.iter().map(|(path,)| path.as_str()));
    let icon_rows: Vec<(String, Option<String>)> =
        sqlx::query_as(FOLDER_ICONS_SQL).fetch_all(pool).await?;
    let icons = icon_rows
        .into_iter()
        .map(|(path, icon_json)| {
            (
                path,
                normalize_folder_icon(icon_json.as_deref().unwrap_or("")),
            )
        })
        .collect();
    Ok(Catalog { paths, icons })
}

/// `useFolderInstructions`
pub async fn load_instructions(pool: &SqlitePool, path: &str) -> anyhow::Result<String> {
    let row: Option<(String,)> = sqlx::query_as(FOLDER_INSTRUCTIONS_SQL)
        .bind(path)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(instructions,)| instructions).unwrap_or_default())
}

/// `useFolderMaterials`
pub async fn load_materials(pool: &SqlitePool, path: &str) -> anyhow::Result<Vec<Material>> {
    let rows: Vec<(String, String, String, i64, String)> = sqlx::query_as(FOLDER_MATERIALS_SQL)
        .bind(path)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, filename, content_type, size_bytes, relative_path)| Material {
                id,
                filename,
                content_type,
                size_bytes,
                relative_path,
            },
        )
        .collect())
}

/// `createNamedFolder`
pub async fn create_folder(pool: &SqlitePool, vault: &Vault, path: &str) -> anyhow::Result<String> {
    let path = require_named(path)?;
    ensure_catalog(pool, &path).await?;
    vault.fs.create_folder(&path)?;
    Ok(path)
}

/// `renameNamedFolder`
pub async fn rename_folder(
    pool: &SqlitePool,
    vault: &Vault,
    old_path: &str,
    new_path: &str,
) -> anyhow::Result<String> {
    let old_path = require_named(old_path)?;
    let new_path = require_named(new_path)?;
    if old_path == new_path {
        return Ok(new_path);
    }
    let [new_exact, new_slash, new_escaped] = like_params(&new_path);
    let taken: Option<(i64,)> = sqlx::query_as(FOLDER_NAME_TAKEN_SQL)
        .bind(&new_exact)
        .bind(&new_exact)
        .bind(&new_exact)
        .bind(&new_slash)
        .bind(&new_escaped)
        .fetch_optional(pool)
        .await?;
    if taken.is_some() {
        return Err(anyhow!("folder_target_exists"));
    }
    // `renameFolderOnDisk`: a missing source folder is created at the new path.
    match vault.fs.rename_folder(&old_path, &new_path) {
        Ok(_) => {}
        Err(error) if error.to_string().contains("folder_source_missing") => {
            vault.fs.create_folder(&new_path)?;
        }
        Err(error) => return Err(error.into()),
    }
    let [old_exact, old_slash, old_escaped] = like_params(&old_path);
    let mut tx = pool.begin().await?;
    sqlx::query(RENAME_FOLDER_SQL)
        .bind(&new_path)
        .bind(&old_path)
        .execute(&mut *tx)
        .await?;
    for ancestor in ancestor_paths(&new_path) {
        sqlx::query(REVIVE_FOLDER_SQL)
            .bind(&ancestor)
            .execute(&mut *tx)
            .await?;
        sqlx::query(INSERT_FOLDER_SQL)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&ancestor)
            .bind(format!("{ancestor}/%"))
            .bind(&ancestor)
            .bind(&ancestor)
            .execute(&mut *tx)
            .await?;
    }
    for sql in [REWRITE_ATTACHMENT_PATHS_SQL, REWRITE_SESSION_PATHS_SQL] {
        sqlx::query(sql)
            .bind(&old_path)
            .bind(&new_path)
            .bind(&new_path)
            .bind(&old_path)
            .bind(&old_exact)
            .bind(&old_slash)
            .bind(&old_escaped)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(new_path)
}

/// `deleteNamedFolder`: sessions move back to the root on disk, then the
/// folder rows, attachments, and session paths are cleared and the directory
/// removed.
pub async fn delete_folder(pool: &SqlitePool, vault: &Vault, path: &str) -> anyhow::Result<()> {
    let path = require_named(path)?;
    let [exact, slash, escaped] = like_params(&path);
    let sessions: Vec<(String, String)> = sqlx::query_as(FOLDER_SESSIONS_SQL)
        .bind(&exact)
        .bind(&slash)
        .bind(&escaped)
        .fetch_all(pool)
        .await?;
    for (id, folder_path) in sessions {
        match vault.fs.move_session(&id, &folder_path, "") {
            Ok(_) => {}
            Err(error) if error.to_string().contains("session_source_missing") => {}
            Err(error) => return Err(error.into()),
        }
    }
    let mut tx = pool.begin().await?;
    for sql in [
        DELETE_FOLDERS_SQL,
        DELETE_FOLDER_ATTACHMENTS_SQL,
        UNFILE_SESSIONS_SQL,
    ] {
        sqlx::query(sql)
            .bind(&exact)
            .bind(&slash)
            .bind(&escaped)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    match vault.fs.delete_folder(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.to_string().contains("folder_source_missing") => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// `updateFolderInstructions`
pub async fn update_instructions(
    pool: &SqlitePool,
    path: &str,
    instructions: &str,
) -> anyhow::Result<()> {
    let path = require_named(path)?;
    ensure_catalog(pool, &path).await?;
    sqlx::query(UPDATE_INSTRUCTIONS_SQL)
        .bind(instructions)
        .bind(&path)
        .execute(pool)
        .await?;
    Ok(())
}

/// `updateFolderIcon`: `ensureFolderStatements(path)` then the icon update,
/// with the icon normalised to its JSON form.
pub async fn update_icon(pool: &SqlitePool, path: &str, icon: &TemplateIcon) -> anyhow::Result<()> {
    let path = require_named(path)?;
    ensure_catalog(pool, &path).await?;
    sqlx::query(UPDATE_ICON_SQL)
        .bind(icon.to_json().to_string())
        .bind(&path)
        .execute(pool)
        .await?;
    Ok(())
}

const MAX_IPC_ATTACHMENT_BYTES: u64 = 4 * 1024 * 1024;

/// `useFolderMaterialUpload`: copy the file into `<folder>/materials`, then
/// `catalogLocalFolderMaterial`; the file is removed again if cataloguing fails.
pub async fn add_material(
    pool: &SqlitePool,
    vault: &Vault,
    path: &str,
    file: &std::path::Path,
) -> anyhow::Result<()> {
    let path = require_named(path)?;
    let filename = file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("invalid attachment filename"))?
        .to_string();
    let data = std::fs::read(file).with_context(|| format!("reading {}", file.display()))?;
    if data.len() as u64 > MAX_IPC_ATTACHMENT_BYTES {
        return Err(anyhow!("Attachments must be smaller than 4 MB"));
    }
    let sha256 = {
        use sha2::Digest as _;
        sha2::Sha256::digest(&data)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let content_type = mime_guess::from_path(file)
        .first_raw()
        .unwrap_or("")
        .to_string();
    let saved = vault.fs.folder_attachment_save(&path, &data, &filename)?;
    let attachment_id = saved.attachment_id;
    let relative_path = format!("materials/{attachment_id}");
    let result: anyhow::Result<()> = async {
        let mut tx = pool.begin().await?;
        let revived = sqlx::query(REVIVE_MATERIAL_SQL)
            .bind(&filename)
            .bind(&content_type)
            .bind(data.len() as i64)
            .bind(&sha256)
            .bind(&attachment_id)
            .bind(&path)
            .bind(&relative_path)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let inserted = sqlx::query(INSERT_MATERIAL_SQL)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&path)
            .bind(format!("{path}/%"))
            .bind(&path)
            .bind(&filename)
            .bind(&relative_path)
            .bind(&content_type)
            .bind(data.len() as i64)
            .bind(&sha256)
            .bind(&attachment_id)
            .bind(&path)
            .bind(&relative_path)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        if revived + inserted != 1 {
            return Err(anyhow!("folder material is unavailable"));
        }
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = vault.fs.folder_attachment_remove(&path, &attachment_id);
    }
    result
}

/// `deleteLocalFolderMaterial`
pub async fn remove_material(
    pool: &SqlitePool,
    vault: &Vault,
    path: &str,
    attachment_id: &str,
) -> anyhow::Result<()> {
    let path = require_named(path)?;
    let relative_path = format!("materials/{attachment_id}");
    let updated = sqlx::query(DELETE_MATERIAL_SQL)
        .bind(&path)
        .bind(&relative_path)
        .execute(pool)
        .await?
        .rows_affected();
    if updated != 1 {
        return Err(anyhow!("folder material is unavailable"));
    }
    vault.fs.folder_attachment_remove(&path, attachment_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_every_ancestor_sorted() {
        assert_eq!(
            collect_folder_paths(["work/clients/acme", "personal", "work"]),
            vec!["personal", "work", "work/clients", "work/clients/acme"]
        );
    }

    #[tokio::test]
    async fn update_icon_creates_the_folder_row_then_writes_the_icon_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = anlg_db_core::Db::connect_local_plain(&dir.path().join("app.db"))
            .await
            .unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        update_icon(
            db.pool(),
            "work/clients",
            &TemplateIcon::Icon {
                name: "briefcase".to_string(),
                color: "#5b67d8".to_string(),
            },
        )
        .await
        .unwrap();
        let rows: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT path, icon_json FROM folders ORDER BY path")
                .fetch_all(db.pool())
                .await
                .unwrap();
        // `ensureFolderStatements` seeds the ancestor with the default icon.
        assert_eq!(
            rows,
            vec![
                (
                    "work".to_string(),
                    Some(r##"{"type":"icon","value":"folder","color":"#9ca3af"}"##.to_string())
                ),
                (
                    "work/clients".to_string(),
                    Some(r##"{"type":"icon","value":"briefcase","color":"#5b67d8"}"##.to_string())
                ),
            ]
        );
        update_icon(
            db.pool(),
            "work/clients",
            &TemplateIcon::Emoji("📁".to_string()),
        )
        .await
        .unwrap();
        let icon: String =
            sqlx::query_scalar("SELECT icon_json FROM folders WHERE path = 'work/clients'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(icon, r##"{"type":"emoji","value":"📁"}"##);
    }

    #[test]
    fn display_name_is_the_last_segment() {
        assert_eq!(display_name("work/clients/acme"), "acme");
        assert_eq!(display_name("  "), "");
    }

    #[test]
    fn folder_icons_unwrap_double_encoded_json() {
        let icon = normalize_folder_icon(r#""{\"type\":\"emoji\",\"value\":\"🚀\"}""#);
        assert_eq!(icon.to_json()["value"], "🚀");
        assert_eq!(
            normalize_folder_icon("garbage"),
            TemplateIcon::default_folder()
        );
        assert_eq!(normalize_folder_icon(""), TemplateIcon::default_folder());
    }
}
