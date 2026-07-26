use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::storage::list_files,
        crate::routes::storage::get_file,
        crate::routes::storage::download_file,
        crate::routes::storage::create_folder,
        crate::routes::storage::delete_file,
        crate::routes::storage::upload_file,
        crate::routes::storage::update_metadata,
    ),
    components(
        schemas(
            crate::routes::storage::ListFilesRequest,
            crate::routes::storage::ListFilesResponse,
            crate::routes::storage::GetFileRequest,
            crate::routes::storage::GetFileResponse,
            crate::routes::storage::DownloadFileRequest,
            crate::routes::storage::DownloadFileResponse,
            crate::routes::storage::CreateFolderRequest,
            crate::routes::storage::CreateFolderResponse,
            crate::routes::storage::DeleteFileRequest,
            crate::routes::storage::UploadFileRequest,
            crate::routes::storage::UploadFileResponse,
            crate::routes::storage::UpdateMetadataRequest,
            crate::routes::storage::UpdateMetadataResponse,
        )
    ),
    tags(
        (name = "storage", description = "Storage management (Google Drive)")
    )
)]
struct ApiDoc;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
