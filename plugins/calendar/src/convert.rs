use hypr_apple_calendar::types::{
    AppleCalendar, AppleEvent, EventStatus as AppleEventStatus, Participant, ParticipantRole,
    ParticipantStatus,
};
use hypr_calendar_interface::{
    AttendeeRole, AttendeeStatus, CalendarEvent, CalendarListItem, CalendarProviderType,
    EventAttendee, EventPerson, EventStatus,
};
use hypr_google_calendar::{
    AccessRole as GoogleAccessRole, Attendee as GoogleAttendee, AttendeeResponseStatus,
    CalendarListEntry as GoogleCalendar, Event as GoogleEvent, EventDateTime,
    EventStatus as GoogleEventStatus,
};
use hypr_outlook_calendar::{
    Attendee as OutlookAttendee, AttendeeType, Calendar as OutlookCalendar, Event as OutlookEvent,
    EventShowAs, ResponseType as OutlookResponseType,
};

pub fn convert_google_calendars(calendars: Vec<GoogleCalendar>) -> Vec<CalendarListItem> {
    calendars
        .into_iter()
        .map(|calendar| {
            let can_edit = calendar
                .access_role
                .as_ref()
                .map(|role| matches!(role, GoogleAccessRole::Writer | GoogleAccessRole::Owner));
            let raw = serde_json::to_string(&calendar).unwrap_or_default();

            CalendarListItem {
                provider: CalendarProviderType::Google,
                id: calendar.id,
                title: calendar
                    .summary_override
                    .or(calendar.summary)
                    .unwrap_or_else(|| "Untitled".to_string()),
                source: calendar.data_owner,
                color: calendar.background_color,
                is_primary: calendar.primary,
                can_edit,
                raw,
            }
        })
        .collect()
}

pub fn convert_outlook_calendars(calendars: Vec<OutlookCalendar>) -> Vec<CalendarListItem> {
    calendars
        .into_iter()
        .map(|calendar| {
            let source = calendar
                .owner
                .as_ref()
                .and_then(|owner| owner.name.clone().or(owner.address.clone()));
            let raw = serde_json::to_string(&calendar).unwrap_or_default();

            CalendarListItem {
                provider: CalendarProviderType::Outlook,
                id: calendar.id,
                title: calendar.name.unwrap_or_else(|| "Untitled".to_string()),
                source,
                color: calendar.hex_color,
                is_primary: calendar.is_default_calendar,
                can_edit: calendar.can_edit,
                raw,
            }
        })
        .collect()
}

pub fn convert_apple_calendars(calendars: Vec<AppleCalendar>) -> Vec<CalendarListItem> {
    calendars
        .into_iter()
        .map(|calendar| {
            let raw = serde_json::to_string(&calendar).unwrap_or_default();

            CalendarListItem {
                provider: CalendarProviderType::Apple,
                id: calendar.id,
                title: calendar.title,
                source: Some(calendar.source.title),
                color: calendar.color.map(apple_color_to_css),
                is_primary: None,
                can_edit: Some(calendar.allows_content_modifications && !calendar.is_immutable),
                raw,
            }
        })
        .collect()
}

fn apple_color_to_css(color: hypr_apple_calendar::types::CalendarColor) -> String {
    format!(
        "rgba({}, {}, {}, {})",
        (color.red * 255.0).round(),
        (color.green * 255.0).round(),
        (color.blue * 255.0).round(),
        color.alpha,
    )
}

pub fn convert_google_events(events: Vec<GoogleEvent>, calendar_id: &str) -> Vec<CalendarEvent> {
    events
        .into_iter()
        .map(|e| convert_google_event(e, calendar_id))
        .collect()
}

pub fn convert_outlook_events(events: Vec<OutlookEvent>, calendar_id: &str) -> Vec<CalendarEvent> {
    events
        .into_iter()
        .map(|e| convert_outlook_event(e, calendar_id))
        .collect()
}

pub fn convert_apple_events(events: Vec<AppleEvent>) -> Vec<CalendarEvent> {
    events.into_iter().map(convert_apple_event).collect()
}

fn convert_google_event(event: GoogleEvent, calendar_id: &str) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();

    let is_all_day = event
        .start
        .as_ref()
        .is_some_and(|s| s.date.is_some() && s.date_time.is_none());

    let started_at = event
        .start
        .as_ref()
        .and_then(event_datetime_to_iso)
        .unwrap_or_default();
    let ended_at = event
        .end
        .as_ref()
        .and_then(event_datetime_to_iso)
        .unwrap_or_default();
    let timezone = event.start.as_ref().and_then(|s| s.time_zone.clone());

    let organizer = event.organizer.as_ref().map(|o| EventPerson {
        name: o.display_name.clone(),
        email: o.email.clone(),
        is_current_user: o.is_self.unwrap_or(false),
    });

    let attendees = event
        .attendees
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(convert_google_attendee)
        .collect();

    let meeting_link = event
        .hangout_link
        .clone()
        .or_else(|| extract_video_entry_point(&event));

    let has_recurrence_rules = event.recurring_event_id.is_some()
        || event.recurrence.as_ref().is_some_and(|r| !r.is_empty());

    CalendarEvent {
        id: event.id,
        calendar_id: calendar_id.to_string(),
        provider: CalendarProviderType::Google,
        external_id: event.ical_uid.unwrap_or_default(),
        title: event.summary.unwrap_or_default(),
        description: event.description,
        location: event.location,
        url: event.html_link,
        meeting_link,
        started_at,
        ended_at,
        timezone,
        is_all_day,
        status: convert_google_status(event.status),
        organizer,
        attendees,
        has_recurrence_rules,
        recurring_event_id: event.recurring_event_id,
        raw,
    }
}

