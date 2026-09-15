use anlg_apple_calendar::types::AppleEvent;
use chrono::SecondsFormat;

use crate::convert::local_date_string;

pub(super) fn identity(event: &AppleEvent) -> (String, Vec<String>) {
    let original = event.occurrence_date.as_ref();
    let recurring = event.has_recurrence_rules || event.is_detached;
    let legacy = if event.has_recurrence_rules {
        format!(
            "{}:{}",
            event.event_identifier,
            local_date_string(
                original.unwrap_or(&event.start_date),
                event.time_zone.as_deref()
            )
        )
    } else {
        event.event_identifier.clone()
    };

    // An unknown original occurrence must not collapse an entire recurring series.
    if recurring && original.is_none() {
        return (legacy, Vec::new());
    }
    let (namespace, uid) = if !event.external_identifier.is_empty() {
        ("uid", &event.external_identifier)
    } else if !event.calendar_item_identifier.is_empty() {
        ("local", &event.calendar_item_identifier)
    } else {
        return (legacy, Vec::new());
    };
    let occurrence = original.filter(|_| recurring).map(|date| {
        if event.is_all_day {
            local_date_string(date, event.time_zone.as_deref())
        } else {
            date.to_rfc3339_opts(SecondsFormat::Secs, true)
        }
    });
    let id = format!(
        "apple:v1:{}",
        serde_json::to_string(&(
            &event.calendar.id,
            namespace,
            without_occurrence_suffix(uid, event),
            occurrence
        ))
        .unwrap()
    );
    let mut aliases = vec![legacy];
    if let Some(original) = original.filter(|_| recurring) {
        let seconds = original.timestamp() - 978_307_200;
        let suffix = format!("/RID={seconds}");
        // EventKit encodes a detached occurrence using Apple-reference-date seconds.
        // Only strip a suffix that agrees with the explicit occurrenceDate field.
        let master = event
            .event_identifier
            .strip_suffix(&suffix)
            .unwrap_or(&event.event_identifier);
        if !event.is_detached || master != event.event_identifier {
            aliases.push(format!(
                "{}:{}",
                master,
                local_date_string(original, event.time_zone.as_deref())
            ));
            aliases.push(format!("{master}{suffix}"));
            aliases.push(format!(
                "{master}{suffix}:{}",
                local_date_string(original, event.time_zone.as_deref())
            ));
        }
    }
    aliases.sort();
    aliases.dedup();
    (id, aliases)
}

