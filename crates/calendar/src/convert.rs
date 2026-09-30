use anlg_apple_calendar::types::{
    AppleCalendar, AppleEvent, EventStatus as AppleEventStatus, Participant, ParticipantRole,
    ParticipantStatus, ParticipantType,
};
use anlg_calendar_interface::{
    AttendanceResponseCounts, AttendanceRosterStatus, AttendeeRole, AttendeeStatus, CalendarEvent,
    CalendarListItem, CalendarProviderType, EventAttendance, EventAttendee, EventPerson,
    EventStatus, SelfAttendanceStatus,
};
use anlg_google_calendar::{
    AccessRole as GoogleAccessRole, Attendee as GoogleAttendee, AttendeeResponseStatus,
    CalendarListEntry as GoogleCalendar, Event as GoogleEvent, EventDateTime,
    EventStatus as GoogleEventStatus,
};
use anlg_outlook_calendar::{
    Attendee as OutlookAttendee, AttendeeType, Calendar as OutlookCalendar,
    DateTimeTimeZone as OutlookDateTimeTimeZone, Event as OutlookEvent, EventShowAs,
    ResponseType as OutlookResponseType,
};
use chrono::{DateTime, MappedLocalTime, NaiveDateTime, Utc};
use std::collections::HashSet;

use crate::windows_tz::windows_tz_to_iana;

