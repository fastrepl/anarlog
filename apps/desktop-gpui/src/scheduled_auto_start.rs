//! `stt/scheduled-auto-start.tsx`: which calendar meetings are due to
//! auto-start, and what the live status lets the scheduler do. The workspace
//! runs the ticks; this is the part `scheduled-auto-start.test.ts` covers.

use std::collections::HashSet;

use chrono::{DateTime, TimeZone as _, Utc};

/// `SCHEDULED_AUTO_START_GRACE_MS`: a meeting that started while the app was
/// asleep or quit is still worth recording, but only briefly.
pub const GRACE_MS: i64 = 5 * 60_000;
/// `TICK_MS`: the retry cadence while a due meeting is blocked.
pub const TICK_MS: u64 = 15_000;

/// `SCHEDULED_MEETINGS_SQL`: calendar blocks without a meeting link are
/// excluded, since auto-start watches every calendar.
pub const SCHEDULED_MEETINGS_SQL: &str = "
  SELECT
    id,
    started_at,
    meeting_link,
    tracking_id_event,
    recurrence_series_id
  FROM events
  WHERE deleted_at IS NULL
    AND is_all_day = 0
    AND started_at <> ''
    AND meeting_link <> ''
  ORDER BY started_at, id
";

/// `ScheduledMeetingRow`
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ScheduledMeeting {
    pub id: String,
    pub started_at: String,
    pub meeting_link: String,
    pub tracking_id_event: String,
    pub recurrence_series_id: String,
}

/// `parseEventInstant`: a naive date-time is UTC wall-clock (leftover Graph
/// strings); anything else parses like `new Date(string)`.
pub fn parse_event_instant(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if is_naive_date_time(trimmed) {
        let normalized = trimmed.replacen(' ', "T", 1);
        return DateTime::parse_from_rfc3339(&format!("{normalized}Z"))
            .ok()
            .map(|parsed| parsed.with_timezone(&Utc))
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M")
                    .ok()
                    .map(|naive| Utc.from_utc_datetime(&naive))
            });
    }
    crate::timeline::parse_date(trimmed, &chrono::Local)
}

/// `NAIVE_DATE_TIME`: `YYYY-MM-DD[T ]HH:MM(:SS(.fff)?)?` with no offset.
fn is_naive_date_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    let digits = |range: std::ops::Range<usize>| {
        bytes
            .get(range)
            .is_some_and(|slice| slice.iter().all(u8::is_ascii_digit))
    };
    if !(digits(0..4)
        && bytes.get(4) == Some(&b'-')
        && digits(5..7)
        && bytes.get(7) == Some(&b'-')
        && digits(8..10)
        && matches!(bytes.get(10), Some(b'T') | Some(b' '))
        && digits(11..13)
        && bytes.get(13) == Some(&b':')
        && digits(14..16))
    {
        return false;
    }
    match &bytes[16..] {
        [] => true,
        [b':', rest @ ..] => {
            let (seconds, fraction) = rest.split_at(rest.len().min(2));
            seconds.len() == 2
                && seconds.iter().all(u8::is_ascii_digit)
                && match fraction {
                    [] => true,
                    [b'.', frac @ ..] => !frac.is_empty() && frac.iter().all(u8::is_ascii_digit),
                    _ => false,
                }
        }
        _ => false,
    }
}

/// `selectDueMeetings`: the meetings that started within the grace window
/// and have not fired, newest start first (back-to-back meetings overlap
/// inside the window; the one that just started is the one the user is
/// walking into).
pub fn select_due_meetings<'a>(
    rows: &'a [ScheduledMeeting],
    now_ms: i64,
    fired: &HashSet<String>,
) -> Vec<&'a ScheduledMeeting> {
    let mut due: Vec<(&ScheduledMeeting, i64)> = rows
        .iter()
        .filter(|row| !fired.contains(&row.id))
        .filter_map(|row| {
            let start_ms = parse_event_instant(&row.started_at)?.timestamp_millis();
            let elapsed = now_ms - start_ms;
            (0..=GRACE_MS).contains(&elapsed).then_some((row, start_ms))
        })
        .collect();
    due.sort_by(|a, b| b.1.cmp(&a.1));
    due.into_iter().map(|(row, _)| row).collect()
}

/// `scheduleNextStart`: how long until the earliest unfired meeting still
/// ahead, if any.
pub fn next_start_delay_ms(
    rows: &[ScheduledMeeting],
    now_ms: i64,
    fired: &HashSet<String>,
) -> Option<i64> {
    rows.iter()
        .filter(|row| !fired.contains(&row.id))
        .filter_map(|row| parse_event_instant(&row.started_at))
        .map(|start| start.timestamp_millis())
        .filter(|start| *start > now_ms)
        .min()
        .map(|start| start - now_ms)
}

