use anlg_meeting_capture::{CaptureProviderKind, MeetingReference};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturePolicy {
    pub workspace_id: String,
    pub capture_enabled: bool,
    pub allowed_providers: Vec<CaptureProviderKind>,
    pub bot_name: String,
    #[serde(default)]
    pub disclosure_text: Option<String>,
    pub skip_if_desktop_capture: bool,
}

impl CapturePolicy {
    pub fn default_off(workspace_id: impl Into<String>) -> Self {
        Self {
            workspace_id: workspace_id.into(),
            capture_enabled: false,
            allowed_providers: vec![CaptureProviderKind::Anarlog],
            bot_name: "Anarlog Notetaker".into(),
            disclosure_text: None,
            skip_if_desktop_capture: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalendarEventInput {
    pub calendar_event_id: String,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    #[serde(default)]
    pub ends_at: Option<DateTime<Utc>>,
    pub meeting: MeetingReference,
    pub provider: CaptureProviderKind,
    pub owner_user_id: String,
    #[serde(default)]
    pub opt_out: bool,
    #[serde(default)]
    pub desktop_capture_active: bool,
    #[serde(default)]
    pub canceled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledCaptureStatus {
    Pending,
    Skipped,
    Canceled,
    Dispatched,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledCapture {
    pub workspace_id: String,
    pub calendar_event_id: String,
    pub job_id: Option<String>,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub meeting: MeetingReference,
    pub provider: CaptureProviderKind,
    pub owner_user_id: String,
    pub status: ScheduledCaptureStatus,
    pub skip_reason: Option<String>,
    pub bot_name: String,
    pub disclosure_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleDecision {
    Pending,
    Skipped(&'static str),
    Canceled(&'static str),
}

pub fn decide_schedule(policy: &CapturePolicy, event: &CalendarEventInput) -> ScheduleDecision {
    if event.canceled {
        return ScheduleDecision::Canceled("calendar_event_canceled");
    }
    if event.opt_out {
        return ScheduleDecision::Canceled("event_opt_out");
    }
    if !policy.capture_enabled {
        return ScheduleDecision::Skipped("capture_policy_disabled");
    }
    if !policy.allowed_providers.contains(&event.provider) {
        return ScheduleDecision::Skipped("provider_not_allowed");
    }
    if event.desktop_capture_active && policy.skip_if_desktop_capture {
        return ScheduleDecision::Skipped("desktop_capture_duplicate");
    }
    ScheduleDecision::Pending
}

pub fn scheduled_job_id(calendar_event_id: &str) -> String {
    let mut job_id = String::from("cal-");
    for byte in calendar_event_id.bytes() {
        if job_id.len() >= 128 {
            break;
        }
        if byte.is_ascii_alphanumeric() || b"-_.".contains(&byte) {
            job_id.push(byte as char);
        } else {
            job_id.push('-');
        }
    }
    if job_id == "cal-" {
        "cal-event".into()
    } else {
        job_id
    }
}

#[cfg(test)]
mod tests {
    use anlg_meeting_capture::{MeetingPlatform, MeetingReference};
    use chrono::DateTime;

    use super::*;

    fn event() -> CalendarEventInput {
        CalendarEventInput {
            calendar_event_id: "evt-1".into(),
            title: "Standup".into(),
            starts_at: DateTime::parse_from_rfc3339("2026-08-21T15:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            ends_at: None,
            meeting: MeetingReference {
                platform: MeetingPlatform::GoogleMeet,
                url: "https://meet.google.com/aaa-bbbb-ccc".into(),
                external_id: None,
                calendar_event_id: Some("evt-1".into()),
            },
            provider: CaptureProviderKind::Anarlog,
            owner_user_id: "owner-a".into(),
            opt_out: false,
            desktop_capture_active: false,
            canceled: false,
        }
    }

    #[test]
    fn default_policy_does_not_schedule_bots() {
        let policy = CapturePolicy::default_off("workspace-a");
        assert_eq!(
            decide_schedule(&policy, &event()),
            ScheduleDecision::Skipped("capture_policy_disabled")
        );
    }

    #[test]
    fn enabled_policy_schedules_one_pending_job_unless_opted_out() {
        let mut policy = CapturePolicy::default_off("workspace-a");
        policy.capture_enabled = true;
        assert_eq!(
            decide_schedule(&policy, &event()),
            ScheduleDecision::Pending
        );

        let mut opted_out = event();
        opted_out.opt_out = true;
        assert_eq!(
            decide_schedule(&policy, &opted_out),
            ScheduleDecision::Canceled("event_opt_out")
        );

        let mut desktop = event();
        desktop.desktop_capture_active = true;
        assert_eq!(
            decide_schedule(&policy, &desktop),
            ScheduleDecision::Skipped("desktop_capture_duplicate")
        );
    }

    #[test]
    fn job_ids_are_stable_identifiers_for_the_same_calendar_event() {
        assert_eq!(scheduled_job_id("evt-1"), "cal-evt-1");
        assert_eq!(scheduled_job_id("evt-1"), scheduled_job_id("evt-1"));
    }
}