pub fn convert_google_calendars(calendars: Vec<GoogleCalendar>) -> Vec<CalendarListItem> {
    calendars
        .into_iter()
        .map(|calendar| {
            let can_edit = calendar
                .access_role
                .as_ref()
                .map(|role| matches!(role, GoogleAccessRole::Writer | GoogleAccessRole::Owner));
            let raw = serde_json::to_string(&calendar).unwrap_or_default();
            // for google calendars, data_owner is only set for secondary calendars
            // calendar.id is the email for primary calendars
            let source = if calendar.primary == Some(true) {
                Some(calendar.id.clone())
            } else {
                calendar.data_owner
            };

            CalendarListItem {
                provider: CalendarProviderType::Google,
                id: calendar.id,
                title: calendar
                    .summary_override
                    .or(calendar.summary)
                    .unwrap_or_else(|| "Untitled".to_string()),
                source,
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

fn apple_color_to_css(color: anlg_apple_calendar::types::CalendarColor) -> String {
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
    let mut occurrences = std::collections::BTreeMap::new();
    for event in events {
        let rank = (
            event.is_detached,
            event.last_modified_date,
            matches!(event.status, AppleEventStatus::Canceled),
            event.event_identifier.clone(),
        );
        let mut converted = convert_apple_event(event);
        match occurrences.entry(converted.id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert((rank, converted));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let (current_rank, current) = entry.get_mut();
                let mut aliases = current.legacy_ids.clone();
                aliases.append(&mut converted.legacy_ids);
                aliases.sort();
                aliases.dedup();
                if rank > *current_rank {
                    *current_rank = rank;
                    *current = converted;
                }
                current.legacy_ids = aliases;
            }
        }
    }
    occurrences.into_values().map(|(_, event)| event).collect()
}

fn convert_google_event(event: GoogleEvent, calendar_id: &str) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();
    let attendance = Some(convert_google_attendance(&event));

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

    let meeting_link = resolve_meeting_link(
        event
            .hangout_link
            .clone()
            .or_else(|| extract_video_entry_point(&event)),
        event.location.as_deref(),
        event.description.as_deref(),
    );

    let has_recurrence_rules = event.recurring_event_id.is_some()
        || event.recurrence.as_ref().is_some_and(|r| !r.is_empty());

    CalendarEvent {
        id: event.id,
        legacy_ids: Vec::new(),
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
        attendance,
        has_recurrence_rules,
        recurring_event_id: event.recurring_event_id,
        raw,
    }
}

fn convert_outlook_event(event: OutlookEvent, calendar_id: &str) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();
    let attendance = Some(convert_outlook_attendance(&event));
    let is_all_day = event.is_all_day.unwrap_or(false);

    let started_at = event
        .start
        .as_ref()
        .map(|start| outlook_datetime_to_iso(start, is_all_day))
        .unwrap_or_default();
    let ended_at = event
        .end
        .as_ref()
        .map(|end| outlook_datetime_to_iso(end, is_all_day))
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

    let description = event.body.and_then(|body| body.content);
    let location = event.location.and_then(|location| location.display_name);
    let meeting_link = resolve_meeting_link(
        event.online_meeting_url.clone().or_else(|| {
            event
                .online_meeting
                .as_ref()
                .and_then(|meeting| meeting.join_url.clone())
        }),
        location.as_deref(),
        description.as_deref(),
    );

    CalendarEvent {
        id: event.id,
        legacy_ids: Vec::new(),
        calendar_id: calendar_id.to_string(),
        provider: CalendarProviderType::Outlook,
        external_id: event.ical_uid.unwrap_or_default(),
        title: event.subject.unwrap_or_default(),
        description,
        location,
        url: event.web_link,
        meeting_link,
        started_at,
        ended_at,
        timezone,
        is_all_day,
        status: convert_outlook_status(event.is_cancelled, event.show_as),
        organizer,
        attendees,
        attendance,
        has_recurrence_rules: event.recurrence.is_some() || event.series_master_id.is_some(),
        recurring_event_id: event.series_master_id,
        raw,
    }
}

fn convert_apple_event(event: AppleEvent) -> CalendarEvent {
    let raw = serde_json::to_string(&event).unwrap_or_default();
    let attendance = Some(convert_apple_attendance(&event));

    let (id, legacy_ids) = crate::apple_identity::identity(&event);

    let organizer = event.organizer.as_ref().map(convert_person);
    let attendees = event.attendees.iter().map(convert_apple_attendee).collect();

    let recurring_event_id = if event.has_recurrence_rules || event.is_detached {
        event
            .recurrence
            .as_ref()
            .map(|recurrence| {
                crate::apple_identity::without_occurrence_suffix(
                    &recurrence.series_identifier,
                    &event,
                )
                .to_string()
            })
            .or_else(|| {
                [&event.external_identifier, &event.calendar_item_identifier]
                    .into_iter()
                    .find(|identifier| !identifier.is_empty())
                    .map(|identifier| {
                        crate::apple_identity::without_occurrence_suffix(identifier, &event)
                            .to_string()
                    })
            })
    } else {
        None
    };

    let meeting_link =
        resolve_meeting_link(None, event.location.as_deref(), event.notes.as_deref());

    CalendarEvent {
        id,
        legacy_ids,
        calendar_id: event.calendar.id,
        provider: CalendarProviderType::Apple,
        external_id: event.external_identifier,
        title: event.title,
        description: event.notes,
        location: event.location,
        url: event.url,
        meeting_link,
        started_at: event.start_date.to_rfc3339(),
        ended_at: event.end_date.to_rfc3339(),
        timezone: event.time_zone,
        is_all_day: event.is_all_day,
        status: convert_apple_status(event.status),
        organizer,
        attendees,
        attendance,
        has_recurrence_rules: event.has_recurrence_rules,
        recurring_event_id,
        raw,
    }
}

#[derive(Clone, Copy)]
enum AttendanceResponse {
    Accepted,
    Tentative,
    Pending,
    Declined,
    Unknown,
}

impl AttendanceResponse {
    fn add_to(self, counts: &mut AttendanceResponseCounts) {
        match self {
            Self::Accepted => counts.accepted += 1,
            Self::Tentative => counts.tentative += 1,
            Self::Pending => counts.pending += 1,
            Self::Declined => counts.declined += 1,
            Self::Unknown => counts.unknown += 1,
        }
    }
}

#[derive(Default)]
struct SeenAttendees {
    ids: HashSet<String>,
    emails: HashSet<String>,
}

impl SeenAttendees {
    fn insert(&mut self, id: Option<&str>, email: Option<&str>) -> bool {
        let id = normalized_identity(id);
        let email = normalized_identity(email);
        if id.as_ref().is_some_and(|id| self.ids.contains(id))
            || email
                .as_ref()
                .is_some_and(|email| self.emails.contains(email))
        {
            return false;
        }

        if let Some(id) = id {
            self.ids.insert(id);
        }
        if let Some(email) = email {
            self.emails.insert(email);
        }
        true
    }
}

fn normalized_identity(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

fn convert_google_attendance(event: &GoogleEvent) -> EventAttendance {
    let self_status = if event
        .organizer
        .as_ref()
        .is_some_and(|organizer| organizer.is_self == Some(true))
    {
        SelfAttendanceStatus::Organizer
    } else if let Some(attendee) = event
        .attendees
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|attendee| attendee.is_self == Some(true))
    {
        match attendee.response_status.as_ref() {
            Some(AttendeeResponseStatus::Accepted) => SelfAttendanceStatus::Accepted,
            Some(AttendeeResponseStatus::Tentative) => SelfAttendanceStatus::Tentative,
            Some(AttendeeResponseStatus::Declined) => SelfAttendanceStatus::Declined,
            Some(AttendeeResponseStatus::Unknown) => SelfAttendanceStatus::Unknown,
            Some(AttendeeResponseStatus::NeedsAction) | None => SelfAttendanceStatus::Pending,
        }
    } else {
        SelfAttendanceStatus::Unknown
    };

    let roster_status = if event.attendees_omitted == Some(true) {
        AttendanceRosterStatus::Incomplete
    } else if event.attendees.is_some() || self_status == SelfAttendanceStatus::Organizer {
        AttendanceRosterStatus::Complete
    } else {
        AttendanceRosterStatus::Unknown
    };

    let mut others = AttendanceResponseCounts::default();
    let mut seen = SeenAttendees::default();
    let has_external_organizer = event
        .organizer
        .as_ref()
        .is_some_and(|organizer| organizer.is_self != Some(true));
    if let Some(organizer) = event.organizer.as_ref().filter(|_| has_external_organizer) {
        seen.insert(organizer.id.as_deref(), organizer.email.as_deref());
        AttendanceResponse::Accepted.add_to(&mut others);
    }

    for attendee in event.attendees.as_deref().unwrap_or_default() {
        if attendee.is_self == Some(true) || attendee.resource == Some(true) {
            continue;
        }
        if attendee.organizer == Some(true) && has_external_organizer {
            continue;
        }
        if !seen.insert(attendee.id.as_deref(), attendee.email.as_deref()) {
            continue;
        }
        if attendee.organizer == Some(true) {
            AttendanceResponse::Accepted.add_to(&mut others);
        } else {
            google_attendance_response(attendee.response_status.as_ref()).add_to(&mut others);
        }
    }

    EventAttendance {
        self_status,
        roster_status,
        others,
    }
}

fn google_attendance_response(status: Option<&AttendeeResponseStatus>) -> AttendanceResponse {
    match status {
        Some(AttendeeResponseStatus::Accepted) => AttendanceResponse::Accepted,
        Some(AttendeeResponseStatus::Tentative) => AttendanceResponse::Tentative,
        Some(AttendeeResponseStatus::Declined) => AttendanceResponse::Declined,
        Some(AttendeeResponseStatus::Unknown) => AttendanceResponse::Unknown,
        Some(AttendeeResponseStatus::NeedsAction) | None => AttendanceResponse::Pending,
    }
}

fn convert_outlook_attendance(event: &OutlookEvent) -> EventAttendance {
    let self_status = if event.is_organizer == Some(true)
        || matches!(
            event
                .response_status
                .as_ref()
                .and_then(|status| status.response.as_ref()),
            Some(OutlookResponseType::Organizer)
        ) {
        SelfAttendanceStatus::Organizer
    } else {
        match event
            .response_status
            .as_ref()
            .and_then(|status| status.response.as_ref())
        {
            Some(OutlookResponseType::Accepted) => SelfAttendanceStatus::Accepted,
            Some(OutlookResponseType::TentativelyAccepted) => SelfAttendanceStatus::Tentative,
            Some(OutlookResponseType::Declined) => SelfAttendanceStatus::Declined,
            Some(OutlookResponseType::None | OutlookResponseType::NotResponded) => {
                SelfAttendanceStatus::Pending
            }
            Some(OutlookResponseType::Unknown) | None => SelfAttendanceStatus::Unknown,
            Some(OutlookResponseType::Organizer) => SelfAttendanceStatus::Organizer,
        }
    };

    let roster_status = if (event.hide_attendees == Some(true)
        && !event.is_organizer.unwrap_or(false))
        || event
            .attendees
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|attendee| matches!(attendee.type_, Some(AttendeeType::Unknown)))
    {
        AttendanceRosterStatus::Incomplete
    } else if event.attendees.is_some() || self_status == SelfAttendanceStatus::Organizer {
        AttendanceRosterStatus::Complete
    } else {
        AttendanceRosterStatus::Unknown
    };

    let mut others = AttendanceResponseCounts::default();
    let mut seen = SeenAttendees::default();
    let has_external_organizer = event.is_organizer != Some(true) && event.organizer.is_some();
    if let Some(organizer) = event.organizer.as_ref().filter(|_| has_external_organizer) {
        let email = organizer
            .email_address
            .as_ref()
            .and_then(|email| email.address.as_deref());
        seen.insert(None, email);
        AttendanceResponse::Accepted.add_to(&mut others);
    }

    for attendee in event.attendees.as_deref().unwrap_or_default() {
        if matches!(attendee.type_, Some(AttendeeType::Resource)) {
            continue;
        }
        let email = attendee
            .email_address
            .as_ref()
            .and_then(|email| email.address.as_deref());
        if !seen.insert(None, email) {
            continue;
        }
        outlook_attendance_response(
            attendee
                .status
                .as_ref()
                .and_then(|status| status.response.as_ref()),
        )
        .add_to(&mut others);
    }

    EventAttendance {
        self_status,
        roster_status,
        others,
    }
}