fn convert_outlook_event(event: OutlookEvent, calendar_id: &str) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();

    let started_at = event
        .start
        .as_ref()
        .map(|start| start.date_time.clone())
        .unwrap_or_default();
    let ended_at = event
        .end
        .as_ref()
        .map(|end| end.date_time.clone())
        .unwrap_or_default();
    let timezone = event
        .start
        .as_ref()
        .and_then(|start| start.time_zone.clone());

    let organizer = event.organizer.as_ref().map(|organizer| EventPerson {
        name: organizer
            .email_address
            .as_ref()
            .and_then(|email| email.name.clone()),
        email: organizer
            .email_address
            .as_ref()
            .and_then(|email| email.address.clone()),
        is_current_user: event.is_organizer.unwrap_or(false),
    });

    let attendees = event
        .attendees
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(convert_outlook_attendee)
        .collect();

    let meeting_link = event.online_meeting_url.clone().or_else(|| {
        event
            .online_meeting
            .as_ref()
            .and_then(|meeting| meeting.join_url.clone())
    });

    CalendarEvent {
        id: event.id,
        calendar_id: calendar_id.to_string(),
        provider: CalendarProviderType::Outlook,
        external_id: event.ical_uid.unwrap_or_default(),
        title: event.subject.unwrap_or_default(),
        description: event.body.and_then(|body| body.content),
        location: event.location.and_then(|location| location.display_name),
        url: event.web_link,
        meeting_link,
        started_at,
        ended_at,
        timezone,
        is_all_day: event.is_all_day.unwrap_or(false),
        status: convert_outlook_status(event.is_cancelled, event.show_as),
        organizer,
        attendees,
        has_recurrence_rules: event.recurrence.is_some() || event.series_master_id.is_some(),
        recurring_event_id: event.series_master_id,
        raw,
    }
}

fn convert_apple_event(event: AppleEvent) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();

    let id = if event.has_recurrence_rules {
        let date = event.occurrence_date.as_ref().unwrap_or(&event.start_date);
        let day = local_date_string(date, event.time_zone.as_deref());
        format!("{}:{}", event.event_identifier, day)
    } else {
        event.event_identifier.clone()
    };

    let organizer = event.organizer.as_ref().map(convert_person);
    let attendees = event.attendees.iter().map(convert_apple_attendee).collect();

    let recurring_event_id = if event.has_recurrence_rules {
        Some(
            event
                .recurrence
                .expect("event with has_recurrence_rules: true must have a recurrence")
                .series_identifier
                .clone(),
        )
    } else {
        None
    };

    CalendarEvent {
        id,
        calendar_id: event.calendar.id,
        provider: CalendarProviderType::Apple,
        external_id: event.external_identifier,
        title: event.title,
        description: event.notes,
        location: event.location,
        url: event.url,
        meeting_link: None,
        started_at: event.start_date.to_rfc3339(),
        ended_at: event.end_date.to_rfc3339(),
        timezone: event.time_zone,
        is_all_day: event.is_all_day,
        status: convert_apple_status(event.status),
        organizer,
        attendees,
        has_recurrence_rules: event.has_recurrence_rules,
        recurring_event_id,
        raw,
    }
}

fn event_datetime_to_iso(edt: &EventDateTime) -> Option<String> {
    if let Some(date) = &edt.date {
        Some(date.and_hms_opt(0, 0, 0)?.and_utc().to_rfc3339())
    } else {
        edt.date_time.as_ref().map(|dt| dt.to_rfc3339())
    }
}

fn convert_google_status(status: Option<GoogleEventStatus>) -> EventStatus {
    match status {
        Some(GoogleEventStatus::Tentative) => EventStatus::Tentative,
        Some(GoogleEventStatus::Cancelled) => EventStatus::Cancelled,
        _ => EventStatus::Confirmed,
    }
}

fn convert_google_attendee(attendee: &GoogleAttendee) -> EventAttendee {
    let is_organizer = attendee.organizer.unwrap_or(false);
    let is_optional = attendee.optional.unwrap_or(false);

    EventAttendee {
        name: attendee.display_name.clone(),
        email: attendee.email.clone(),
        is_current_user: attendee.is_self.unwrap_or(false),
        status: convert_google_attendee_status(&attendee.response_status),
        role: if is_organizer {
            AttendeeRole::Chair
        } else if is_optional {
            AttendeeRole::Optional
        } else {
            AttendeeRole::Required
        },
    }
}

