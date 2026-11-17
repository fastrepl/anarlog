use std::{io::Read, path::PathBuf, sync::Arc};

use anlg_db_execute::DbExecutor;
use anlg_fs_sync_core::{FsSyncCore, normalize_folder_path};
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::mutations::{NOW, cas, failure, statement, transaction};

#[derive(Clone)]
pub struct Material {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

pub enum MaterialCommand {
    Import(PathBuf),
    Remove(Arc<str>),
}

pub fn materials(runtime: &RuntimeHandle, folder: Arc<str>) -> Result<Reply<Vec<Material>>> {
    runtime.read(CancellationToken::new(),move |services| async move {
        let rows=services.executor.execute("SELECT id,filename FROM folder_attachments WHERE folder_path=? AND deleted_at IS NULL ORDER BY filename,id LIMIT 100".into(),vec![json!(folder)]).await.map_err(failure)?;
        Ok(rows.iter().filter_map(|row| Some(Material{id:row["id"].as_str()?.into(),name:row["filename"].as_str()?.into()})).collect())
    })
}

pub fn material_command(
    runtime: &RuntimeHandle,
    folder: Arc<str>,
    command: MaterialCommand,
) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let executor=&services.executor;
        let exists=executor.execute("SELECT id FROM folders WHERE path=? AND deleted_at IS NULL".into(),vec![json!(folder)]).await.map_err(failure)?;
        if exists.is_empty() {return Err(ServiceError::Conflict);}
        match command {
            MaterialCommand::Remove(id)=>transaction(executor,vec![statement(format!("UPDATE folder_attachments SET deleted_at={NOW},updated_at={NOW} WHERE id=? AND folder_path=? AND deleted_at IS NULL"),vec![json!(id),json!(folder)],Some(1))]).await,
            MaterialCommand::Import(path)=>{
                let name=path.file_name().and_then(|name| name.to_str()).ok_or_else(|| failure("Invalid material filename"))?;
                let mut bytes=Vec::new();
                std::fs::File::open(&path).map_err(failure)?.take(32*1024*1024+1).read_to_end(&mut bytes).map_err(failure)?;
                if bytes.len()>32*1024*1024 {return Err(failure("Materials are limited to 32 MiB"));}
                let content_type=match path.extension().and_then(|ext| ext.to_str()) {Some("pdf")=>"application/pdf",Some("md")=>"text/markdown",Some("txt")=>"text/plain",_=>"application/octet-stream"};
                let fs=filesystem(executor).await?;
                let saved=fs.folder_attachment_save(&folder,&bytes,name).map_err(failure)?;
                let checksum=Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
                let result=transaction(executor,vec![statement("INSERT INTO folder_attachments(id,folder_path,filename,relative_path,content_type,size_bytes,sha256,source_id) VALUES (?,?,?,?,?,?,?,?)".into(),vec![json!(Uuid::new_v4().to_string()),json!(folder),json!(name),json!(format!("materials/{}",saved.attachment_id)),json!(content_type),json!(bytes.len()),json!(checksum),json!(saved.attachment_id)],Some(1))]).await;
                if result.is_err() {fs.folder_attachment_remove(&folder,&saved.attachment_id).map_err(failure)?;}
                result
            }
        }
    })
}

pub(super) async fn filesystem(executor: &DbExecutor) -> Result<FsSyncCore> {
    let rows = executor
        .execute(
            "SELECT file FROM pragma_database_list WHERE name='main'".into(),
            vec![],
        )
        .await
        .map_err(failure)?;
    let database = rows
        .first()
        .and_then(|row| row["file"].as_str())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| failure("The profile has no filesystem root"))?;
    let root = PathBuf::from(database)
        .parent()
        .ok_or_else(|| failure("Invalid profile path"))?
        .join("vault");
    Ok(FsSyncCore::new(root))
}

