use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use super::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum IgnoredCalendarItemKind {
    Events,
    Series,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct UpdateIgnoredCalendarItemRequest {
    pub kind: IgnoredCalendarItemKind,
    pub item_id: String,
    pub ignored: bool,
}

pub async fn update_ignored_calendar_item(
    pool: &SqlitePool,
    request: UpdateIgnoredCalendarItemRequest,
) -> Result<(), String> {
    let kind = match request.kind {
        IgnoredCalendarItemKind::Events => anlg_db_app::IgnoredCalendarItemKind::Events,
        IgnoredCalendarItemKind::Series => anlg_db_app::IgnoredCalendarItemKind::Series,
    };
    anlg_db_app::update_ignored_calendar_item(
        pool,
        kind,
        &request.item_id,
        request.ignored,
        &js_iso8601_timestamp(),
    )
    .await
}