pub(super) fn without_occurrence_suffix<'a>(identifier: &'a str, event: &AppleEvent) -> &'a str {
    if (event.has_recurrence_rules || event.is_detached)
        && let Some(original) = event.occurrence_date
    {
        // EventKit also appends RID to calendarItemExternalIdentifier for exceptions.
        let suffix = format!("/RID={}", original.timestamp() - 978_307_200);
        return identifier.strip_suffix(&suffix).unwrap_or(identifier);
    }
    identifier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::convert_apple_events;

    fn event() -> AppleEvent {
        serde_json::from_value(serde_json::json!({
            "event_identifier": "store-uuid:meeting-uid",
            "calendar_item_identifier": "local-item-uuid",
            "external_identifier": "meeting-uid",
            "calendar": {"id": "calendar-uuid", "title": "Calendar"},
            "title": "Meeting",
            "time_zone": "UTC",
            "start_date": "2026-09-15T10:00:00Z",
            "end_date": "2026-09-15T11:00:00Z",
            "last_modified_date": "2026-09-01T00:00:00Z",
            "is_all_day": false,
            "availability": "Busy",
            "status": "Confirmed",
            "has_alarms": false,
            "has_attendees": false,
            "has_notes": false,
            "has_recurrence_rules": false,
            "attendees": [],
            "is_detached": false,
            "alarms": [],
            "is_birthday": false
        }))
        .unwrap()
    }

    fn recurring() -> AppleEvent {
        let mut event = event();
        event.has_recurrence_rules = true;
        event.occurrence_date = Some(event.start_date);
        event
    }

    fn detached() -> AppleEvent {
        let mut event = recurring();
        event.has_recurrence_rules = false;
        event.is_detached = true;
        event.event_identifier = format!(
            "{}/RID={}",
            event.event_identifier,
            event.occurrence_date.unwrap().timestamp() - 978_307_200
        );
        event.external_identifier = format!(
            "{}/RID={}",
            event.external_identifier,
            event.occurrence_date.unwrap().timestamp() - 978_307_200
        );
        event.start_date += chrono::Duration::days(2);
        event.end_date += chrono::Duration::days(2);
        event.title = "Renamed and rescheduled".into();
        event
    }

    #[test]
    fn single_event_identity_survives_edits_and_local_identifier_changes() {
        let original = event();
        let mut edited = original.clone();
        edited.title = "Another title".into();
        edited.start_date += chrono::Duration::days(7);
        edited.end_date += chrono::Duration::days(7);
        edited.event_identifier = "other-store:meeting-uid".into();
        edited.calendar_item_identifier = "other-local-item".into();
        assert_eq!(identity(&original).0, identity(&edited).0);
    }

    #[test]
    fn detached_occurrence_keeps_original_identity_and_exact_legacy_aliases() {
        let original = recurring();
        let moved = detached();
        assert_eq!(identity(&original).0, identity(&moved).0);
        let aliases = identity(&moved).1;
        assert!(aliases.contains(&"store-uuid:meeting-uid:2026-09-15".into()));
        assert!(aliases.contains(&moved.event_identifier));
        assert!(!aliases.contains(&"store-uuid:meeting-uid:2026-09-17".into()));
        let converted = convert_apple_events(vec![moved]);
        assert_eq!(
            converted[0].recurring_event_id.as_deref(),
            Some("meeting-uid")
        );
    }

    #[test]
    fn detached_override_wins_even_if_series_was_modified_more_recently() {
        let mut original = recurring();
        original.last_modified_date = Some(original.start_date);
        let moved = detached();
        for source in [
            vec![original.clone(), moved.clone()],
            vec![moved.clone(), original.clone()],
        ] {
            let converted = convert_apple_events(source);
            assert_eq!(converted.len(), 1);
            assert_eq!(converted[0].title, moved.title);
            assert!(converted[0].legacy_ids.contains(&identity(&original).1[0]));
        }
    }

    #[test]
    fn separate_uids_calendars_and_occurrences_remain_separate() {
        let original = recurring();
        let mut different_uid = original.clone();
        different_uid.external_identifier = "independent-meeting".into();
        let mut different_calendar = original.clone();
        different_calendar.calendar.id = "other-calendar".into();
        let mut same_day_occurrence = original.clone();
        same_day_occurrence.occurrence_date =
            Some(original.start_date + chrono::Duration::hours(1));
        assert_eq!(
            convert_apple_events(vec![
                original,
                different_uid,
                different_calendar,
                same_day_occurrence
            ])
            .len(),
            4
        );
        let one = event();
        let mut two = one.clone();
        two.external_identifier = "another-single-event".into();
        assert_eq!(convert_apple_events(vec![one, two]).len(), 2);
    }

    #[test]
    fn newest_duplicate_record_supplies_content_and_cancellation() {
        let original = recurring();
        let mut newer = original.clone();
        newer.event_identifier = "other-store:meeting-uid".into();
        newer.last_modified_date = Some(original.start_date);
        newer.status = anlg_apple_calendar::types::EventStatus::Canceled;
        for source in [
            vec![original.clone(), newer.clone()],
            vec![newer.clone(), original.clone()],
        ] {
            let converted = convert_apple_events(source);
            assert_eq!(converted.len(), 1);
            assert_eq!(
                converted[0].status,
                anlg_calendar_interface::EventStatus::Cancelled
            );
            assert!(
                converted[0]
                    .legacy_ids
                    .contains(&"store-uuid:meeting-uid:2026-09-15".into())
            );
            assert!(
                converted[0]
                    .legacy_ids
                    .contains(&"other-store:meeting-uid:2026-09-15".into())
            );
        }
    }

    #[test]
    fn unknown_original_occurrence_does_not_merge_series_instances() {
        let mut one = recurring();
        one.occurrence_date = None;
        let mut two = one.clone();
        two.start_date += chrono::Duration::days(1);
        assert_ne!(identity(&one).0, identity(&two).0);
    }

    #[test]
    fn missing_external_uid_uses_local_identity_without_title_matching() {
        let mut one = event();
        one.external_identifier.clear();
        let mut two = one.clone();
        two.calendar_item_identifier = "another-item".into();
        assert_ne!(identity(&one).0, identity(&two).0);
    }

    #[test]
    fn malformed_detached_rid_is_not_used_as_a_master_alias() {
        let mut moved = detached();
        moved.event_identifier = "store-uuid:meeting-uid/RID=1".into();
        assert_eq!(identity(&moved).1, vec![moved.event_identifier]);
    }

    #[test]
    fn all_day_identity_uses_original_calendar_date() {
        let mut one = recurring();
        one.is_all_day = true;
        one.time_zone = Some("America/Los_Angeles".into());
        one.occurrence_date = Some("2026-09-15T07:00:00Z".parse().unwrap());
        let mut two = one.clone();
        two.time_zone = Some("Asia/Seoul".into());
        two.occurrence_date = Some("2026-09-14T15:00:00Z".parse().unwrap());
        assert_eq!(identity(&one).0, identity(&two).0);
    }
}