fn outlook_attendance_response(status: Option<&OutlookResponseType>) -> AttendanceResponse {
    match status {
        Some(OutlookResponseType::Accepted | OutlookResponseType::Organizer) => {
            AttendanceResponse::Accepted
        }
        Some(OutlookResponseType::TentativelyAccepted) => AttendanceResponse::Tentative,
        Some(OutlookResponseType::Declined) => AttendanceResponse::Declined,
        Some(OutlookResponseType::None | OutlookResponseType::NotResponded) => {
            AttendanceResponse::Pending
        }
        Some(OutlookResponseType::Unknown) | None => AttendanceResponse::Unknown,
    }
}

fn convert_apple_attendance(event: &AppleEvent) -> EventAttendance {
    let self_status = if event
        .organizer
        .as_ref()
        .is_some_and(|organizer| organizer.is_current_user)
    {
        SelfAttendanceStatus::Organizer
    } else if let Some(attendee) = event
        .attendees
        .iter()
        .find(|attendee| attendee.is_current_user)
    {
        apple_self_attendance_status(&attendee.status)
    } else {
        SelfAttendanceStatus::Unknown
    };

    let roster_status = if event.attendees.iter().any(|attendee| {
        matches!(
            attendee.participant_type,
            ParticipantType::Group | ParticipantType::Unknown
        )
    }) || (event.has_attendees && event.attendees.is_empty())
    {
        AttendanceRosterStatus::Incomplete
    } else {
        AttendanceRosterStatus::Complete
    };

    let mut others = AttendanceResponseCounts::default();
    let mut seen = SeenAttendees::default();
    let has_external_organizer = event
        .organizer
        .as_ref()
        .is_some_and(|organizer| !organizer.is_current_user);
    if let Some(organizer) = event.organizer.as_ref().filter(|_| has_external_organizer) {
        insert_apple_identity(&mut seen, organizer);
        AttendanceResponse::Accepted.add_to(&mut others);
    }

    for attendee in &event.attendees {
        if attendee.is_current_user
            || matches!(
                attendee.participant_type,
                ParticipantType::Room | ParticipantType::Resource
            )
            || matches!(attendee.role, ParticipantRole::NonParticipant)
        {
            continue;
        }
        if has_external_organizer && matches!(attendee.role, ParticipantRole::Chair) {
            continue;
        }
        if !insert_apple_identity(&mut seen, attendee) {
            continue;
        }
        if matches!(attendee.role, ParticipantRole::Chair) {
            AttendanceResponse::Accepted.add_to(&mut others);
        } else {
            apple_attendance_response(&attendee.status).add_to(&mut others);
        }
    }

    EventAttendance {
        self_status,
        roster_status,
        others,
    }
}

