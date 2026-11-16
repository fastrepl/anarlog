//! Sidebar timeline model, ported from the shipping `sidebar/timeline`: upcoming calendar
//! events merge with the session page into date (or folder) buckets, with the current-time
//! indicator and the upcoming-meeting status.

use std::{cmp::Ordering as CmpOrdering, sync::Arc, time::Duration};

use anlg_fs_sync_core::normalize_folder_path;
use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use desktop_runtime::{
    CancellationToken, QueryWatch, Reply, Result, RuntimeHandle, SessionSummary,
};
use serde_json::Value;

use super::mutations::failure;

const UPCOMING_WINDOW_MS: i64 = 5 * 60 * 1000;

macro_rules! event_filter {
    () => {
        "e.deleted_at IS NULL
        AND COALESCE(CASE WHEN json_valid(e.attendance_json) THEN json_extract(e.attendance_json,'$.self_status') END,'')
            NOT IN ('pending','tentative','declined')
        AND NOT EXISTS(
            SELECT 1 FROM app_settings a,
                json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
            WHERE e.tracking_id_event <> '' AND ignored.type = 'object' AND (
                (a.id = 'ignored_events' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
                OR (a.id = 'ignored_recurring_series' AND e.recurrence_series_id <> ''
                    AND json_extract(ignored.value,'$.id') = e.recurrence_series_id)))"
    };
}

/// Events the timeline can show: not over yet and starting before the day after tomorrow in
/// any timezone. Events that already have a note are shown through that note instead.
const EVENT_QUERY: &str = concat!(
    "SELECT e.id, substr(e.title,1,4096) AS title, e.started_at, e.ended_at, e.is_all_day,
        e.tracking_id_event AS tracking, e.recurrence_series_id AS series
    FROM events e WHERE ",
    event_filter!(),
    " AND NOT EXISTS(SELECT 1 FROM sessions s WHERE s.deleted_at IS NULL AND (s.event_id = e.id
        OR (e.tracking_id_event <> '' AND s.event_json <> ''
            AND CASE WHEN json_valid(s.event_json) THEN json_extract(s.event_json,'$.tracking_id') END = e.tracking_id_event)))
    AND julianday(COALESCE(NULLIF(e.ended_at,''), e.started_at)) > julianday('now','-1 day')
    AND julianday(e.started_at) < julianday('now','+3 days')
    ORDER BY e.started_at, e.id LIMIT 200"
);

const LATER_QUERY: &str = concat!(
    "SELECT EXISTS(SELECT 1 FROM events e WHERE ",
    event_filter!(),
    " AND julianday(e.started_at) >= julianday('now','+3 days')) AS later"
);

#[derive(Clone, Debug, PartialEq)]
pub struct TimelineEvent {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
    pub all_day: bool,
    /// The provider's id for hiding the event; empty when it has none.
    pub tracking: Arc<str>,
    /// Recurring series; empty for one-off events.
    pub series: Arc<str>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventWindow {
    pub events: Arc<[TimelineEvent]>,
    /// An event starts after the queried window, so the calendar has more to show.
    pub later: bool,
}

pub fn load_events(
    runtime: &RuntimeHandle,
    cancel: CancellationToken,
) -> Result<Reply<EventWindow>> {
    runtime.read(cancel, move |services| async move {
        let rows = services
            .executor
            .execute(EVENT_QUERY.into(), vec![])
            .await
            .map_err(failure)?;
        let later = services
            .executor
            .execute(LATER_QUERY.into(), vec![])
            .await
            .map_err(failure)?;
        Ok(EventWindow {
            events: rows.iter().filter_map(event_row).collect(),
            later: later
                .first()
                .and_then(|row| row["later"].as_i64())
                .is_some_and(|later| later != 0),
        })
    })
}

/// Fires whenever events, notes linked to them or ignored-event settings change.
pub fn watch_events(runtime: &RuntimeHandle) -> Result<Reply<QueryWatch>> {
    runtime.watch_query(EVENT_QUERY.into(), vec![])
}

fn event_row(row: &Value) -> Option<TimelineEvent> {
    let text = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or_default();
    Some(TimelineEvent {
        id: text("id").into(),
        title: text("title").into(),
        start: parse_time(text("started_at"))?,
        end: parse_time(text("ended_at")),
        all_day: row.get("is_all_day").and_then(Value::as_i64) == Some(1),
        tracking: text("tracking").into(),
        series: text("series").into(),
    })
}

/// Shipping's event menu (`useIgnoredEvents`): hide one occurrence, or its whole series.
pub fn event_menu_labels(recurring: bool) -> Vec<String> {
    if recurring {
        vec![
            "Delete This Event".into(),
            "Delete All Recurring Events".into(),
        ]
    } else {
        vec!["Delete Event".into()]
    }
}

/// Hides an event by its tracking id, or a recurring series by its id.
pub fn ignore(
    runtime: &RuntimeHandle,
    kind: anlg_db_app::IgnoredCalendarItemKind,
    id: Arc<str>,
) -> Result<Reply<()>> {
    runtime.submit(move |services| async move {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        anlg_db_app::update_ignored_calendar_item(services.db.pool(), kind, &id, true, &now)
            .await
            .map_err(failure)
    })
}

/// Accepts RFC 3339 timestamps and the date-only form all-day events can use.
pub fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
}

