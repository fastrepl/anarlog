//! Calendar sync plugin glue.

pub mod json;
pub mod source;
pub mod store;

pub use json::JsonCalendarSyncStore;
pub use source::PluginCalendarSyncSource;
