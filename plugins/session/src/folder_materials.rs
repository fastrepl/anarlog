use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CatalogFolderMaterialRequest {
    pub folder_path: String,
    pub attachment_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TombstoneFolderMaterialRequest {
    pub folder_path: String,
    pub attachment_id: String,
}

pub async fn catalog_folder_material(
    pool: &SqlitePool,
    request: CatalogFolderMaterialRequest,
) -> Result<(), String> {
    let relative_path = format!("materials/{}", request.attachment_id);
    let metadata_id = uuid::Uuid::new_v4().to_string();

    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let updated = anlg_db_app::update_folder_attachment(
        &mut transaction,
        &request.filename,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &request.attachment_id,
        &request.folder_path,
        &relative_path,
    )
    .await
    .map_err(|error| error.to_string())?;

    let inserted = anlg_db_app::insert_folder_attachment(
        &mut transaction,
        &metadata_id,
        &request.folder_path,
        &request.filename,
        &relative_path,
        &request.content_type,
        request.size_bytes,
        &request.sha256,
        &request.attachment_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if updated + inserted != 1 {
        return Err("folder material is unavailable".to_string());
    }
    Ok(())
}

pub async fn tombstone_folder_material(
    pool: &SqlitePool,
    request: TombstoneFolderMaterialRequest,
) -> Result<(), String> {
    let relative_path = format!("materials/{}", request.attachment_id);
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let updated = anlg_db_app::tombstone_folder_attachment(
        &mut transaction,
        &request.folder_path,
        &relative_path,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    if updated != 1 {
        return Err("folder material is unavailable".to_string());
    }
    Ok(())
}
