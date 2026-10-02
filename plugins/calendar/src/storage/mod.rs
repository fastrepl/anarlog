mod connection;
mod connection_sync;
mod identity;
mod ignored;
mod inventory;
mod participants;
mod selection;
pub(crate) mod transaction_utils;

pub use connection::{TombstoneCalendarConnectionRequest, tombstone_calendar_connection};
pub use connection_sync::{SyncCalendarConnectionEventsRequest, sync_calendar_connection_events};
pub use ignored::{UpdateIgnoredCalendarItemRequest, update_ignored_calendar_item};
pub use inventory::{ApplyCalendarInventoryRequest, apply_calendar_inventory};
pub use selection::{SetCalendarEnabledRequest, set_calendar_enabled};

use anlg_calendar_interface::CalendarProviderType;

fn provider_str(provider: CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}
