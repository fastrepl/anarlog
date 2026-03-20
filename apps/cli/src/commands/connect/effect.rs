use super::{ConnectProvider, ConnectionType};

use super::runtime::CalendarItem;

pub(crate) struct SaveData {
    pub connection_types: Vec<ConnectionType>,
    pub provider: ConnectProvider,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
}

pub(crate) struct CalendarSaveData {
    pub provider: String,
    pub items: Vec<(CalendarItem, bool)>,
}

pub(crate) enum Effect {
    Save(SaveData),
    Exit,
    CheckCalendarPermission,
    RequestCalendarPermission,
    ResetCalendarPermission,
    LoadCalendars,
    SaveCalendars(CalendarSaveData),
}