fn convert_google_attendee_status(status: &Option<AttendeeResponseStatus>) -> AttendeeStatus {
    match status {
        Some(AttendeeResponseStatus::Accepted) => AttendeeStatus::Accepted,
        Some(AttendeeResponseStatus::Tentative) => AttendeeStatus::Tentative,
        Some(AttendeeResponseStatus::Declined) => AttendeeStatus::Declined,
        _ => AttendeeStatus::Pending,
    }
}

fn extract_video_entry_point(event: &GoogleEvent) -> Option<String> {
    event
        .conference_data
        .as_ref()?
        .entry_points
        .as_ref()?
        .iter()
        .find(|ep| {
            matches!(
                ep.entry_point_type,
                hypr_google_calendar::EntryPointType::Video
            )
        })
        .map(|ep| ep.uri.clone())
}

fn convert_outlook_status(is_cancelled: Option<bool>, show_as: Option<EventShowAs>) -> EventStatus {
    if is_cancelled.unwrap_or(false) {
        EventStatus::Cancelled
    } else if matches!(show_as, Some(EventShowAs::Tentative)) {
        EventStatus::Tentative
    } else {
        EventStatus::Confirmed
    }
}

fn convert_outlook_attendee(attendee: &OutlookAttendee) -> EventAttendee {
    EventAttendee {
        name: attendee
            .email_address
            .as_ref()
            .and_then(|email| email.name.clone()),
        email: attendee
            .email_address
            .as_ref()
            .and_then(|email| email.address.clone()),
        is_current_user: false,
        status: convert_outlook_attendee_status(attendee),
        role: convert_outlook_attendee_role(attendee.type_.as_ref()),
    }
}

fn convert_outlook_attendee_status(attendee: &OutlookAttendee) -> AttendeeStatus {
    match attendee
        .status
        .as_ref()
        .and_then(|status| status.response.as_ref())
    {
        Some(OutlookResponseType::Accepted) | Some(OutlookResponseType::Organizer) => {
            AttendeeStatus::Accepted
        }
        Some(OutlookResponseType::TentativelyAccepted) => AttendeeStatus::Tentative,
        Some(OutlookResponseType::Declined) => AttendeeStatus::Declined,
        _ => AttendeeStatus::Pending,
    }
}

fn convert_outlook_attendee_role(role: Option<&AttendeeType>) -> AttendeeRole {
    match role {
        Some(AttendeeType::Optional) => AttendeeRole::Optional,
        Some(AttendeeType::Resource) => AttendeeRole::NonParticipant,
        _ => AttendeeRole::Required,
    }
}

fn convert_apple_status(status: AppleEventStatus) -> EventStatus {
    match status {
        AppleEventStatus::None | AppleEventStatus::Confirmed => EventStatus::Confirmed,
        AppleEventStatus::Tentative => EventStatus::Tentative,
        AppleEventStatus::Canceled => EventStatus::Cancelled,
    }
}

fn convert_person(participant: &Participant) -> EventPerson {
    EventPerson {
        name: participant.name.clone(),
        email: participant.email.clone(),
        is_current_user: participant.is_current_user,
    }
}

fn convert_apple_attendee(participant: &Participant) -> EventAttendee {
    EventAttendee {
        name: participant.name.clone(),
        email: participant.email.clone(),
        is_current_user: participant.is_current_user,
        status: convert_apple_attendee_status(&participant.status),
        role: convert_apple_attendee_role(&participant.role),
    }
}

fn convert_apple_attendee_status(status: &ParticipantStatus) -> AttendeeStatus {
    match status {
        ParticipantStatus::Unknown | ParticipantStatus::Pending => AttendeeStatus::Pending,
        ParticipantStatus::Accepted
        | ParticipantStatus::Delegated
        | ParticipantStatus::Completed
        | ParticipantStatus::InProgress => AttendeeStatus::Accepted,
        ParticipantStatus::Tentative => AttendeeStatus::Tentative,
        ParticipantStatus::Declined => AttendeeStatus::Declined,
    }
}

fn convert_apple_attendee_role(role: &ParticipantRole) -> AttendeeRole {
    match role {
        ParticipantRole::Unknown | ParticipantRole::Required => AttendeeRole::Required,
        ParticipantRole::Optional => AttendeeRole::Optional,
        ParticipantRole::Chair => AttendeeRole::Chair,
        ParticipantRole::NonParticipant => AttendeeRole::NonParticipant,
    }
}

fn local_date_string(date: &chrono::DateTime<chrono::Utc>, event_tz: Option<&str>) -> String {
    if let Some(tz_name) = event_tz
        && let Ok(tz) = tz_name.parse::<chrono_tz::Tz>()
    {
        return date.with_timezone(&tz).format("%Y-%m-%d").to_string();
    }

    date.with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}
