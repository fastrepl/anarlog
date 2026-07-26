use crate::types::{Collection, ImportStats};
use std::path::Path;
use std::time::Duration;

const SCAN_TIMEOUT: Duration = Duration::from_secs(30);
const IMPORT_TIMEOUT: Duration = Duration::from_secs(300);

pub async fn import_all_from_path(path: &Path) -> Result<Collection, crate::Error> {
    let data = tokio::time::timeout(
        IMPORT_TIMEOUT,
        legacy_db_parser::v0::parse_from_sqlite(path),
    )
    .await
    .map_err(|_| crate::Error::Timeout {
        operation: "Hyprnote legacy import",
        seconds: IMPORT_TIMEOUT.as_secs(),
    })??;
    Ok(data)
}

pub async fn import_stats_from_path(path: &Path) -> Result<ImportStats, crate::Error> {
    let stats = tokio::time::timeout(
        SCAN_TIMEOUT,
        legacy_db_parser::v0::parse_stats_from_sqlite(path),
    )
    .await
    .map_err(|_| crate::Error::Timeout {
        operation: "Hyprnote legacy import scan",
        seconds: SCAN_TIMEOUT.as_secs(),
    })??;

    Ok(stats.into())
}
