use crate::commands::connect::effect::CalendarSaveData;
use crate::commands::connect::{ConnectProvider, ConnectionType};
use crate::commands::model::Commands as ModelCommands;

pub(crate) enum Effect {
    Launch(super::EntryCommand),
    LoadMeetings,
    LoadModels,
    LoadTimelineContacts,
    LoadTimelineEntries(String),
    SaveConnect {
        connection_types: Vec<ConnectionType>,
        provider: ConnectProvider,
        base_url: Option<String>,
        api_key: Option<String>,
    },
    CheckCalendarPermission,
    RequestCalendarPermission,
    ResetCalendarPermission,
    LoadCalendars,
    SaveCalendars(CalendarSaveData),
    OpenAuth,
    OpenBug,
    OpenHello,
    OpenDesktop,
    RunModel(ModelCommands),
    Exit,
}
