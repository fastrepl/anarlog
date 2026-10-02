use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EnsureFolderCatalogRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RenameFolderCatalogRequest {
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DeleteFolderCatalogRequest {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct UpdateFolderInstructionsRequest {
    pub path: String,
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct UpdateFolderWorkspaceRequest {
    pub path: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct UpdateFolderIconRequest {
    pub path: String,
    pub icon_json: String,
}

async fn ensure_folder(conn: &mut SqliteConnection, path: &str) -> Result<(), String> {
    anlg_db_app::revive_folder(conn, path)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::insert_folder_if_missing(conn, &uuid::Uuid::new_v4().to_string(), path)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub async fn ensure_folder_catalog(
    pool: &SqlitePool,
    request: EnsureFolderCatalogRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    for path in &request.paths {
        ensure_folder(&mut transaction, path).await?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn rename_folder_catalog(
    pool: &SqlitePool,
    request: RenameFolderCatalogRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::rename_folder_row(&mut transaction, &request.new_path, &request.old_path)
        .await
        .map_err(|error| error.to_string())?;
    ensure_folder(&mut transaction, &request.new_path).await?;
    anlg_db_app::rewrite_folder_attachment_paths(
        &mut transaction,
        &request.old_path,
        &request.new_path,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::rewrite_session_folder_paths(
        &mut transaction,
        &request.old_path,
        &request.new_path,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn delete_folder_catalog(
    pool: &SqlitePool,
    request: DeleteFolderCatalogRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::tombstone_folders_under(&mut transaction, &request.path)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::tombstone_folder_attachments_under(&mut transaction, &request.path)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::clear_session_folder_paths(&mut transaction, &request.path)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn update_folder_instructions(
    pool: &SqlitePool,
    request: UpdateFolderInstructionsRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::update_folder_instructions(&mut transaction, &request.path, &request.instructions)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn update_folder_workspace(
    pool: &SqlitePool,
    request: UpdateFolderWorkspaceRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::update_folder_workspace(&mut transaction, &request.path, &request.workspace_id)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

pub async fn update_folder_icon(
    pool: &SqlitePool,
    request: UpdateFolderIconRequest,
) -> Result<(), String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    ensure_folder(&mut transaction, &request.path).await?;
    anlg_db_app::update_folder_icon(&mut transaction, &request.path, &request.icon_json)
        .await
        .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}
