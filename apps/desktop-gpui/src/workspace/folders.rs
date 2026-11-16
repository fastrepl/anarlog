use std::{io::Read, path::PathBuf, sync::Arc};

use anlg_db_execute::DbExecutor;
use anlg_fs_sync_core::{FsSyncCore, normalize_folder_path};
use desktop_runtime::{CancellationToken, Reply, Result, RuntimeHandle, ServiceError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    mutations::{Draft, NOW, cas, failure, statement, transaction},
    ports::CatalogRow,
};

#[derive(Clone)]
pub struct Material {
    pub id: Arc<str>,
    pub name: Arc<str>,
}

pub enum MaterialCommand {
    Import(PathBuf),
    Remove(Arc<str>),
}

pub fn notes(
    runtime: &RuntimeHandle,
    folder: Arc<str>,
    offset: u32,
    cancel: CancellationToken,
) -> Result<Reply<desktop_runtime::LibraryPage>> {
    runtime.read(cancel,move |services| async move {
        let rows=services.executor.execute("SELECT id,substr(title,1,4096) AS title,created_at,updated_at,folder_path FROM sessions WHERE deleted_at IS NULL AND (folder_path=?1 OR substr(folder_path,1,length(?1)+1)=?1 || '/') ORDER BY created_at DESC,id LIMIT 101 OFFSET ?2".into(),vec![json!(folder),json!(offset)]).await.map_err(failure)?;
        let has_more=rows.len()>100;
        let ids=rows.iter().take(100).filter_map(|row| row["id"].as_str()).collect::<Vec<_>>();
        let mut tags=services.session_tag_lines(&ids).await?;
        let items=rows.into_iter().take(100).map(|row| desktop_runtime::SessionSummary{
            id:desktop_runtime::SessionId(row["id"].as_str().unwrap_or("").into()),
            title:row["title"].as_str().unwrap_or("").into(),
            created_at:row["created_at"].as_str().unwrap_or("").into(),
            updated_at:row["updated_at"].as_str().unwrap_or("").into(),
            folder_path:row["folder_path"].as_str().unwrap_or("").into(),
            tag_line:tags.remove(row["id"].as_str().unwrap_or("")).unwrap_or_default(),
        }).collect::<Vec<_>>().into();
        Ok(desktop_runtime::LibraryPage{items,offset,has_more})
    })
}

pub fn batch(
    runtime: &RuntimeHandle,
    ids: Arc<[Arc<str>]>,
    destination: Option<Arc<str>>,
) -> Result<Reply<String>> {
    runtime.submit(move |services| async move {
        if ids.is_empty() || ids.len()>100 {return Err(failure("Select between 1 and 100 folders"));}
        let rows=services.executor.execute("SELECT * FROM folders WHERE id IN(SELECT value FROM json_each(?)) AND deleted_at IS NULL ORDER BY path".into(),vec![json!(ids).to_string().into()]).await.map_err(failure)?;
        if rows.len()!=ids.len() {return Err(ServiceError::Conflict);}
        let destination=destination.map(|path| normalize_folder_path(&path).map_err(failure)).transpose()?;
        let mut roots=Vec::<Value>::new();
        for row in rows {
            let path=row["path"].as_str().ok_or(ServiceError::Conflict)?;
            if roots.iter().any(|root| path.strip_prefix(root["path"].as_str().unwrap_or("")).is_some_and(|suffix| suffix.starts_with('/'))) {continue;}
            if destination.as_ref().is_some_and(|target| target==path || target.starts_with(&format!("{path}/"))) {return Err(failure("Cannot move a folder into itself"));}
            roots.push(row);
        }
        let mut completed=0;
        for base in roots {
            let path=base["path"].as_str().ok_or(ServiceError::Conflict)?;
            let target=destination.as_ref().map(|parent| if parent.is_empty() {path.rsplit('/').next().unwrap_or(path).to_owned()} else {format!("{parent}/{}",path.rsplit('/').next().unwrap_or(path))});
            let draft=Draft {
                row: CatalogRow {id:base["id"].as_str().ok_or(ServiceError::Conflict)?.into(),kind:"folder".into(),title:path.into(),subtitle:"".into(),pinned:false,self_contact:false},
                fields:vec![
                    super::mutations::Field{key:"path",label:"Folder",value:target.clone().unwrap_or_else(||path.into())},
                    super::mutations::Field{key:"instructions",label:"Instructions",value:base["instructions"].as_str().unwrap_or("").into()},
                ],
                base:Some(base.clone()),groups:None,groups_dirty:false,
            };
            let result=if target.is_some() {save(&services.executor,draft).await} else {remove(&services.executor,draft).await};
            if let Err(error)=result {return Err(failure(format!("Changed {completed} folder roots; stopped at {path}: {error}")));}
            completed+=1;
        }
        Ok(format!("{} {completed} folder roots",if destination.is_some() {"Moved"} else {"Deleted"}))
    })
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

pub(super) async fn save(executor: &DbExecutor, mut draft: Draft) -> Result<CatalogRow> {
    let path = draft
        .fields
        .iter()
        .find(|field| field.key == "path")
        .map(|field| field.value.trim())
        .unwrap_or("");
    let path = normalize_folder_path(path).map_err(failure)?;
    if path.is_empty() {
        return Err(failure("A folder name is required"));
    }
    let instructions = draft
        .fields
        .iter()
        .find(|field| field.key == "instructions")
        .map(|field| field.value.as_str())
        .unwrap_or("");
    let fs = filesystem(executor).await?;
    let old = draft.base.as_ref().and_then(|base| base["path"].as_str());
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
    let mut statements = Vec::new();
    if draft.base.is_some() {
        let (predicate, mut params) = cas(&draft)?;
        let mut values = vec![json!(path), json!(instructions)];
        values.append(&mut params);
        statements.push(statement(
            format!("UPDATE folders SET path=?,instructions=?,updated_at={NOW} WHERE {predicate}"),
            values,
            Some(1),
        ));
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
    } else {
        statements.push(statement(format!("INSERT INTO folders(id,path,instructions,created_at,updated_at) VALUES (?,?,?,{NOW},{NOW})"),vec![json!(draft.row.id),json!(path),json!(instructions)],Some(1)));
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
    draft.row.title = path.into();
    Ok(draft.row)
}

pub(super) async fn remove(executor: &DbExecutor, draft: Draft) -> Result<CatalogRow> {
    let path = draft
        .base
        .as_ref()
        .and_then(|base| base["path"].as_str())
        .ok_or(ServiceError::Conflict)?;
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
    let (predicate, params) = cas(&draft)?;
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
    Ok(draft.row)
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
    let rows = executor.execute("SELECT id,folder_path,updated_at FROM sessions WHERE id IN(SELECT value FROM json_each(?)) AND deleted_at IS NULL AND locked=0".into(),vec![json!(ids).to_string().into()]).await.map_err(failure)?;
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
        statements.push(statement(format!("UPDATE sessions SET folder_path=?,updated_at={NOW} WHERE id=? AND folder_path=? AND updated_at=? AND deleted_at IS NULL AND locked=0"),vec![json!(path),json!(id),json!(old),row["updated_at"].clone()],Some(1)));
    }
    if let Err(error) = transaction(executor, statements).await {
        for (id, old) in moved.iter().rev() {
            fs.move_session(id, &path, old).map_err(failure)?;
        }
        return Err(error);
    }
    Ok(())
}
