use chrono::SecondsFormat;
use sqlx::{Sqlite, Transaction};

pub(crate) fn js_iso8601_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) async fn rollback_error(transaction: Transaction<'_, Sqlite>, error: String) -> String {
    let _ = transaction.rollback().await;
    error
}
