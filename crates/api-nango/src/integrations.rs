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

pub struct GoogleMail;

impl NangoIntegrationId for GoogleMail {
    const ID: &'static str = "google-mail";
}

pub struct OutlookCalendar;

impl NangoIntegrationId for OutlookCalendar {
    const ID: &'static str = "outlook-calendar";
}

pub struct GitHub;

impl NangoIntegrationId for GitHub {
    const ID: &'static str = "github";
}

pub struct Linear;

impl NangoIntegrationId for Linear {
    const ID: &'static str = "linear";
}
