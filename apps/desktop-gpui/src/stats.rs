//! `settings/stats/{queries,activity}.ts`: the personal activity records and
//! `summarizeActivity`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc, Weekday};

/// `ActivityRecord`
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ActivityRecord {
    pub session_id: String,
    pub started_at_ms: i64,
    pub created_at: String,
    /// The welcome demo's session (`WELCOME_NOTE_TRACKING_ID`), which the
    /// badges leave out.
    #[sqlx(default)]
    pub is_demo: i64,
    pub duration_ms: i64,
}

/// `ACTIVITY_SQL`: one row per transcript with words, with the duration
/// taken from the largest word `end_ms`.
pub const ACTIVITY_SQL: &str = "
  SELECT
    transcript.session_id,
    transcript.started_at_ms,
    transcript.created_at,
    CASE WHEN json_valid(session.event_json)
      THEN COALESCE(json_extract(session.event_json, '$.tracking_id') = 'anarlog-onboarding-demo-v1', 0)
      ELSE 0 END AS is_demo,
    MAX(CASE
      WHEN word.type = 'object'
        AND json_type(word.value, '$.end_ms') IN ('integer', 'real')
      THEN MAX(0, json_extract(word.value, '$.end_ms'))
      ELSE 0
    END) AS duration_ms
  FROM transcripts AS transcript
  JOIN sessions AS session ON session.id = transcript.session_id
  JOIN json_each(CASE
    WHEN json_valid(transcript.words_json) THEN
      CASE WHEN json_type(transcript.words_json) = 'array'
        THEN transcript.words_json ELSE '[]' END
    ELSE '[]'
  END) AS word
  WHERE session.deleted_at IS NULL
    AND transcript.deleted_at IS NULL
    AND COALESCE(session.owner_user_id, '') IN (?, '', '00000000-0000-0000-0000-000000000000')
    AND CASE WHEN word.type = 'object'
      THEN LENGTH(TRIM(COALESCE(json_extract(word.value, '$.text'), ''))) > 0
      ELSE 0 END
  GROUP BY transcript.id
";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Range {
    All,
    Days30,
    Days7,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub conversations: usize,
    pub total_conversations: usize,
    pub hours: f64,
    pub active_days: usize,
    pub streak: usize,
    /// Every calendar day from the week containing a year ago to today.
    pub days: Vec<Day>,
    /// `weekdayCounts`: the week's days from `weekStartsOn`, each with the
    /// conversations that first started on it in the selected period.
    pub weekday_counts: Vec<(Weekday, usize)>,
    /// `medianMinutes`: the median transcribed length of the timed
    /// conversations, `None` without any.
    pub median_minutes: Option<f64>,
    pub timed_conversations: usize,
    /// `conversationDays`: the distinct days the period's conversations
    /// first started on.
    pub conversation_days: usize,
}

/// date-fns `startOfWeek`
fn start_of_week(date: NaiveDate, week_starts_on: Weekday) -> NaiveDate {
    let offset = (7 + date.weekday().num_days_from_sunday() as i64
        - week_starts_on.num_days_from_sunday() as i64)
        % 7;
    date - Duration::days(offset)
}