fn insert_apple_identity(seen: &mut SeenAttendees, participant: &Participant) -> bool {
    let id = participant
        .contact
        .as_ref()
        .map(|contact| contact.identifier.as_str());
    seen.insert(id, participant.email.as_deref())
}

fn apple_self_attendance_status(status: &ParticipantStatus) -> SelfAttendanceStatus {
    match status {
        ParticipantStatus::Accepted
        | ParticipantStatus::Delegated
        | ParticipantStatus::Completed
        | ParticipantStatus::InProgress => SelfAttendanceStatus::Accepted,
        ParticipantStatus::Tentative => SelfAttendanceStatus::Tentative,
        ParticipantStatus::Declined => SelfAttendanceStatus::Declined,
        ParticipantStatus::Pending => SelfAttendanceStatus::Pending,
        ParticipantStatus::Unknown => SelfAttendanceStatus::Unknown,
    }
}

fn apple_attendance_response(status: &ParticipantStatus) -> AttendanceResponse {
    match status {
        ParticipantStatus::Accepted
        | ParticipantStatus::Delegated
        | ParticipantStatus::Completed
        | ParticipantStatus::InProgress => AttendanceResponse::Accepted,
        ParticipantStatus::Tentative => AttendanceResponse::Tentative,
        ParticipantStatus::Declined => AttendanceResponse::Declined,
        ParticipantStatus::Pending => AttendanceResponse::Pending,
        ParticipantStatus::Unknown => AttendanceResponse::Unknown,
    }
}