/// Renames the folder row `base` to `path` on disk and in the catalog, moving nested
/// folders, notes and materials.
async fn rename_row(executor: &DbExecutor, base: Value, path: &str) -> Result<()> {
    let path = normalize_folder_path(path.trim()).map_err(failure)?;
    if path.is_empty() {
        return Err(failure("A folder name is required"));
    }
    let fs = filesystem(executor).await?;
    let old = base["path"].as_str();
    if old != Some(&path) {
        let collision = executor
            .execute(
                "SELECT id FROM folders WHERE path=? AND deleted_at IS NULL".into(),
                vec![json!(path)],
            )
            .await
            .map_err(failure)?;
        if !collision.is_empty() {
            return Err(failure("A folder with that name already exists"));
        }
        if let Some(old) = old {
            if path.starts_with(&format!("{old}/")) {
                return Err(failure("Cannot move a folder into itself"));
            }
            match fs.rename_folder(old, &path) {
                Ok(_) => {}
                Err(anlg_fs_sync_core::Error::Path(message))
                    if message == "folder_source_missing" =>
                {
                    fs.create_folder(&path).map_err(failure)?
                }
                Err(error) => return Err(failure(error)),
            }
        } else {
            fs.create_folder(&path).map_err(failure)?;
        }
    }
    let (predicate, mut params) = cas(&base)?;
    let mut values = vec![json!(path)];
    values.append(&mut params);
    let mut statements = vec![statement(
        format!("UPDATE folders SET path=?,updated_at={NOW} WHERE {predicate}"),
        values,
        Some(1),
    )];
    if let Some(old) = old.filter(|old| *old != path) {
        for (table, column) in [
            ("folders", "path"),
            ("sessions", "folder_path"),
            ("folder_attachments", "folder_path"),
        ] {
            statements.push(statement(format!("UPDATE {table} SET {column}=?1 || substr({column},length(?2)+1),updated_at={NOW} WHERE deleted_at IS NULL AND ({column}=?2 OR substr({column},1,length(?2)+1)=?2 || '/')"),
                vec![json!(path),json!(old)],None));
        }
    }
    let segments = path.split('/').collect::<Vec<_>>();
    for count in 1..segments.len() {
        statements.push(statement(format!("INSERT INTO folders(id,path,created_at,updated_at) SELECT ?,?,{NOW},{NOW} WHERE NOT EXISTS(SELECT 1 FROM folders WHERE path=? AND deleted_at IS NULL)"),vec![json!(Uuid::new_v4().to_string()),json!(segments[..count].join("/")),json!(segments[..count].join("/"))],None));
    }
    if let Err(error) = transaction(executor, statements).await {
        if let Some(old) = old.filter(|old| *old != path)
            && let Err(rollback) = fs.rename_folder(&path, old)
        {
            return Err(failure(format!(
                "{error}; filesystem rollback failed: {rollback}. Files remain at {path}."
            )));
        }
        return Err(error);
    }
    Ok(())
}

async fn remove(executor: &DbExecutor, base: Value) -> Result<()> {
    let path = base["path"].as_str().ok_or(ServiceError::Conflict)?;
    let fs = filesystem(executor).await?;
    let rows = executor.execute("SELECT id,folder_path FROM sessions WHERE deleted_at IS NULL AND (folder_path=?1 OR substr(folder_path,1,length(?1)+1)=?1 || '/')".into(),vec![json!(path)]).await.map_err(failure)?;
    let mut moved = Vec::new();
    for row in &rows {
        let id = row["id"].as_str().unwrap_or("");
        let folder = row["folder_path"].as_str().unwrap_or("");
        match fs.move_session(id, folder, "") {
            Ok(_) => moved.push((id, folder)),
            Err(anlg_fs_sync_core::Error::Path(message)) if message == "session_source_missing" => {
            }
            Err(error) => {
                rollback_moves(&fs, &moved)?;
                return Err(failure(error));
            }
        }
    }
    let (predicate, params) = cas(&base)?;
    let mut statements = vec![statement(
        format!("UPDATE folders SET updated_at={NOW} WHERE {predicate}"),
        params,
        Some(1),
    )];
    for (table, column) in [("folders", "path"), ("folder_attachments", "folder_path")] {
        statements.push(statement(format!("UPDATE {table} SET deleted_at={NOW},updated_at={NOW} WHERE deleted_at IS NULL AND ({column}=?1 OR substr({column},1,length(?1)+1)=?1 || '/')"),vec![json!(path)],None));
    }
    for row in &rows {
        statements.push(statement(format!("UPDATE sessions SET folder_path='',updated_at={NOW} WHERE id=? AND folder_path=? AND deleted_at IS NULL"),vec![row["id"].clone(),row["folder_path"].clone()],Some(1)));
    }
    if let Err(error) = transaction(executor, statements).await {
        rollback_moves(&fs, &moved)?;
        return Err(error);
    }
    // Keep material files for recovery of the soft-deleted catalog rows.
    Ok(())
}

