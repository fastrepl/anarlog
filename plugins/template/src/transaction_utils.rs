use chrono::SecondsFormat;
use sqlx::{Sqlite, Transaction};

pub(crate) fn js_iso8601_timestamp() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) async fn rollback_row_count_mismatch(
    transaction: Transaction<'_, Sqlite>,
    statement_index: usize,
    actual: u64,
    expected: u64,
) -> String {
    let _ = transaction.rollback().await;
    format!("transaction statement {statement_index} affected {actual} rows; expected {expected}")
}