// Graph stores timed start/end as a timezone-naive `{date}T{time}` plus a
// separate timeZone field (Windows IDs or IANA). Persisting that string as-is
// makes JS `Date` treat UTC wall-clock values as local, so CEST notifications
// fire two hours early. All-day values are calendar dates (midnight on that
// date), not instants — converting those through UTC shifts the day west of UTC.
fn outlook_datetime_to_iso(value: &OutlookDateTimeTimeZone, is_all_day: bool) -> String {
    if is_all_day {
        return outlook_all_day_to_iso(value);
    }

    let raw = value.date_time.trim();
    if raw.is_empty() {
        return String::new();
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return dt.with_timezone(&Utc).to_rfc3339();
    }

    let Some(naive) = parse_outlook_naive(raw) else {
        return value.date_time.clone();
    };

    naive_in_timezone(naive, value.time_zone.as_deref()).to_rfc3339()
}

fn outlook_all_day_to_iso(value: &OutlookDateTimeTimeZone) -> String {
    let raw = value.date_time.trim();
    if let Some(date) = raw.get(..10).filter(|date| {
        date.as_bytes().get(4) == Some(&b'-')
            && date.as_bytes().get(7) == Some(&b'-')
            && date
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'-')
    }) {
        return format!("{date}T00:00:00");
    }

    value.date_time.clone()
}

fn parse_outlook_naive(value: &str) -> Option<NaiveDateTime> {
    const FORMATS: [&str; 4] = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%d %H:%M:%S",
    ];

    FORMATS
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
}

fn naive_in_timezone(naive: NaiveDateTime, time_zone: Option<&str>) -> DateTime<Utc> {
    let tz_name = time_zone.map(str::trim).filter(|name| !name.is_empty());
    let Some(tz_name) = tz_name else {
        return naive.and_utc();
    };

    if is_utc_timezone(tz_name) {
        return naive.and_utc();
    }

    let iana = windows_tz_to_iana(tz_name).unwrap_or(tz_name);
    let Ok(tz) = iana.parse::<chrono_tz::Tz>() else {
        return naive.and_utc();
    };

    match naive.and_local_timezone(tz) {
        MappedLocalTime::Single(dt) => dt.with_timezone(&Utc),
        MappedLocalTime::Ambiguous(earliest, _) => earliest.with_timezone(&Utc),
        MappedLocalTime::None => naive.and_utc(),
    }
}

