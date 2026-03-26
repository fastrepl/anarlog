pub(crate) mod storage;

use axum::{Router, routing::post};

pub fn router() -> Router {
    Router::new()
        .route("/files", post(storage::list_files))
        .route("/files/get", post(storage::get_file))
        .route("/files/download", post(storage::download_file))
        .route("/files/create-folder", post(storage::create_folder))
        .route("/files/delete", post(storage::delete_file))
        .route("/files/upload", post(storage::upload_file))
        .route("/files/update", post(storage::update_metadata))
}