pub fn local_time(date: DateTime<Utc>, timezone: Option<Tz>) -> DateTime<FixedOffset> {
    match timezone {
        Some(timezone) => date.with_timezone(&timezone).fixed_offset(),
        None => date.with_timezone(&Local).fixed_offset(),
    }
}

pub fn folder_label(path: &str) -> String {
    let normalized = normalize_folder_path(path.trim()).unwrap_or_default();
    if normalized.encode_utf16().count() > 200
        || normalized
            .split('/')
            .any(|segment| segment.encode_utf16().count() > 80)
    {
        String::new()
    } else {
        normalized
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Time,
    Date,
}

pub fn bucket(date: NaiveDate, today: NaiveDate) -> (String, Precision) {
    let days = date.signed_duration_since(today).num_days();
    let label = match days {
        0 => "Today".into(),
        -1 => "Yesterday".into(),
        1 => "Tomorrow".into(),
        -6..=-2 => format!("{} days ago", -days),
        2..=6 => format!("in {days} days"),
        -27..=27 => {
            let weeks = (days.unsigned_abs() + 3) / 7;
            let label = match (days < 0, weeks) {
                (true, 1) => "a week ago".into(),
                (true, _) => format!("{weeks} weeks ago"),
                (false, 1) => "next week".into(),
                (false, _) => format!("in {weeks} weeks"),
            };
            return (label, Precision::Date);
        }
        _ => {
            let months = ((date.year() - today.year()) * 12 + date.month() as i32
                - today.month() as i32)
                .unsigned_abs()
                .max(1);
            let label = match (days < 0, months) {
                (true, 1) => "a month ago".into(),
                (true, _) => format!("{months} months ago"),
                (false, 1) => "next month".into(),
                (false, _) => format!("in {months} months"),
            };
            return (label, Precision::Date);
        }
    };
    (label, Precision::Time)
}

fn is_future_bucket(label: &str) -> bool {
    matches!(label, "Tomorrow" | "next week" | "next month") || label.starts_with("in ")
}

/// The item time; date precision (week and month buckets, folder grouping) adds the date and,
/// outside the current year, the year.
pub fn timestamp(
    date: DateTime<FixedOffset>,
    precision: Precision,
    current_year: i32,
    use_24_hour_time: bool,
) -> String {
    let time = date
        .format(if use_24_hour_time {
            "%H:%M"
        } else {
            "%-I:%M %p"
        })
        .to_string();
    if precision == Precision::Time {
        return time;
    }
    let date = date.format(if date.year() == current_year {
        "%b %-d"
    } else {
        "%b %-d, %Y"
    });
    format!("{date}, {time}")
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grouping {
    #[default]
    Date,
    Folder,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ordering {
    #[default]
    Newest,
    Oldest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub grouping: Grouping,
    pub ordering: Ordering,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Session(usize),
    Event(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub source: Source,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub precision: Precision,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    /// Room for the "Open calendar" chip above the first bucket.
    TopSpacer,
    Heading(Arc<str>),
    /// `indicator` places the current-time line inside an ongoing item, as a fraction of its
    /// height from the top.
    Item {
        item: usize,
        indicator: Option<f32>,
    },
    /// The current-time line between items, with the shipping gaps around it.
    Now {
        above: f32,
        below: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Upcoming {
    pub item: usize,
    pub label: String,
    /// Time left in the five-minute lead-up; 1 while the meeting is in progress.
    pub progress: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    pub rows: Vec<Row>,
    pub items: Vec<Item>,
    /// Sessions or events exist past tomorrow (shipping `hasMoreFutureItems`).
    pub later: bool,
    pub upcoming: Option<Upcoming>,
    pub now: Option<DateTime<Utc>>,
}

impl Timeline {
    pub fn now_row(&self) -> Option<usize> {
        self.rows.iter().position(|row| {
            matches!(
                row,
                Row::Now { .. }
                    | Row::Item {
                        indicator: Some(_),
                        ..
                    }
            )
        })
    }

    pub fn item_row(&self, item: usize) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| matches!(row, Row::Item { item: found, .. } if *found == item))
    }

    /// The heading of the bucket a row belongs to.
    pub fn heading_for(&self, row: usize) -> Option<(usize, &Arc<str>)> {
        self.rows
            .get(..=row.min(self.rows.len().checked_sub(1)?))?
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, row)| match row {
                Row::Heading(label) => Some((index, label)),
                _ => None,
            })
    }

    /// When the clock next changes what the timeline shows: every second while a meeting is
    /// about to start, otherwise the next minute or item boundary.
    pub fn next_refresh(&self) -> Duration {
        let Some(now) = self.now else {
            return Duration::from_secs(60);
        };
        let now_ms = now.timestamp_millis();
        if self
            .upcoming
            .as_ref()
            .is_some_and(|status| status.progress < 1.)
        {
            return Duration::from_millis((1000 - now_ms.rem_euclid(1000)).max(1) as u64);
        }
        let mut next = now_ms - now_ms.rem_euclid(60_000) + 60_000 + 100;
        for item in &self.items {
            let Some(start) = item.start.map(|start| start.timestamp_millis()) else {
                continue;
            };
            let end = item.end.map(|end| end.timestamp_millis() + 1);
            for deadline in [Some(start - UPCOMING_WINDOW_MS), Some(start), end]
                .into_iter()
                .flatten()
            {
                if deadline > now_ms && deadline < next {
                    next = deadline;
                }
            }
        }
        Duration::from_millis((next - now_ms).max(1) as u64)
    }
}

struct Bucket {
    label: Arc<str>,
    items: Vec<usize>,
}

pub fn build(
    sessions: &[SessionSummary],
    events: &EventWindow,
    view: View,
    now: DateTime<Utc>,
    timezone: Option<Tz>,
) -> Timeline {
    let today = local_time(now, timezone).date_naive();
    let upper = tomorrow_upper_bound(now, timezone);
    let date_grouping = view.grouping == Grouping::Date;
    let mut later = date_grouping && events.later;
    let mut items = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        // Notes created from a calendar event sit at the event's time, like shipping.
        let start = parse_time(session.starts_at());
        if date_grouping && start.is_some_and(|start| start >= upper) {
            later = true;
            continue;
        }
        items.push(Item {
            source: Source::Session(index),
            start,
            end: parse_time(&session.event_end).filter(|_| !session.event_start.is_empty()),
            precision: Precision::Date,
        });
    }
    if date_grouping {
        for (index, event) in events.events.iter().enumerate() {
            if event.start >= upper {
                later = true;
            } else if event.end.unwrap_or(event.start) > now {
                items.push(Item {
                    source: Source::Event(index),
                    start: Some(event.start),
                    end: event.end,
                    precision: Precision::Time,
                });
            }
        }
    }
    let title = |item: &Item| -> &str {
        match item.source {
            Source::Session(index) => &sessions[index].title,
            Source::Event(index) => &events.events[index].title,
        }
    };
    items.sort_by(|left, right| {
        let order = left.start.cmp(&right.start);
        let order = match view.ordering {
            Ordering::Newest => order.reverse(),
            Ordering::Oldest => order,
        };
        order.then_with(|| title(left).cmp(title(right)))
    });

    let mut buckets: Vec<Bucket> = Vec::new();
    match view.grouping {
        Grouping::Date => {
            for (index, item) in items.iter_mut().enumerate() {
                let (label, precision) = match item.start {
                    Some(start) => bucket(local_time(start, timezone).date_naive(), today),
                    None => ("Undated".into(), Precision::Date),
                };
                item.precision = precision;
                match buckets.last_mut() {
                    Some(bucket) if *bucket.label == *label => bucket.items.push(index),
                    _ => buckets.push(Bucket {
                        label: label.into(),
                        items: vec![index],
                    }),
                }
            }
        }
        Grouping::Folder => {
            let mut folders: Vec<(String, Vec<usize>)> = Vec::new();
            for (index, item) in items.iter().enumerate() {
                let Source::Session(session) = item.source else {
                    continue;
                };
                let key = folder_label(&sessions[session].folder_path);
                match folders.iter_mut().find(|(folder, _)| *folder == key) {
                    Some((_, members)) => members.push(index),
                    None => folders.push((key, vec![index])),
                }
            }
            folders.sort_by(
                |(left, _), (right, _)| match (left.is_empty(), right.is_empty()) {
                    (true, false) => CmpOrdering::Greater,
                    (false, true) => CmpOrdering::Less,
                    _ => left
                        .to_lowercase()
                        .cmp(&right.to_lowercase())
                        .then(left.cmp(right)),
                },
            );
            buckets = folders
                .into_iter()
                .map(|(folder, items)| Bucket {
                    label: if folder.is_empty() {
                        "No folder".into()
                    } else {
                        folder.into()
                    },
                    items,
                })
                .collect();
        }
    }

    let mut rows = Vec::new();
    if later {
        rows.push(Row::TopSpacer);
    }
    let today_bucket = buckets.iter().position(|bucket| &*bucket.label == "Today");
    let fallback = (date_grouping && today_bucket.is_none())
        .then(|| fallback_indicator(&buckets, &items, now, view.ordering))
        .flatten();
    for (index, bucket) in buckets.iter().enumerate() {
        if date_grouping && today_bucket.is_none() && fallback == Some(index) {
            rows.push(Row::Now {
                above: if index == 0 { 24. } else { 12. },
                below: 12.,
            });
        }
        rows.push(Row::Heading(bucket.label.clone()));
        let placement = (today_bucket == Some(index))
            .then(|| today_placement(&bucket.items, &items, now, view.ordering));
        for (position, &item) in bucket.items.iter().enumerate() {
            match placement {
                Some(Placement::Before(at)) if at == position => rows.push(Row::Now {
                    above: 0.,
                    below: 0.,
                }),
                _ => {}
            }
            rows.push(Row::Item {
                item,
                indicator: match placement {
                    Some(Placement::Inside(at, progress)) if at == position => Some(1. - progress),
                    _ => None,
                },
            });
        }
        if placement == Some(Placement::After) {
            rows.push(Row::Now {
                above: 0.,
                below: 0.,
            });
        }
    }
    if date_grouping && today_bucket.is_none() && fallback.is_none() {
        rows.push(Row::Now {
            above: 0.,
            below: 0.,
        });
    }

    let upcoming = upcoming(&items, now, |item| match item.source {
        Source::Event(index) => events.events[index].all_day,
        Source::Session(_) => false,
    });
    Timeline {
        rows,
        items,
        later,
        upcoming: upcoming.filter(|_| date_grouping),
        now: Some(now),
    }
}

/// Start of the day after tomorrow, local to the timeline.
fn tomorrow_upper_bound(now: DateTime<Utc>, timezone: Option<Tz>) -> DateTime<Utc> {
    let day = local_time(now + chrono::Duration::days(2), timezone).date_naive();
    let midnight = day.and_hms_opt(0, 0, 0).unwrap();
    let local = match timezone {
        Some(timezone) => timezone
            .from_local_datetime(&midnight)
            .earliest()
            .map(|date| date.with_timezone(&Utc)),
        None => Local
            .from_local_datetime(&midnight)
            .earliest()
            .map(|date| date.with_timezone(&Utc)),
    };
    local.unwrap_or_else(|| midnight.and_utc())
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Placement {
    Inside(usize, f32),
    Before(usize),
    After,
}

fn today_placement(
    members: &[usize],
    items: &[Item],
    now: DateTime<Utc>,
    ordering: Ordering,
) -> Placement {
    for (position, &item) in members.iter().enumerate() {
        if let (Some(start), Some(end)) = (items[item].start, items[item].end)
            && start <= now
            && now <= end
            && end > start
        {
            let progress =
                (now - start).num_milliseconds() as f32 / (end - start).num_milliseconds() as f32;
            return Placement::Inside(position, progress.clamp(0., 1.));
        }
    }
    members
        .iter()
        .position(|&item| match items[item].start {
            None => true,
            Some(start) => match ordering {
                Ordering::Oldest => start >= now,
                Ordering::Newest => start < now,
            },
        })
        .map_or(Placement::After, Placement::Before)
}

/// The bucket the current-time line precedes when there is no Today bucket; `None` puts it
/// after the last bucket.
fn fallback_indicator(
    buckets: &[Bucket],
    items: &[Item],
    now: DateTime<Utc>,
    ordering: Ordering,
) -> Option<usize> {
    if ordering == Ordering::Oldest {
        return buckets.iter().position(|bucket| {
            is_future_bucket(&bucket.label)
                || bucket
                    .items
                    .last()
                    .and_then(|&item| items[item].start)
                    .is_some_and(|start| start >= now)
        });
    }
    let mut stale_future = None;
    for (index, bucket) in buckets.iter().enumerate() {
        let Some(&first) = bucket.items.first() else {
            continue;
        };
        match items[first].start {
            Some(start) if start < now => {}
            _ => continue,
        }
        if is_future_bucket(&bucket.label) {
            stale_future = Some(index + 1);
            continue;
        }
        return Some(stale_future.unwrap_or(index));
    }
    stale_future.filter(|&index| index < buckets.len())
}

fn upcoming(
    items: &[Item],
    now: DateTime<Utc>,
    all_day: impl Fn(&Item) -> bool,
) -> Option<Upcoming> {
    let mut active: Option<(usize, DateTime<Utc>)> = None;
    let mut nearest: Option<(usize, i64)> = None;
    for (index, item) in items.iter().enumerate() {
        let Some(start) = item.start else {
            continue;
        };
        if all_day(item) {
            continue;
        }
        if let Some(end) = item.end
            && end > start
            && start <= now
            && now <= end
        {
            if active.is_none_or(|(_, current)| end < current) {
                active = Some((index, end));
            }
            continue;
        }
        let diff = (start - now).num_milliseconds();
        if diff > 0 && diff <= UPCOMING_WINDOW_MS && nearest.is_none_or(|(_, best)| diff < best) {
            nearest = Some((index, diff));
        }
    }
    if let Some((item, _)) = active {
        return Some(Upcoming {
            item,
            label: "Now".into(),
            progress: 1.,
        });
    }
    let (item, diff) = nearest?;
    let seconds = (diff / 1000).max(1);
    Some(Upcoming {
        item,
        label: if seconds < 60 {
            format!("In {seconds}s")
        } else {
            format!("In {}m {}s", seconds / 60, seconds % 60)
        },
        progress: (diff as f32 / UPCOMING_WINDOW_MS as f32).clamp(0., 1.),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_runtime::SessionId;

    fn at(value: &str) -> DateTime<Utc> {
        parse_time(value).unwrap()
    }

    fn session(id: &str, title: &str, created_at: &str, folder: &str) -> SessionSummary {
        SessionSummary {
            id: SessionId(id.into()),
            title: title.into(),
            updated_at: created_at.into(),
            created_at: created_at.into(),
            folder_path: folder.into(),
            tag_line: "".into(),
            event_start: "".into(),
            event_end: "".into(),
        }
    }

    fn event(id: &str, start: &str, end: &str) -> TimelineEvent {
        TimelineEvent {
            id: id.into(),
            title: id.into(),
            start: at(start),
            end: parse_time(end),
            all_day: false,
            tracking: "".into(),
            series: "".into(),
        }
    }

    fn labels(
        timeline: &Timeline,
        sessions: &[SessionSummary],
        events: &EventWindow,
    ) -> Vec<String> {
        timeline
            .rows
            .iter()
            .map(|row| match row {
                Row::TopSpacer => "spacer".into(),
                Row::Heading(label) => format!("# {label}"),
                Row::Now { .. } => "now".into(),
                Row::Item { item, indicator } => {
                    let title = match timeline.items[*item].source {
                        Source::Session(index) => sessions[index].title.to_string(),
                        Source::Event(index) => events.events[index].title.to_string(),
                    };
                    match indicator {
                        Some(_) => format!("{title} (now inside)"),
                        None => title,
                    }
                }
            })
            .collect()
    }

    const UTC: Option<Tz> = Some(chrono_tz::UTC);

    #[test]
    fn events_merge_into_tomorrow_and_today_around_the_current_time() {
        let now = at("2026-11-16T12:00:00Z");
        let sessions = [
            session("a", "Morning notes", "2026-11-16T09:00:00Z", ""),
            session("b", "Last week", "2026-11-09T09:00:00Z", ""),
        ];
        let events = EventWindow {
            events: vec![
                event("Review", "2026-11-16T14:00:00Z", "2026-11-16T15:00:00Z"),
                event("Standup", "2026-11-17T09:30:00Z", "2026-11-17T09:45:00Z"),
                event("Planning", "2026-11-18T09:00:00Z", "2026-11-18T10:00:00Z"),
                event("Finished", "2026-11-16T08:00:00Z", "2026-11-16T09:00:00Z"),
            ]
            .into(),
            later: false,
        };
        let timeline = build(&sessions, &events, View::default(), now, UTC);
        assert!(timeline.later, "Planning starts after tomorrow");
        assert_eq!(
            labels(&timeline, &sessions, &events),
            [
                "spacer",
                "# Tomorrow",
                "Standup",
                "# Today",
                "Review",
                "now",
                "Morning notes",
                "# a week ago",
                "Last week"
            ]
        );
        assert_eq!(timeline.upcoming, None);
    }

    #[test]
    fn notes_from_calendar_events_sit_at_the_event_time() {
        let now = at("2026-11-16T12:00:00Z");
        let mut standup = session("a", "Standup notes", "2026-11-10T09:00:00Z", "");
        standup.event_start = "2026-11-17T09:30:00Z".into();
        standup.event_end = "2026-11-17T09:45:00Z".into();
        let mut sync = session("b", "Sync notes", "2026-11-16T08:00:00Z", "");
        sync.event_start = "2026-11-16T11:30:00Z".into();
        sync.event_end = "2026-11-16T12:30:00Z".into();
        let sessions = [
            standup,
            sync,
            session("c", "Plain", "2026-11-16T09:00:00Z", ""),
        ];
        let events = EventWindow::default();
        let timeline = build(&sessions, &events, View::default(), now, UTC);
        assert_eq!(
            labels(&timeline, &sessions, &events),
            [
                "# Tomorrow",
                "Standup notes",
                "# Today",
                "Sync notes (now inside)",
                "Plain"
            ]
        );
    }

    #[test]
    fn the_current_time_sits_inside_an_ongoing_meeting_and_marks_it_upcoming() {
        let now = at("2026-11-16T12:15:00Z");
        let events = EventWindow {
            events: vec![event(
                "Sync",
                "2026-11-16T12:00:00Z",
                "2026-11-16T13:00:00Z",
            )]
            .into(),
            later: false,
        };
        let timeline = build(&[], &events, View::default(), now, UTC);
        assert_eq!(
            timeline.rows,
            [
                Row::Heading("Today".into()),
                Row::Item {
                    item: 0,
                    indicator: Some(0.75)
                }
            ]
        );
        assert_eq!(
            timeline.upcoming,
            Some(Upcoming {
                item: 0,
                label: "Now".into(),
                progress: 1.
            })
        );
    }

    #[test]
    fn meetings_within_five_minutes_count_down_every_second() {
        let now = at("2026-11-16T11:55:30Z");
        let events = EventWindow {
            events: vec![
                event("Soon", "2026-11-16T11:59:00Z", "2026-11-16T12:30:00Z"),
                event("Later", "2026-11-16T12:01:00Z", "2026-11-16T12:30:00Z"),
            ]
            .into(),
            later: false,
        };
        let timeline = build(&[], &events, View::default(), now, UTC);
        let upcoming = timeline.upcoming.clone().unwrap();
        assert_eq!(
            timeline.items[upcoming.item].start,
            Some(at("2026-11-16T11:59:00Z"))
        );
        assert_eq!(upcoming.label, "In 3m 30s");
        assert_eq!(timeline.next_refresh(), Duration::from_millis(1000));

        let quiet = build(&[], &EventWindow::default(), View::default(), now, UTC);
        assert_eq!(quiet.upcoming, None);
        assert_eq!(quiet.next_refresh(), Duration::from_millis(30_100));
    }

    #[test]
    fn without_a_today_bucket_the_line_precedes_the_first_past_bucket() {
        let now = at("2026-11-16T12:00:00Z");
        let sessions = [
            session("a", "Yesterday note", "2026-11-15T09:00:00Z", ""),
            session("b", "Older", "2026-11-12T09:00:00Z", ""),
        ];
        let events = EventWindow {
            events: vec![event(
                "Standup",
                "2026-11-17T09:30:00Z",
                "2026-11-17T09:45:00Z",
            )]
            .into(),
            later: true,
        };
        let timeline = build(&sessions, &events, View::default(), now, UTC);
        assert_eq!(
            labels(&timeline, &sessions, &events),
            [
                "spacer",
                "# Tomorrow",
                "Standup",
                "now",
                "# Yesterday",
                "Yesterday note",
                "# 4 days ago",
                "Older"
            ]
        );
        assert_eq!(
            timeline.rows[3],
            Row::Now {
                above: 12.,
                below: 12.
            }
        );

        let past_only = build(
            &sessions[1..],
            &EventWindow::default(),
            View::default(),
            now,
            UTC,
        );
        assert_eq!(
            past_only.rows[0],
            Row::Now {
                above: 24.,
                below: 12.
            }
        );
        assert!(!past_only.later);
    }

    #[test]
    fn folder_grouping_drops_events_and_sorts_folders_with_unfiled_last() {
        let now = at("2026-11-16T12:00:00Z");
        let sessions = [
            session("a", "Plain", "2026-11-16T09:00:00Z", ""),
            session("b", "Research one", "2026-11-10T09:00:00Z", "research"),
            session("c", "Admin", "2026-11-11T09:00:00Z", "Admin"),
            session("d", "Research two", "2026-11-12T09:00:00Z", "research"),
            session("e", "Future", "2026-11-20T09:00:00Z", ""),
        ];
        let events = EventWindow {
            events: vec![event(
                "Review",
                "2026-11-16T14:00:00Z",
                "2026-11-16T15:00:00Z",
            )]
            .into(),
            later: true,
        };
        let view = View {
            grouping: Grouping::Folder,
            ordering: Ordering::Oldest,
        };
        let timeline = build(&sessions, &events, view, now, UTC);
        assert!(!timeline.later);
        assert_eq!(
            labels(&timeline, &sessions, &events),
            [
                "# Admin",
                "Admin",
                "# research",
                "Research one",
                "Research two",
                "# No folder",
                "Plain",
                "Future"
            ]
        );
    }

    #[test]
    fn oldest_first_reverses_items_and_moves_the_line_before_upcoming_items() {
        let now = at("2026-11-16T12:00:00Z");
        let sessions = [
            session("a", "Late", "2026-11-16T10:00:00Z", ""),
            session("b", "Early", "2026-11-16T08:00:00Z", ""),
        ];
        let events = EventWindow {
            events: vec![event(
                "Review",
                "2026-11-16T14:00:00Z",
                "2026-11-16T15:00:00Z",
            )]
            .into(),
            later: false,
        };
        let view = View {
            grouping: Grouping::Date,
            ordering: Ordering::Oldest,
        };
        let timeline = build(&sessions, &events, view, now, UTC);
        assert_eq!(
            labels(&timeline, &sessions, &events),
            ["# Today", "Early", "Late", "now", "Review"]
        );
    }

    #[test]
    fn sidebar_folders_follow_shipping_normalization_and_utf16_limits() {
        for (input, expected) in [
            ("  Work\\日本語/  ", "Work/日本語"),
            ("   ", ""),
            ("/Work", ""),
            ("Work//Meetings", ""),
            ("Work/../Meetings", ""),
            ("./Work", ""),
        ] {
            assert_eq!(folder_label(input), expected);
        }
        assert_eq!(folder_label(&"🚀".repeat(40)), "🚀".repeat(40));
        assert_eq!(folder_label(&"🚀".repeat(41)), "");
        assert_eq!(folder_label(&format!("{0}/{0}/{0}", "a".repeat(70))), "");
    }

    fn time(date: DateTime<FixedOffset>, today: NaiveDate, use_24_hour_time: bool) -> String {
        let precision = bucket(date.date_naive(), today).1;
        timestamp(date, precision, today.year(), use_24_hour_time)
    }

    #[test]
    fn timeline_timestamps_keep_time_and_only_show_the_year_when_needed() {
        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        for (date, twelve_hour, twenty_four_hour) in [
            ("2026-01-15T00:05:00Z", "12:05 AM", "00:05"),
            ("2026-01-15T12:05:00Z", "12:05 PM", "12:05"),
            ("2026-01-09T23:59:00Z", "11:59 PM", "23:59"),
            ("2026-01-08T13:02:00Z", "Jan 8, 1:02 PM", "Jan 8, 13:02"),
            ("2026-01-21T13:02:00Z", "1:02 PM", "13:02"),
            ("2026-01-22T13:02:00Z", "Jan 22, 1:02 PM", "Jan 22, 13:02"),
            (
                "2025-12-31T13:02:00Z",
                "Dec 31, 2025, 1:02 PM",
                "Dec 31, 2025, 13:02",
            ),
        ] {
            let date = DateTime::parse_from_rfc3339(date).unwrap();
            assert_eq!(time(date, today, false), twelve_hour);
            assert_eq!(time(date, today, true), twenty_four_hour);
        }
    }

    #[test]
    fn timeline_timezone_applies_to_grouping_year_boundaries_and_dst() {
        let zone = Some(chrono_tz::America::Los_Angeles);
        let date = |value: &str| local_time(at(value), zone);
        let previous_year = date("2026-01-01T00:30:00Z");
        let today = date("2026-01-08T00:00:00Z").date_naive();
        assert_eq!(bucket(previous_year.date_naive(), today).0, "a week ago");
        assert_eq!(time(previous_year, today, false), "Dec 31, 2025, 4:30 PM");

        let before = date("2026-03-08T09:59:00Z");
        let after = date("2026-03-08T10:00:00Z");
        assert_eq!(time(before, after.date_naive(), false), "1:59 AM");
        assert_eq!(time(after, after.date_naive(), false), "3:00 AM");
        assert_eq!(bucket(before.date_naive(), after.date_naive()).0, "Today");

        for instant in ["2026-11-01T08:30:00Z", "2026-11-01T09:30:00Z"] {
            let repeated_hour = date(instant);
            assert_eq!(
                time(repeated_hour, repeated_hour.date_naive(), true),
                "01:30"
            );
        }
    }

    #[test]
    fn timeline_buckets_match_the_source_relative_day_week_and_month_boundaries() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 21).unwrap();
        for (days, label) in [
            (0, "Today"),
            (-1, "Yesterday"),
            (1, "Tomorrow"),
            (-6, "6 days ago"),
            (-7, "a week ago"),
            (-11, "2 weeks ago"),
            (-28, "a month ago"),
            (14, "in 2 weeks"),
        ] {
            assert_eq!(bucket(today + chrono::Duration::days(days), today).0, label);
        }
    }
}