fn rollback_moves(fs: &FsSyncCore, moved: &[(&str, &str)]) -> Result<()> {
    for (id, folder) in moved.iter().rev() {
        fs.move_session(id, "", folder).map_err(|error| {
            failure(format!(
                "Folder operation failed and restoring {id} failed: {error}"
            ))
        })?;
    }
    Ok(())
}

pub(super) async fn move_notes(
    executor: &DbExecutor,
    ids: &[Value],
    destination: &str,
) -> Result<()> {
    let path = normalize_folder_path(destination).map_err(failure)?;
    if !path.is_empty()
        && executor
            .execute(
                "SELECT id FROM folders WHERE path=? AND deleted_at IS NULL".into(),
                vec![json!(path)],
            )
            .await
            .map_err(failure)?
            .is_empty()
    {
        return Err(failure("Choose an existing folder"));
    }
    let fs = filesystem(executor).await?;
    let rows = executor.execute("SELECT id,folder_path,updated_at FROM sessions WHERE id IN(SELECT value FROM json_each(?)) AND deleted_at IS NULL".into(),vec![json!(ids).to_string().into()]).await.map_err(failure)?;
    if rows.len() != ids.len() {
        return Err(ServiceError::Conflict);
    }
    let mut moved = Vec::new();
    let mut statements = Vec::new();
    for row in &rows {
        let id = row["id"].as_str().unwrap_or("");
        let old = row["folder_path"].as_str().unwrap_or("");
        if old == path {
            continue;
        }
        match fs.move_session(id, old, &path) {
            Ok(_) => moved.push((id, old)),
            Err(anlg_fs_sync_core::Error::Path(message)) if message == "session_source_missing" => {
            }
            Err(error) => {
                for (id, old) in moved.iter().rev() {
                    fs.move_session(id, &path, old).map_err(failure)?;
                }
                return Err(failure(error));
            }
        }
        statements.push(statement(format!("UPDATE sessions SET folder_path=?,updated_at={NOW} WHERE id=? AND folder_path=? AND updated_at=? AND deleted_at IS NULL"),vec![json!(path),json!(id),json!(old),row["updated_at"].clone()],Some(1)));
    }
    if let Err(error) = transaction(executor, statements).await {
        for (id, old) in moved.iter().rev() {
            fs.move_session(id, &path, old).map_err(failure)?;
        }
        return Err(error);
    }
    Ok(())
}

/// Every folder the library knows about: catalog rows, plus paths that only notes or
/// materials still reference (shipping's `FOLDER_PATHS_SQL`).
pub const LIBRARY_SQL: &str = "SELECT path AS folder_path, icon_json FROM folders
        WHERE deleted_at IS NULL AND path != ''
    UNION ALL SELECT DISTINCT folder_path, NULL FROM sessions
        WHERE deleted_at IS NULL AND folder_path != ''
    UNION ALL SELECT DISTINCT folder_path, NULL FROM folder_attachments
        WHERE deleted_at IS NULL AND folder_path != ''
    LIMIT 5000";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// Normalized paths and all their ancestors, in `localeCompare` order.
    pub paths: Vec<Arc<str>>,
    pub icons: std::collections::HashMap<Arc<str>, super::resource_list::Glyph>,
}

impl Library {
    pub fn icon(&self, path: &str) -> super::resource_list::Glyph {
        self.icons
            .get(path)
            .cloned()
            .unwrap_or_else(super::resource_list::Glyph::folder_default)
    }
}

pub fn decode_library(rows: &[Value]) -> Library {
    let mut paths = std::collections::BTreeSet::new();
    let mut icons = std::collections::HashMap::new();
    for row in rows {
        let Some(path) = row["folder_path"]
            .as_str()
            .and_then(|path| normalize_folder_path(path).ok())
            .filter(|path| !path.is_empty())
        else {
            continue;
        };
        if let Some(raw) = row["icon_json"].as_str() {
            icons.insert(
                Arc::from(path.as_str()),
                super::resource_list::Glyph::folder(raw),
            );
        }
        let mut ancestor = String::new();
        for segment in path.split('/') {
            if !ancestor.is_empty() {
                ancestor.push('/');
            }
            ancestor.push_str(segment);
            paths.insert(ancestor.clone());
        }
    }
    let mut paths = paths.into_iter().map(Arc::from).collect::<Vec<Arc<str>>>();
    paths.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| b.cmp(a))
    });
    Library { paths, icons }
}

