use crossterm::event::KeyEvent;
use hypr_db_app::{HumanRow, OrganizationRow, TimelineRow};

pub(crate) enum Action {
    Key(KeyEvent),
    ContactsLoaded {
        orgs: Vec<OrganizationRow>,
        humans: Vec<HumanRow>,
    },
    ContactsLoadError(String),
    EntriesLoaded(Vec<TimelineRow>),
    EntriesLoadError(String),
}