fn is_utc_timezone(name: &str) -> bool {
    name.eq_ignore_ascii_case("UTC")
        || name.eq_ignore_ascii_case("GMT")
        || name.eq_ignore_ascii_case("Etc/UTC")
        || name.eq_ignore_ascii_case("Etc/GMT")
        || name
            .rsplit('/')
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case("UTC"))
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
        role: if attendee.resource.unwrap_or(false) {
            AttendeeRole::NonParticipant
        } else if is_organizer {
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
                anlg_google_calendar::EntryPointType::Video
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

pub(super) fn local_date_string(
    date: &chrono::DateTime<chrono::Utc>,
    event_tz: Option<&str>,
) -> String {
    if let Some(tz_name) = event_tz
        && let Ok(tz) = tz_name.parse::<chrono_tz::Tz>()
    {
        return date.with_timezone(&tz).format("%Y-%m-%d").to_string();
    }

    date.with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}

// Provider-native links win; otherwise fall back to a link parsed from the
// location, then from the description, so every event crosses the Tauri
// bridge with its final meeting link already resolved.
fn resolve_meeting_link(
    provider_link: Option<String>,
    location: Option<&str>,
    description: Option<&str>,
) -> Option<String> {
    provider_link
        .or_else(|| location.and_then(crate::parse_meeting_link))
        .or_else(|| description.and_then(crate::parse_meeting_link))
}

#[cfg(test)]
mod meeting_link_tests {
    use super::*;

    const MEET_LINK: &str = "https://meet.google.com/abc-defg-hij";
    const CAL_LINK: &str = "https://app.cal.com/video/abc123";

    #[test]
    fn resolve_meeting_link_precedence() {
        for (provider, location, description, expected) in [
            (
                Some("https://provider.example/join"),
                Some(MEET_LINK),
                Some(CAL_LINK),
                Some("https://provider.example/join"),
            ),
            (None, Some(MEET_LINK), Some(CAL_LINK), Some(MEET_LINK)),
            (
                None,
                Some("Conference room 4"),
                Some(CAL_LINK),
                Some(CAL_LINK),
            ),
            (None, Some("Conference room 4"), Some("Agenda"), None),
            (None, None, None, None),
        ] {
            assert_eq!(
                resolve_meeting_link(provider.map(str::to_string), location, description)
                    .as_deref(),
                expected
            );
        }
    }

    #[test]
    fn google_events_resolve_meeting_link() {
        let event_with_description_link: GoogleEvent = serde_json::from_value(serde_json::json!({
            "id": "evt-1",
            "summary": "Weekly sync",
            "location": "Conference room 4",
            "description": format!("Join here: {MEET_LINK}"),
        }))
        .unwrap();
        let event_with_provider_link: GoogleEvent = serde_json::from_value(serde_json::json!({
            "id": "evt-1",
            "hangoutLink": "https://meet.google.com/xyz-abcd-efg",
            "description": format!("Old link: {MEET_LINK}"),
        }))
        .unwrap();

        let converted = convert_google_events(
            vec![event_with_description_link, event_with_provider_link],
            "cal-1",
        );
        assert_eq!(converted[0].meeting_link.as_deref(), Some(MEET_LINK));
        assert_eq!(
            converted[1].meeting_link.as_deref(),
            Some("https://meet.google.com/xyz-abcd-efg")
        );
    }

    #[test]
    fn outlook_events_parse_links_from_the_body() {
        let event: OutlookEvent = serde_json::from_value(serde_json::json!({
            "id": "evt-2",
            "subject": "Design review",
            "body": { "content": format!("Agenda + {CAL_LINK}") },
        }))
        .unwrap();

        let converted = convert_outlook_events(vec![event], "cal-2");
        assert_eq!(converted[0].meeting_link.as_deref(), Some(CAL_LINK));
    }

    fn outlook_event_with_start(
        date_time: &str,
        time_zone: Option<&str>,
        is_all_day: bool,
    ) -> OutlookEvent {
        serde_json::from_value(serde_json::json!({
            "id": "evt-tz",
            "subject": "Timezone check",
            "isAllDay": is_all_day,
            "start": { "dateTime": date_time, "timeZone": time_zone },
            "end": { "dateTime": date_time, "timeZone": time_zone },
        }))
        .unwrap()
    }

    #[test]
    fn outlook_timed_events_normalize_to_utc() {
        for (date_time, tz, expected_started_at) in [
            (
                "2026-08-27T12:00:00.0000000",
                "UTC",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T14:00:00.0000000",
                "W. Europe Standard Time",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T14:00:00",
                "Europe/Paris",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T14:00:00+02:00",
                "UTC",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T15:00:00.0000000",
                "Turkey Standard Time",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T20:00:00.0000000",
                "Taipei Standard Time",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T20:00:00.0000000",
                "W. Australia Standard Time",
                "2026-08-27T12:00:00+00:00",
            ),
            (
                "2026-08-27T06:00:00.0000000",
                "Central Standard Time (Mexico)",
                "2026-08-27T12:00:00+00:00",
            ),
        ] {
            let converted = convert_outlook_events(
                vec![outlook_event_with_start(date_time, Some(tz), false)],
                "cal-2",
            );
            assert_eq!(converted[0].started_at, expected_started_at, "{tz}");
            if date_time == "2026-08-27T12:00:00.0000000" {
                assert_eq!(converted[0].ended_at, "2026-08-27T12:00:00+00:00");
            }
        }
    }

    #[test]
    fn outlook_all_day_keeps_the_calendar_date() {
        for tz in ["UTC", "Pacific Standard Time"] {
            let converted = convert_outlook_events(
                vec![outlook_event_with_start(
                    "2026-08-27T00:00:00.0000000",
                    Some(tz),
                    true,
                )],
                "cal-2",
            );

            assert_eq!(converted[0].started_at, "2026-08-27T00:00:00");
            assert!(converted[0].is_all_day);
        }
    }
}

#[cfg(test)]
mod attendance_tests {
    use super::*;

    fn attendance(event: &CalendarEvent) -> &EventAttendance {
        event.attendance.as_ref().unwrap()
    }

    fn google_event(value: serde_json::Value) -> CalendarEvent {
        let event = serde_json::from_value(value).unwrap();
        convert_google_event(event, "calendar")
    }

    fn outlook_event(value: serde_json::Value) -> CalendarEvent {
        let event = serde_json::from_value(value).unwrap();
        convert_outlook_event(event, "calendar")
    }

    fn apple_event(
        organizer: Option<serde_json::Value>,
        attendees: Vec<serde_json::Value>,
        has_attendees: bool,
    ) -> CalendarEvent {
        let event = serde_json::from_value(serde_json::json!({
            "event_identifier": "event",
            "calendar_item_identifier": "item",
            "external_identifier": "external",
            "calendar": { "id": "calendar", "title": "Calendar" },
            "title": "Meeting",
            "location": null,
            "url": null,
            "notes": null,
            "creation_date": null,
            "last_modified_date": null,
            "time_zone": "UTC",
            "start_date": "2026-09-30T10:00:00Z",
            "end_date": "2026-09-30T11:00:00Z",
            "is_all_day": false,
            "availability": "Busy",
            "status": "Confirmed",
            "has_alarms": false,
            "has_attendees": has_attendees,
            "has_notes": false,
            "has_recurrence_rules": false,
            "organizer": organizer,
            "attendees": attendees,
            "structured_location": null,
            "recurrence": null,
            "occurrence_date": null,
            "is_detached": false,
            "alarms": [],
            "birthday_contact_identifier": null,
            "is_birthday": false,
        }))
        .unwrap();
        convert_apple_event(event)
    }

    fn apple_participant(
        email: &str,
        is_current_user: bool,
        status: &str,
        participant_type: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "name": email,
            "email": email,
            "is_current_user": is_current_user,
            "role": "Required",
            "status": status,
            "participant_type": participant_type,
            "url": null,
            "contact": null,
        })
    }

    #[test]
    fn google_organizer_excludes_self_and_resources() {
        let event = google_event(serde_json::json!({
            "id": "event",
            "organizer": { "email": "me@example.com", "self": true },
            "attendees": [
                { "email": "me@example.com", "self": true, "organizer": true, "responseStatus": "accepted" },
                { "email": "declined@example.com", "responseStatus": "declined" },
                { "email": "room@example.com", "resource": true, "responseStatus": "accepted" }
            ]
        }));

        assert_eq!(
            attendance(&event).self_status,
            SelfAttendanceStatus::Organizer
        );
        assert_eq!(
            attendance(&event).roster_status,
            AttendanceRosterStatus::Complete
        );
        assert_eq!(attendance(&event).others.declined, 1);
        assert_eq!(attendance(&event).others.accepted, 0);
    }

    #[test]
    fn google_uses_self_response_and_deduplicates_external_organizer() {
        let event = google_event(serde_json::json!({
            "id": "event",
            "organizer": { "id": "organizer-id", "email": "HOST@example.com" },
            "attendees": [
                { "id": "organizer-id", "email": "host@example.com", "organizer": true, "responseStatus": "declined" },
                { "email": "me@example.com", "self": true, "responseStatus": "accepted" },
                { "email": "pending@example.com", "responseStatus": "needsAction" },
                { "email": "unknown@example.com", "responseStatus": "futureValue" }
            ]
        }));

        let attendance = attendance(&event);
        assert_eq!(attendance.self_status, SelfAttendanceStatus::Accepted);
        assert_eq!(attendance.others.accepted, 1);
        assert_eq!(attendance.others.pending, 1);
        assert_eq!(attendance.others.unknown, 1);
        assert_eq!(attendance.others.declined, 0);
    }

    #[test]
    fn google_marks_omitted_and_missing_rosters_conservatively() {
        let omitted = google_event(serde_json::json!({
            "id": "omitted",
            "attendeesOmitted": true,
            "attendees": [{ "self": true, "responseStatus": "tentative" }]
        }));
        let missing = google_event(serde_json::json!({ "id": "missing" }));

        assert_eq!(
            attendance(&omitted).self_status,
            SelfAttendanceStatus::Tentative
        );
        assert_eq!(
            attendance(&omitted).roster_status,
            AttendanceRosterStatus::Incomplete
        );
        assert_eq!(
            attendance(&missing).self_status,
            SelfAttendanceStatus::Unknown
        );
        assert_eq!(
            attendance(&missing).roster_status,
            AttendanceRosterStatus::Unknown
        );
    }

    #[test]
    fn outlook_uses_top_level_response_status() {
        for (response, expected) in [
            ("accepted", SelfAttendanceStatus::Accepted),
            ("tentativelyAccepted", SelfAttendanceStatus::Tentative),
            ("declined", SelfAttendanceStatus::Declined),
            ("notResponded", SelfAttendanceStatus::Pending),
            ("none", SelfAttendanceStatus::Pending),
            ("futureValue", SelfAttendanceStatus::Unknown),
        ] {
            let event = outlook_event(serde_json::json!({
                "id": response,
                "responseStatus": { "response": response },
                "attendees": []
            }));
            assert_eq!(attendance(&event).self_status, expected, "{response}");
        }
    }

    #[test]
    fn outlook_counts_external_organizer_once_and_excludes_resources() {
        let event = outlook_event(serde_json::json!({
            "id": "event",
            "responseStatus": { "response": "accepted" },
            "organizer": { "emailAddress": { "address": "HOST@example.com" } },
            "attendees": [
                { "emailAddress": { "address": "host@example.com" }, "status": { "response": "declined" } },
                { "emailAddress": { "address": "pending@example.com" }, "status": { "response": "notResponded" } },
                { "type": "resource", "emailAddress": { "address": "room@example.com" }, "status": { "response": "accepted" } }
            ]
        }));

        let attendance = attendance(&event);
        assert_eq!(attendance.others.accepted, 1);
        assert_eq!(attendance.others.pending, 1);
        assert_eq!(attendance.others.declined, 0);
    }

    #[test]
    fn outlook_organizer_and_hidden_roster_are_normalized() {
        let organizer = outlook_event(serde_json::json!({
            "id": "organizer",
            "isOrganizer": true
        }));
        let hidden = outlook_event(serde_json::json!({
            "id": "hidden",
            "hideAttendees": true,
            "responseStatus": { "response": "accepted" },
            "attendees": [{ "status": { "response": "declined" } }]
        }));
        let unknown_kind = outlook_event(serde_json::json!({
            "id": "unknown-kind",
            "responseStatus": { "response": "accepted" },
            "attendees": [{ "type": "futureValue", "status": { "response": "declined" } }]
        }));

        assert_eq!(
            attendance(&organizer).self_status,
            SelfAttendanceStatus::Organizer
        );
        assert_eq!(
            attendance(&organizer).roster_status,
            AttendanceRosterStatus::Complete
        );
        assert_eq!(
            attendance(&hidden).roster_status,
            AttendanceRosterStatus::Incomplete
        );
        assert_eq!(
            attendance(&unknown_kind).roster_status,
            AttendanceRosterStatus::Incomplete
        );
    }

    #[test]
    fn apple_maps_self_status_and_excludes_rooms_and_resources() {
        let event = apple_event(
            Some(apple_participant(
                "host@example.com",
                false,
                "Accepted",
                "Person",
            )),
            vec![
                apple_participant("HOST@example.com", false, "Declined", "Person"),
                apple_participant("me@example.com", true, "Delegated", "Person"),
                apple_participant("declined@example.com", false, "Declined", "Person"),
                apple_participant("room@example.com", false, "Accepted", "Room"),
                apple_participant("resource@example.com", false, "Accepted", "Resource"),
            ],
            true,
        );

        let attendance = attendance(&event);
        assert_eq!(attendance.self_status, SelfAttendanceStatus::Accepted);
        assert_eq!(attendance.others.accepted, 1);
        assert_eq!(attendance.others.declined, 1);
    }

    #[test]
    fn apple_organizer_and_group_or_missing_rosters_are_conservative() {
        let organizer = apple_participant("me@example.com", true, "Accepted", "Person");
        let organized = apple_event(Some(organizer), Vec::new(), false);
        let group = apple_event(
            None,
            vec![apple_participant(
                "group@example.com",
                false,
                "Pending",
                "Group",
            )],
            true,
        );
        let missing = apple_event(None, Vec::new(), true);
        let unknown_kind = apple_event(
            None,
            vec![apple_participant(
                "unknown@example.com",
                false,
                "Declined",
                "Unknown",
            )],
            true,
        );

        assert_eq!(
            attendance(&organized).self_status,
            SelfAttendanceStatus::Organizer
        );
        assert_eq!(
            attendance(&organized).roster_status,
            AttendanceRosterStatus::Complete
        );
        assert_eq!(
            attendance(&group).roster_status,
            AttendanceRosterStatus::Incomplete
        );
        assert_eq!(attendance(&group).others.pending, 1);
        assert_eq!(
            attendance(&missing).roster_status,
            AttendanceRosterStatus::Incomplete
        );
        assert_eq!(
            attendance(&unknown_kind).roster_status,
            AttendanceRosterStatus::Incomplete
        );
    }

    #[test]
    fn apple_distinguishes_unknown_from_pending_self_response() {
        let unknown = apple_event(
            None,
            vec![apple_participant(
                "me@example.com",
                true,
                "Unknown",
                "Person",
            )],
            true,
        );
        let pending = apple_event(
            None,
            vec![apple_participant(
                "me@example.com",
                true,
                "Pending",
                "Person",
            )],
            true,
        );

        assert_eq!(
            attendance(&unknown).self_status,
            SelfAttendanceStatus::Unknown
        );
        assert_eq!(
            attendance(&pending).self_status,
            SelfAttendanceStatus::Pending
        );
    }

    #[test]
    fn attendance_contract_serializes_with_stable_lowercase_values() {
        let value = serde_json::to_value(EventAttendance {
            self_status: SelfAttendanceStatus::Organizer,
            roster_status: AttendanceRosterStatus::Incomplete,
            others: AttendanceResponseCounts {
                accepted: 1,
                tentative: 2,
                pending: 3,
                declined: 4,
                unknown: 5,
            },
        })
        .unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "self_status": "organizer",
                "roster_status": "incomplete",
                "others": {
                    "accepted": 1,
                    "tentative": 2,
                    "pending": 3,
                    "declined": 4,
                    "unknown": 5,
                },
            })
        );
    }
}
