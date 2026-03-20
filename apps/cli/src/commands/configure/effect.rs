use hypr_db_app::CalendarRow;

use super::app::Tab;

pub enum Effect {
    Exit,
    LoadSettings,
    SaveProvider { tab: Tab, provider: String },
    LoadCalendars,
    SaveCalendars(Vec<CalendarRow>),
    CheckCalendarPermission,
}