/// `LiveSessionStatus` as far as the scheduler cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveStatus {
    Inactive,
    Active,
    Finalizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Retry,
    Skip,
}

/// `getScheduledAutoStartAction`
pub fn action(status: LiveStatus) -> Action {
    match status {
        LiveStatus::Active => Action::Skip,
        LiveStatus::Finalizing => Action::Retry,
        LiveStatus::Inactive => Action::Start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_778_846_400_000; // 2026-05-15T12:00:00.000Z

    fn meeting(id: &str, offset_ms: i64) -> ScheduledMeeting {
        ScheduledMeeting {
            id: id.into(),
            started_at: DateTime::<Utc>::from_timestamp_millis(NOW + offset_ms)
                .unwrap()
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            meeting_link: format!("https://zoom.us/j/{id}"),
            tracking_id_event: format!("tracking-{id}"),
            recurrence_series_id: String::new(),
        }
    }

    fn select(rows: &[ScheduledMeeting], fired: &[&str]) -> Vec<String> {
        let fired: HashSet<String> = fired.iter().map(|id| id.to_string()).collect();
        select_due_meetings(rows, NOW, &fired)
            .into_iter()
            .map(|row| row.id.clone())
            .collect()
    }

    #[test]
    fn selects_meetings_inside_the_grace_window_newest_first() {
        assert_eq!(select(&[meeting("a", 0)], &[]), ["a"]);
        assert!(select(&[meeting("a", 30_000)], &[]).is_empty());
        assert_eq!(select(&[meeting("a", -GRACE_MS + 1)], &[]), ["a"]);
        assert!(select(&[meeting("a", -GRACE_MS - 1)], &[]).is_empty());
        assert!(select(&[meeting("a", 0)], &["a"]).is_empty());
        let rows = [
            meeting("earlier", -4 * 60_000),
            meeting("latest", -30_000),
            meeting("middle", -2 * 60_000),
        ];
        assert_eq!(select(&rows, &[]), ["latest", "middle", "earlier"]);
        let rows = [meeting("earlier", -60_000), meeting("latest", -30_000)];
        assert_eq!(select(&rows, &["latest"]), ["earlier"]);
        let mut broken = meeting("broken", 0);
        broken.started_at = "not-a-date".into();
        assert_eq!(select(&[broken, meeting("good", -60_000)], &[]), ["good"]);
    }

    #[test]
    fn next_start_skips_fired_and_past_meetings() {
        let rows = [
            meeting("past", -60_000),
            meeting("soon", 45_000),
            meeting("later", 3_600_000),
        ];
        assert_eq!(
            next_start_delay_ms(&rows, NOW, &HashSet::new()),
            Some(45_000)
        );
        let fired: HashSet<String> = ["soon".to_string()].into_iter().collect();
        assert_eq!(next_start_delay_ms(&rows, NOW, &fired), Some(3_600_000));
        assert_eq!(next_start_delay_ms(&rows[..1], NOW, &HashSet::new()), None);
    }

    #[test]
    fn event_instants_treat_naive_strings_as_utc() {
        assert_eq!(
            parse_event_instant("2026-05-15T12:00:00")
                .unwrap()
                .timestamp_millis(),
            NOW
        );
        assert_eq!(
            parse_event_instant("2026-05-15 12:00")
                .unwrap()
                .timestamp_millis(),
            NOW
        );
        assert_eq!(
            parse_event_instant("2026-05-15T12:00:00.000Z")
                .unwrap()
                .timestamp_millis(),
            NOW
        );
        assert_eq!(
            parse_event_instant("2026-05-15T14:00:00+02:00")
                .unwrap()
                .timestamp_millis(),
            NOW
        );
        assert!(parse_event_instant("not-a-date").is_none());
        assert!(!is_naive_date_time("2026-05-15T12:00:00Z"));
        assert!(is_naive_date_time("2026-05-15T12:00:00.5"));
        assert!(!is_naive_date_time("2026-05-15T12:00:00."));
    }

    #[test]
    fn live_status_maps_to_the_scheduler_action() {
        assert_eq!(action(LiveStatus::Active), Action::Skip);
        assert_eq!(action(LiveStatus::Finalizing), Action::Retry);
        assert_eq!(action(LiveStatus::Inactive), Action::Start);
    }
}
