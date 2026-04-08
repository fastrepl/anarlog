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

pub struct Outlook;

impl NangoIntegrationId for Outlook {
    const ID: &'static str = "outlook";
}

pub struct GitHub;

impl NangoIntegrationId for GitHub {
    const ID: &'static str = "github";
}

pub struct Linear;

impl NangoIntegrationId for Linear {
    const ID: &'static str = "linear";
}
