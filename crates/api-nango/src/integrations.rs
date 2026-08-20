use std::collections::HashMap;

use anlg_nango::{ConnectionConfigOverride, IntegrationConfigDefault};

pub trait NangoIntegrationId: Send + Sync + 'static {
    const ID: &'static str;
}

pub const GOOGLE_CALENDAR_OAUTH_SCOPES: &str = "https://www.googleapis.com/auth/calendar.readonly https://www.googleapis.com/auth/calendar.events.readonly";

pub const OUTLOOK_OAUTH_SCOPES: &str = "offline_access User.Read Calendars.Read";

pub const ZOOM_OAUTH_SCOPES: &str =
    "user:read:user cloud_recording:read:list_user_recordings meeting:read:summary";

pub const FATHOM_OAUTH_SCOPES: &str = "public_api";

pub const WEBEX_OAUTH_SCOPES: &str =
    "spark:people_read meeting:schedules_read meeting:transcripts_read";

pub const GOOGLE_MEET_OAUTH_SCOPES: &str =
    "https://www.googleapis.com/auth/meetings.space.readonly";

pub const MICROSOFT_TEAMS_OAUTH_SCOPES: &str =
    "offline_access User.Read Calendars.Read OnlineMeetings.Read OnlineMeetingTranscript.Read.All";

pub fn oauth_scopes_override(integration_id: &str) -> Option<&'static str> {
    match integration_id {
        GoogleCalendar::ID => Some(GOOGLE_CALENDAR_OAUTH_SCOPES),
        Outlook::ID => Some(OUTLOOK_OAUTH_SCOPES),
        Zoom::ID => Some(ZOOM_OAUTH_SCOPES),
        Fathom::ID => Some(FATHOM_OAUTH_SCOPES),
        Webex::ID => Some(WEBEX_OAUTH_SCOPES),
        GoogleMeet::ID => Some(GOOGLE_MEET_OAUTH_SCOPES),
        MicrosoftTeams::ID => Some(MICROSOFT_TEAMS_OAUTH_SCOPES),
        _ => None,
    }
}

pub fn integrations_config_defaults(
    integration_id: &str,
) -> Option<HashMap<String, IntegrationConfigDefault>> {
    let scopes = oauth_scopes_override(integration_id)?;
    Some(HashMap::from([(
        integration_id.to_string(),
        IntegrationConfigDefault {
            user_scopes: None,
            connection_config: Some(ConnectionConfigOverride {
                oauth_scopes_override: Some(scopes.to_string()),
            }),
        },
    )]))
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

pub struct Slack;

impl NangoIntegrationId for Slack {
    const ID: &'static str = "slack";
}

pub struct Discord;

impl NangoIntegrationId for Discord {
    const ID: &'static str = "discord";
}

pub struct Notion;

impl NangoIntegrationId for Notion {
    const ID: &'static str = "notion";
}

pub struct Zoom;

impl NangoIntegrationId for Zoom {
    const ID: &'static str = "zoom";
}

pub struct Fathom;

impl NangoIntegrationId for Fathom {
    const ID: &'static str = "fathom";
}

pub struct Webex;

impl NangoIntegrationId for Webex {
    const ID: &'static str = "webex";
}

pub struct GoogleMeet;

impl NangoIntegrationId for GoogleMeet {
    const ID: &'static str = "google-meet";
}

pub struct MicrosoftTeams;

impl NangoIntegrationId for MicrosoftTeams {
    const ID: &'static str = "microsoft-teams";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_connect_requests_readonly_google_scopes() {
        let defaults = integrations_config_defaults(GoogleCalendar::ID).unwrap();
        let config = defaults.get(GoogleCalendar::ID).unwrap();
        let scopes = config
            .connection_config
            .as_ref()
            .and_then(|c| c.oauth_scopes_override.as_deref())
            .unwrap();

        assert_eq!(scopes, GOOGLE_CALENDAR_OAUTH_SCOPES);
        assert!(
            !scopes.split_whitespace().any(|scope| {
                scope.ends_with("/calendar") || scope.ends_with("/calendar.events")
            }),
            "calendar connect must not request write scopes: {scopes}"
        );
    }

    #[test]
    fn calendar_connect_requests_readonly_outlook_scopes() {
        let defaults = integrations_config_defaults(Outlook::ID).unwrap();
        let scopes = defaults
            .get(Outlook::ID)
            .unwrap()
            .connection_config
            .as_ref()
            .and_then(|c| c.oauth_scopes_override.as_deref())
            .unwrap();

        assert_eq!(scopes, OUTLOOK_OAUTH_SCOPES);
        assert!(scopes.contains("Calendars.Read"));
        assert!(!scopes.contains("Calendars.ReadWrite"));
    }

    #[test]
    fn zoom_connect_requests_readonly_recording_scopes() {
        let defaults = integrations_config_defaults(Zoom::ID).unwrap();
        let scopes = defaults
            .get(Zoom::ID)
            .unwrap()
            .connection_config
            .as_ref()
            .and_then(|c| c.oauth_scopes_override.as_deref())
            .unwrap();

        assert_eq!(scopes, ZOOM_OAUTH_SCOPES);
        assert!(scopes.contains("cloud_recording:read:list_user_recordings"));
        assert!(scopes.contains("meeting:read:summary"));
        assert!(!scopes.contains("recording:write"));
    }

    #[test]
    fn meeting_import_connect_requests_readonly_scopes() {
        assert_eq!(
            integrations_config_defaults(Fathom::ID)
                .unwrap()
                .get(Fathom::ID)
                .unwrap()
                .connection_config
                .as_ref()
                .and_then(|c| c.oauth_scopes_override.as_deref())
                .unwrap(),
            FATHOM_OAUTH_SCOPES
        );
        assert!(
            oauth_scopes_override(Webex::ID)
                .unwrap()
                .contains("meeting:transcripts_read")
        );
        assert!(
            oauth_scopes_override(GoogleMeet::ID)
                .unwrap()
                .contains("meetings.space.readonly")
        );
        let teams = oauth_scopes_override(MicrosoftTeams::ID).unwrap();
        assert!(teams.contains("OnlineMeetingTranscript.Read.All"));
        assert!(teams.contains("Calendars.Read"));
        assert!(!teams.contains("Calendars.ReadWrite"));
    }

    #[test]
    fn other_integrations_keep_dashboard_scopes() {
        assert!(integrations_config_defaults(GitHub::ID).is_none());
        assert!(integrations_config_defaults(Slack::ID).is_none());
        assert!(integrations_config_defaults("unknown").is_none());
    }
}
