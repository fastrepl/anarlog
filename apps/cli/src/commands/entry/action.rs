use crossterm::event::KeyEvent;

use crate::commands::connect::ConnectionType;
use crate::commands::connect::runtime::RuntimeEvent as ConnectRuntimeEvent;
use crate::commands::model::list::ModelRow;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    SubmitCommand(String),
    StatusMessage(String),
    ConnectRuntime(ConnectRuntimeEvent),
    MeetingsLoaded(Vec<hypr_db_app::MeetingRow>),
    MeetingsLoadError(String),
    EventsLoaded(Vec<hypr_db_app::EventRow>),
    CalendarNotConfigured,
    ModelsLoaded(Vec<ModelRow>),
    ModelsLoadError(String),
    ConnectSaved {
        connection_types: Vec<ConnectionType>,
        provider_id: String,
    },
    TimelineContactsLoaded {
        orgs: Vec<hypr_db_app::OrganizationRow>,
        humans: Vec<hypr_db_app::HumanRow>,
    },
    TimelineContactsLoadError(String),
    TimelineEntriesLoaded(Vec<hypr_db_app::TimelineRow>),
    TimelineEntriesLoadError(String),
}
