mod as_is;
#[cfg(feature = "legacy-import")]
mod hyprnote;

pub use as_is::AsIsData;

use crate::types::{Collection, ImportSource, ImportSourceInfo, ImportStats, TransformKind};

pub async fn import_all(source: &ImportSource) -> Result<Collection, crate::Error> {
    match source.transform {
        TransformKind::HyprnoteV0 => {
            #[cfg(feature = "legacy-import")]
            {
                hyprnote::v0::import_all_from_path(&source.path).await
            }
            #[cfg(not(feature = "legacy-import"))]
            {
                Err(crate::Error::LegacyImportUnavailable)
            }
        }
        TransformKind::AsIs => as_is::load_data(&source.path),
    }
}

pub async fn import_stats(source: &ImportSource) -> Result<ImportStats, crate::Error> {
    match source.transform {
        TransformKind::HyprnoteV0 => {
            #[cfg(feature = "legacy-import")]
            {
                hyprnote::v0::import_stats_from_path(&source.path).await
            }
            #[cfg(not(feature = "legacy-import"))]
            {
                Err(crate::Error::LegacyImportUnavailable)
            }
        }
        TransformKind::AsIs => {
            let data = import_all(source).await?;
            Ok(ImportStats::from_data(&data))
        }
    }
}

pub fn all_sources() -> Vec<ImportSource> {
    #[cfg(not(feature = "legacy-import"))]
    {
        return Vec::new();
    }

    #[cfg(feature = "legacy-import")]
    [
        ImportSource::hyprnote_stable(),
        ImportSource::hyprnote_nightly(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn list_available_sources() -> Vec<ImportSourceInfo> {
    all_sources()
        .into_iter()
        .filter(|s| s.is_available())
        .map(|s| s.info())
        .collect()
}