pub fn display_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Creates catalog rows for `path` and its ancestors when missing, then returns its row.
async fn ensure(executor: &DbExecutor, path: &str) -> Result<Value> {
    let mut statements = Vec::new();
    let mut ancestor = String::new();
    for segment in path.split('/') {
        if !ancestor.is_empty() {
            ancestor.push('/');
        }
        ancestor.push_str(segment);
        statements.push(statement(format!("INSERT INTO folders(id,path,created_at,updated_at) SELECT ?,?,{NOW},{NOW} WHERE NOT EXISTS(SELECT 1 FROM folders WHERE path=? AND deleted_at IS NULL)"),vec![json!(Uuid::new_v4().to_string()),json!(ancestor),json!(ancestor)],None));
    }
    transaction(executor, statements).await?;
    let rows = executor
        .execute(
            "SELECT * FROM folders WHERE path=? AND deleted_at IS NULL".into(),
            vec![json!(path)],
        )
        .await
        .map_err(failure)?;
    rows.into_iter().next().ok_or(ServiceError::Conflict)
}

fn named(path: &str) -> Result<String> {
    let path = normalize_folder_path(path.trim()).map_err(failure)?;
    if path.is_empty() {
        return Err(failure("A folder name is required"));
    }
    Ok(path)
}

/// `createNamedFolder`: idempotent catalog rows plus the vault directory.
pub fn create(runtime: &RuntimeHandle, path: Arc<str>) -> Result<Reply<Arc<str>>> {
    runtime.submit(move |services| async move {
        let path = named(&path)?;
        ensure(&services.executor, &path).await?;
        filesystem(&services.executor)
            .await?
            .create_folder(&path)
            .map_err(failure)?;
        Ok(path.into())
    })
}

/// Renames the folder at `old` to `new`, moving nested folders, notes and materials.
pub fn rename(runtime: &RuntimeHandle, old: Arc<str>, new: Arc<str>) -> Result<Reply<Arc<str>>> {
    runtime.submit(move |services| async move {
        let new = named(&new)?;
        let base = ensure(&services.executor, &old).await?;
        rename_row(&services.executor, base, &new).await?;
        Ok(new.into())
    })
}

/// Deletes the folder and its nested folders; their notes move to the library root.
pub fn delete(runtime: &RuntimeHandle, path: Arc<str>) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let base = ensure(&services.executor, &path).await?;
        remove(&services.executor, base).await?;
        Ok(())
    })
}

pub enum Update {
    Instructions(Arc<str>),
    Icon(super::resource_list::Glyph),
}

pub fn update(runtime: &RuntimeHandle, path: Arc<str>, update: Update) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        ensure(&services.executor, &path).await?;
        let (column, value) = match update {
            Update::Instructions(text) => ("instructions", text.to_string()),
            Update::Icon(icon) => ("icon_json", icon.to_json()),
        };
        transaction(&services.executor, vec![statement(format!("UPDATE folders SET {column}=?,updated_at={NOW} WHERE path=? AND deleted_at IS NULL"),vec![json!(value),json!(path)],Some(1))]).await
    })
}

pub fn instructions(runtime: &RuntimeHandle, path: Arc<str>) -> Result<Reply<Arc<str>>> {
    runtime.read(CancellationToken::new(), move |services| async move {
        let rows = services
            .executor
            .execute(
                "SELECT instructions FROM folders WHERE path=? AND deleted_at IS NULL LIMIT 1"
                    .into(),
                vec![json!(path)],
            )
            .await
            .map_err(failure)?;
        Ok(rows
            .first()
            .and_then(|row| row["instructions"].as_str())
            .unwrap_or_default()
            .into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_list_adds_ancestors_and_sorts_like_shipping() {
        let library = decode_library(&[
            json!({"folder_path": "work/Projects", "icon_json": null}),
            json!({"folder_path": "Archive", "icon_json": r#"{"type":"emoji","value":"📦"}"#}),
            json!({"folder_path": "../escape", "icon_json": null}),
            json!({"folder_path": "", "icon_json": null}),
        ]);
        assert_eq!(
            library.paths.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            ["Archive", "work", "work/Projects"]
        );
        assert_eq!(
            library.icon("Archive"),
            super::super::resource_list::Glyph::Emoji("📦".into())
        );
        assert_eq!(
            library.icon("work"),
            super::super::resource_list::Glyph::folder_default()
        );
        assert_eq!(display_name("work/Projects"), "Projects");
    }
}
