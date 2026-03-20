use crossterm::event::KeyEvent;

pub(crate) enum Action {
    Key(KeyEvent),
    MeetingsLoaded(Vec<hypr_db_app::MeetingRow>),
    EventsLoaded(Vec<hypr_db_app::EventRow>),
    CalendarNotConfigured,
    LoadError(String),
}
