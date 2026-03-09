pub trait NangoIntegrationId: Send + Sync + 'static {
    const ID: &'static str;
}

pub struct GoogleCalendar;

impl NangoIntegrationId for GoogleCalendar {
    const ID: &'static str = "google-calendar";
}

pub struct GoogleDrive;

impl NangoIntegrationId for GoogleDrive {
    const ID: &'static str = "google-drive";
}

pub struct OutlookCalendar;

impl NangoIntegrationId for OutlookCalendar {
    const ID: &'static str = "outlook-calendar";
}
