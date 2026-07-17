use crate::types::Collection;
use std::path::Path;

pub async fn import_all_from_path(path: &Path) -> Result<Collection, crate::Error> {
    let data = hypr_granola::importer::import_all_from_path(path).await?;
    Ok(data)
}