/// `summarizeActivity(records, now, timezone, weekStartsOn, range)`
pub fn summarize<Tz: TimeZone>(
    records: &[ActivityRecord],
    now: DateTime<Utc>,
    tz: &Tz,
    week_starts_on: Weekday,
    range: Range,
) -> Summary {
    let local_now = now.with_timezone(tz).date_naive();
    let cutoff = match range {
        Range::All => None,
        Range::Days7 => Some(local_now - Duration::days(6)),
        Range::Days30 => Some(local_now - Duration::days(29)),
    };
    let in_range = |day: NaiveDate| cutoff.is_none_or(|cutoff| day >= cutoff);

    let mut sessions: BTreeSet<&str> = BTreeSet::new();
    let mut all_sessions: BTreeSet<&str> = BTreeSet::new();
    let mut daily_sessions: BTreeMap<NaiveDate, BTreeSet<&str>> = BTreeMap::new();
    let mut weeks: BTreeSet<NaiveDate> = BTreeSet::new();
    let mut intervals: HashMap<&str, Vec<(i64, i64)>> = HashMap::new();
    // `sessionStarts`: each period conversation's earliest local start.
    let mut session_starts: HashMap<&str, DateTime<Tz>> = HashMap::new();

    for record in records {
        let started_at = if record.started_at_ms > 0 {
            Some(record.started_at_ms)
        } else {
            crate::timeline::parse_date(&record.created_at, tz).map(|d| d.timestamp_millis())
        };
        let Some(started_at) = started_at else {
            continue;
        };
        if started_at > now.timestamp_millis() {
            continue;
        }
        let Some(date) = Utc.timestamp_millis_opt(started_at).single() else {
            continue;
        };
        let local = date.with_timezone(tz);
        let day = local.date_naive();
        all_sessions.insert(&record.session_id);
        daily_sessions
            .entry(day)
            .or_default()
            .insert(&record.session_id);
        weeks.insert(start_of_week(day, week_starts_on));
        if !in_range(day) {
            continue;
        }
        sessions.insert(&record.session_id);
        let first = session_starts
            .entry(&record.session_id)
            .or_insert_with(|| local.clone());
        if local < *first {
            *first = local;
        }
        if record.duration_ms <= 0 {
            continue;
        }
        let end = (started_at + record.duration_ms).min(now.timestamp_millis());
        intervals
            .entry(&record.session_id)
            .or_default()
            .push((started_at, end));
    }

    let mut duration_ms: i64 = 0;
    let mut durations: Vec<i64> = Vec::new();
    for spans in intervals.values_mut() {
        spans.sort_by_key(|span| span.0);
        let mut previous_end = 0;
        let mut session_duration_ms = 0;
        for &(start, end) in spans.iter() {
            session_duration_ms += (end - start.max(previous_end)).max(0);
            previous_end = previous_end.max(end);
        }
        duration_ms += session_duration_ms;
        if session_duration_ms > 0 {
            durations.push(session_duration_ms);
        }
    }
    durations.sort_unstable();
    let middle = durations.len() / 2;
    let median_minutes = if durations.is_empty() {
        None
    } else if durations.len() % 2 == 1 {
        Some(durations[middle] as f64 / 60_000.0)
    } else {
        Some((durations[middle - 1] + durations[middle]) as f64 / 2.0 / 60_000.0)
    };
    let mut weekday_counts: Vec<(Weekday, usize)> = (0..7)
        .map(|index| {
            let mut weekday = week_starts_on;
            for _ in 0..index {
                weekday = weekday.succ();
            }
            (weekday, 0)
        })
        .collect();
    for start in session_starts.values() {
        let offset = (7 + start.weekday().num_days_from_sunday() as usize
            - week_starts_on.num_days_from_sunday() as usize)
            % 7;
        weekday_counts[offset].1 += 1;
    }
    let conversation_days: BTreeSet<NaiveDate> = session_starts
        .values()
        .map(|start| start.date_naive())
        .collect();

    let mut current_week = start_of_week(local_now, week_starts_on);
    if !weeks.contains(&current_week) {
        current_week -= Duration::days(7);
    }
    let mut streak = 0;
    while weeks.contains(&current_week) {
        streak += 1;
        current_week -= Duration::days(7);
    }

    let heatmap_start = start_of_week(local_now - Duration::days(364), week_starts_on);
    let mut days = Vec::new();
    let mut date = heatmap_start;
    while date <= local_now {
        days.push(Day {
            date,
            count: daily_sessions.get(&date).map_or(0, BTreeSet::len),
        });
        date += Duration::days(1);
    }

    Summary {
        conversations: sessions.len(),
        total_conversations: all_sessions.len(),
        hours: duration_ms as f64 / 3_600_000.0,
        active_days: daily_sessions.keys().filter(|day| in_range(**day)).count(),
        streak,
        days,
        weekday_counts,
        median_minutes,
        timed_conversations: durations.len(),
        conversation_days: conversation_days.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        "2026-09-05T12:00:00Z".parse().unwrap()
    }

    fn record(session: &str, date: &str) -> ActivityRecord {
        ActivityRecord {
            session_id: session.to_string(),
            started_at_ms: date
                .parse::<DateTime<Utc>>()
                .map(|d| d.timestamp_millis())
                .unwrap_or(0),
            created_at: date.to_string(),
            is_demo: 0,
            duration_ms: 3_600_000,
        }
    }

    fn timed(session: &str, date: &str, duration_ms: i64) -> ActivityRecord {
        ActivityRecord {
            duration_ms,
            ..record(session, date)
        }
    }

    fn day(summary: &Summary, key: &str) -> Option<usize> {
        let key: NaiveDate = key.parse().unwrap();
        summary
            .days
            .iter()
            .find(|day| day.date == key)
            .map(|day| day.count)
    }

    #[test]
    fn starts_empty_with_a_complete_calendar() {
        let stats = summarize(&[], now(), &Utc, Weekday::Sun, Range::All);
        assert_eq!(stats.conversations, 0);
        assert_eq!(stats.active_days, 0);
        assert_eq!(stats.hours, 0.0);
        assert_eq!(stats.streak, 0);
        assert_eq!(stats.median_minutes, None);
        assert_eq!(stats.timed_conversations, 0);
        assert_eq!(stats.conversation_days, 0);
        assert_eq!(stats.days[0].date.weekday(), Weekday::Sun);
        assert_eq!(stats.days.last().unwrap().date.to_string(), "2026-09-05");
        assert!(stats.days.len() >= 365);
    }

    #[test]
    fn deduplicates_sessions_and_overlapping_timing_while_retaining_resumed_recording() {
        let stats = summarize(
            &[
                record("a", "2026-09-04T09:00:00Z"),
                record("a", "2026-09-04T09:30:00Z"),
                record("a", "2026-09-04T12:00:00Z"),
                record("b", "2026-09-04T14:00:00Z"),
            ],
            now(),
            &Utc,
            Weekday::Sun,
            Range::All,
        );
        assert_eq!(stats.conversations, 2);
        assert_eq!(stats.active_days, 1);
        assert_eq!(stats.hours, 3.5);
        assert_eq!(day(&stats, "2026-09-04"), Some(2));
    }

    #[test]
    fn filters_calendar_days_in_the_selected_timezone_while_keeping_all_time_milestones() {
        let stats = summarize(
            &[
                record("old", "2026-08-29T14:59:00Z"),
                record("edge", "2026-08-29T15:00:00Z"),
                record("today", "2026-09-04T23:00:00Z"),
            ],
            now(),
            &chrono_tz::Asia::Seoul,
            Weekday::Mon,
            Range::Days7,
        );
        assert_eq!(stats.conversations, 2);
        assert_eq!(stats.active_days, 2);
        assert_eq!(stats.total_conversations, 3);
        assert_eq!(day(&stats, "2026-09-05"), Some(1));
    }

    #[test]
    fn keeps_a_weekly_streak_during_an_unfinished_week_and_resets_after_a_missed_week() {
        let records = [
            record("a", "2026-08-19T12:00:00Z"),
            record("b", "2026-08-26T12:00:00Z"),
        ];
        assert_eq!(
            summarize(&records, now(), &Utc, Weekday::Mon, Range::All).streak,
            2
        );
        let later: DateTime<Utc> = "2026-09-07T12:00:00Z".parse().unwrap();
        assert_eq!(
            summarize(&records, later, &Utc, Weekday::Mon, Range::All).streak,
            0
        );
    }

    #[test]
    fn handles_dst_without_dropping_or_duplicating_heatmap_days() {
        let stats = summarize(
            &[
                record("a", "2026-03-08T06:30:00Z"),
                record("b", "2026-03-08T07:30:00Z"),
            ],
            now(),
            &chrono_tz::America::New_York,
            Weekday::Sun,
            Range::All,
        );
        assert_eq!(stats.active_days, 1);
        let keys: BTreeSet<NaiveDate> = stats.days.iter().map(|day| day.date).collect();
        assert_eq!(keys.len(), stats.days.len());
        assert_eq!(day(&stats, "2026-03-08"), Some(2));
    }

    #[test]
    fn ignores_invalid_and_future_dates_falls_back_to_creation_time_and_caps_unfinished_timing() {
        let mut legacy = record("legacy", "2026-09-04T00:00:00Z");
        legacy.started_at_ms = 0;
        let stats = summarize(
            &[
                record("future", "2027-01-01T00:00:00Z"),
                record("invalid", "invalid"),
                legacy,
                record("current", "2026-09-05T11:30:00Z"),
            ],
            now(),
            &Utc,
            Weekday::Sun,
            Range::All,
        );
        assert_eq!(stats.conversations, 2);
        assert_eq!(stats.hours, 1.5);
    }

    #[test]
    fn counts_each_conversation_on_its_earliest_capture_weekday_regardless_of_transcript_order() {
        let stats = summarize(
            &[
                timed("resumed", "2026-09-03T09:00:00Z", 30 * 60_000),
                timed("resumed", "2026-09-02T09:00:00Z", 30 * 60_000),
                timed("resumed", "2026-09-02T09:15:00Z", 30 * 60_000),
                timed("second", "2026-09-02T14:00:00Z", 15 * 60_000),
                timed("untimed", "2026-09-04T14:00:00Z", 0),
            ],
            now(),
            &Utc,
            Weekday::Mon,
            Range::All,
        );
        let weekdays: Vec<Weekday> = stats.weekday_counts.iter().map(|(day, _)| *day).collect();
        assert_eq!(
            weekdays,
            [
                Weekday::Mon,
                Weekday::Tue,
                Weekday::Wed,
                Weekday::Thu,
                Weekday::Fri,
                Weekday::Sat,
                Weekday::Sun
            ]
        );
        let counts: Vec<usize> = stats
            .weekday_counts
            .iter()
            .map(|(_, count)| *count)
            .collect();
        assert_eq!(counts, [0, 0, 2, 0, 1, 0, 0]);
        assert_eq!(stats.conversations, 3);
        assert_eq!(stats.conversation_days, 2);
        assert_eq!(stats.timed_conversations, 2);
        assert_eq!(stats.median_minutes, Some(45.0));
    }

    #[test]
    fn uses_the_selected_period_and_timezone_for_weekdays_and_typical_length() {
        let stats = summarize(
            &[
                timed("resumed", "2026-08-01T09:00:00Z", 3_600_000),
                timed("resumed", "2026-08-29T15:00:00Z", 600_000),
                record("old", "2026-08-29T14:59:00Z"),
                timed("seoul-wednesday", "2026-09-01T16:00:00Z", 1_200_000),
                timed("third", "2026-09-04T16:00:00Z", 1_800_000),
            ],
            now(),
            &chrono_tz::Asia::Seoul,
            Weekday::Sun,
            Range::Days7,
        );
        let counts: Vec<usize> = stats
            .weekday_counts
            .iter()
            .map(|(_, count)| *count)
            .collect();
        assert_eq!(counts, [1, 0, 0, 1, 0, 0, 1]);
        assert_eq!(stats.median_minutes, Some(20.0));
        assert_eq!(stats.conversations, 3);
    }

    #[test]
    fn does_not_turn_missing_or_unfinished_timing_into_a_zero_minute_median() {
        let stats = summarize(
            &[
                timed("negative", "2026-09-03T09:00:00Z", -1),
                timed("zero", "2026-09-02T09:00:00Z", 0),
                record("just-started", "2026-09-05T12:00:00Z"),
                record("future", "2026-10-01T09:00:00Z"),
            ],
            now(),
            &Utc,
            Weekday::Sun,
            Range::All,
        );
        assert_eq!(stats.median_minutes, None);
        assert_eq!(stats.timed_conversations, 0);
        let total: usize = stats.weekday_counts.iter().map(|(_, count)| count).sum();
        assert_eq!(total, 3);
    }
}
