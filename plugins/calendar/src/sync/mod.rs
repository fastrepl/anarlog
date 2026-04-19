//! Calendar sync storage layer.
//!
//! - [`store`]  — backend-agnostic trait + domain types.
//! - [`json`]   — file-backed backend (today's implementation).
//! - [`source`] — provider-fetch pipeline that writes through the trait.

pub mod json;
pub mod source;
pub mod store;

pub use json::JsonCalendarSyncStore;
pub use source::PluginCalendarSyncSource;
pub use store::CalendarSyncStore;
