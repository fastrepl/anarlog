//! Port of the Tauri sidebar's timeline rules
//! (`apps/desktop/src/sidebar/timeline/utils/index.ts` and `item.tsx`). Any
//! change there must be mirrored here; the tests below reuse its fixtures.

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub event_json: String,
    pub folder_id: String,
    pub locked: i64,
}

/// `useTimelineEventsTable` row: calendar events not yet backed by a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct EventRow {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub tracking_id_event: String,
    pub is_all_day: i64,
    pub meeting_link: String,
    pub calendar_color: String,
    pub calendar_id: String,
    pub recurrence_series_id: String,
    pub location: String,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Time,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Session,
    /// A calendar event with no session yet; the app opens one on click.
    Event,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    pub id: String,
    pub title: String,
    pub timestamp: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub locked: bool,
    pub folder_id: String,
    pub event: Option<SessionEvent>,
    /// `isIgnored(tracking_id_event, recurrence_series_id)` while the timeline
    /// shows deleted events: the row renders dimmed, struck through, inert.
    pub ignored: bool,
}

/// The parts of `sessions.event_json` the main window reads. Fields stay
/// `Option` because the app's `??` fallbacks only fire for missing keys, not
/// for empty strings.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct SessionEvent {
    pub tracking_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub meeting_link: Option<String>,
    pub location: Option<String>,
    #[serde(default)]
    pub participants: Vec<serde_json::Value>,
}

impl SessionEvent {
    pub fn parse(event_json: &str) -> Option<Self> {
        if event_json.trim().is_empty() {
            return None;
        }
        serde_json::from_str(event_json).ok()
    }

    /// `apps/desktop/src/session/hooks/useRemoteMeeting.ts`.
    pub fn remote_meeting(&self) -> Option<RemoteMeeting> {
        let url = url::Url::parse(self.meeting_link.as_deref()?).ok()?;
        let host = url.host_str()?;
        if host.contains("zoom.us") {
            Some(RemoteMeeting::Zoom)
        } else if host.contains("meet.google.com") {
            Some(RemoteMeeting::GoogleMeet)
        } else if host.contains("webex.com") {
            Some(RemoteMeeting::Webex)
        } else if host.contains("teams.microsoft.com") {
            Some(RemoteMeeting::Teams)
        } else if host == "app.cal.com" && url.path().starts_with("/video/") {
            Some(RemoteMeeting::CalCom)
        } else {
            None
        }
    }

    pub fn is_welcome_demo(&self) -> bool {
        self.tracking_id.as_deref() == Some("anarlog-onboarding-demo-v1")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteMeeting {
    Zoom,
    GoogleMeet,
    Webex,
    Teams,
    CalCom,
}

const MAX_FOLDER_PATH_LENGTH: usize = 200;
const MAX_FOLDER_SEGMENT_LENGTH: usize = 80;

/// `normalizeFolderPath` from `apps/desktop/src/session/folders.ts`.
pub fn normalize_folder_path(path: &str) -> Option<String> {
    let replaced = path.replace('\\', "/");
    let replaced = replaced.trim();
    if replaced.is_empty() {
        return Some(String::new());
    }
    if replaced.starts_with('/') {
        return None;
    }
    let mut segments = Vec::new();
    for raw in replaced.trim_end_matches('/').split('/') {
        if raw.is_empty() || raw == "." || raw == ".." || raw.len() > MAX_FOLDER_SEGMENT_LENGTH {
            return None;
        }
        segments.push(raw);
    }
    let normalized = segments.join("/");
    (normalized.len() <= MAX_FOLDER_PATH_LENGTH).then_some(normalized)
}

/// The folder line under a sidebar row (`sidebar_show_folder` defaults on).
pub fn folder_label(folder_id: &str) -> Option<String> {
    normalize_folder_path(folder_id).filter(|label| !label.is_empty())
}

/// `calculateTodayIndicatorPlacement`: where the red current-time line sits
/// among a bucket's items (sorted newest first).
#[derive(Debug, Clone, PartialEq)]
pub enum IndicatorPlacement {
    /// Overlaid on the item at `index`, `progress` (0..1) of the way through it.
    Inside { index: usize, progress: f32 },
    /// Between items, before `index`.
    Before { index: usize },
    /// After every item.
    After,
}

/// `SidebarNotesGroupBy`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupBy {
    #[default]
    Date,
    Folder,
}

/// `SidebarNotesSortOrder`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Newest,
    Oldest,
}

/// `calculateTodayIndicatorPlacement` with `calculateIndicatorIndex`'s
/// order-aware boundary.
pub fn indicator_placement(
    items: &[Item],
    now: DateTime<Utc>,
    order: SortOrder,
) -> IndicatorPlacement {
    let now_ms = now.timestamp_millis();
    if let Some((index, item)) = items.iter().enumerate().find(|(_, item)| {
        let start = item.timestamp.timestamp_millis();
        item.ended_at.is_some_and(|end| {
            let end = end.timestamp_millis();
            start <= now_ms && now_ms <= end && end > start
        })
    }) {
        let start = item.timestamp.timestamp_millis();
        let end = item.ended_at.expect("checked above").timestamp_millis();
        return IndicatorPlacement::Inside {
            index,
            progress: (now_ms - start) as f32 / (end - start) as f32,
        };
    }
    let boundary = |item: &Item| match order {
        SortOrder::Newest => item.timestamp.timestamp_millis() < now_ms,
        SortOrder::Oldest => item.timestamp.timestamp_millis() >= now_ms,
    };
    match items.iter().position(boundary) {
        Some(index) => IndicatorPlacement::Before { index },
        None => IndicatorPlacement::After,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub label: String,
    pub precision: Precision,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline {
    pub buckets: Vec<Bucket>,
    /// Sessions after the end of tomorrow are hidden; the sidebar shows a chip
    /// instead of listing them.
    pub has_more_future_items: bool,
}

/// `safeParseDate`: JavaScript `new Date(string)` semantics for the shapes
/// the app stores: RFC 3339, ISO date-time without offset (local time), and
/// date-only (UTC midnight).
pub fn parse_date<Tz: TimeZone>(value: &str, tz: &Tz) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Some(parsed.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
        return tz
            .from_local_datetime(&naive)
            .earliest()
            .map(|local| local.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?));
    }
    None
}

/// `getSessionEvent(row)?.started_at ?? row.created_at`.
pub fn session_timestamp<Tz: TimeZone>(row: &SessionRow, tz: &Tz) -> Option<DateTime<Utc>> {
    let event = SessionEvent::parse(&row.event_json);
    let source = event
        .as_ref()
        .and_then(|event| event.started_at.as_deref())
        .unwrap_or(&row.created_at);
    parse_date(source, tz)
}

fn display_title(title: &str) -> &str {
    if title.is_empty() { "Untitled" } else { title }
}

/// `localeCompare` approximation: case-insensitive first, then exact.
fn compare_titles(left: &str, right: &str) -> std::cmp::Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn start_of_day_ms<Tz: TimeZone>(date: DateTime<Utc>, tz: &Tz) -> i64 {
    let local = date.with_timezone(tz);
    let midnight = local
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight exists");
    tz.from_local_datetime(&midnight)
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| local.timestamp_millis())
}

fn start_of_month_ms<Tz: TimeZone>(date: DateTime<Utc>, tz: &Tz) -> i64 {
    let local = date.with_timezone(tz);
    let first = NaiveDate::from_ymd_opt(local.year(), local.month(), 1)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .expect("first of month exists");
    tz.from_local_datetime(&first)
        .earliest()
        .map(|d| d.timestamp_millis())
        .unwrap_or_else(|| local.timestamp_millis())
}

fn calendar_days_between<Tz: TimeZone>(date: DateTime<Utc>, now: DateTime<Utc>, tz: &Tz) -> i64 {
    (date.with_timezone(tz).date_naive() - now.with_timezone(tz).date_naive()).num_days()
}

fn calendar_months_between<Tz: TimeZone>(date: DateTime<Utc>, now: DateTime<Utc>, tz: &Tz) -> i64 {
    let date = date.with_timezone(tz);
    let now = now.with_timezone(tz);
    (i64::from(date.year()) - i64::from(now.year())) * 12
        + (i64::from(date.month()) - i64::from(now.month()))
}

fn shift(now: DateTime<Utc>, days: i64) -> DateTime<Utc> {
    now + Duration::milliseconds(days * DAY_MS)
}

/// `getBucketInfo`. Returns the label, the key buckets sort by, and whether
/// rows in the bucket show a time or a date.
pub fn bucket_info<Tz: TimeZone>(
    date: DateTime<Utc>,
    now: DateTime<Utc>,
    tz: &Tz,
) -> (String, i64, Precision) {
    let days_diff = calendar_days_between(date, now, tz);
    let sort_key = start_of_day_ms(date, tz);
    let abs_days = days_diff.abs();

    match days_diff {
        0 => return ("Today".into(), sort_key, Precision::Time),
        -1 => return ("Yesterday".into(), sort_key, Precision::Time),
        1 => return ("Tomorrow".into(), sort_key, Precision::Time),
        _ => {}
    }

    if days_diff < 0 {
        if abs_days <= 6 {
            return (format!("{abs_days} days ago"), sort_key, Precision::Time);
        }
        if abs_days <= 27 {
            let weeks = ((abs_days as f64 / 7.0).round() as i64).max(1);
            let week_range_end_day = (weeks * 7 - 3).max(7);
            let week_sort_key = start_of_day_ms(shift(now, -week_range_end_day), tz);
            let label = if weeks == 1 {
                "a week ago".to_string()
            } else {
                format!("{weeks} weeks ago")
            };
            return (label, week_sort_key, Precision::Date);
        }
        let months = calendar_months_between(date, now, tz).abs().max(1);
        let month_sort_key = start_of_month_ms(date, tz).min(start_of_day_ms(shift(now, -28), tz));
        let label = if months == 1 {
            "a month ago".to_string()
        } else {
            format!("{months} months ago")
        };
        return (label, month_sort_key, Precision::Date);
    }

    if abs_days <= 6 {
        return (format!("in {abs_days} days"), sort_key, Precision::Time);
    }
    if abs_days <= 27 {
        let weeks = ((abs_days as f64 / 7.0).round() as i64).max(1);
        let week_range_start_day = (weeks * 7 - 3).max(7);
        let week_sort_key = start_of_day_ms(shift(now, week_range_start_day), tz);
        let label = if weeks == 1 {
            "next week".to_string()
        } else {
            format!("in {weeks} weeks")
        };
        return (label, week_sort_key, Precision::Date);
    }
    let mut months = calendar_months_between(date, now, tz);
    if months == 0 {
        months = 1;
    }
    let month_sort_key = start_of_month_ms(date, tz).max(start_of_day_ms(shift(now, 28), tz));
    let label = if months == 1 {
        "next month".to_string()
    } else {
        format!("in {months} months")
    };
    (label, month_sort_key, Precision::Date)
}

/// `formatDisplayTime` from `item.tsx`: `h:mm a` upper-cased, prefixed with
/// `MMM d` (or `MMM d, yyyy` in another year) for date-precision buckets.
pub fn format_display_time<Tz: TimeZone>(
    date: DateTime<Utc>,
    precision: Precision,
    now: DateTime<Utc>,
    tz: &Tz,
) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let local = date.with_timezone(tz);
    let time = local.format("%-I:%M %p").to_string();
    match precision {
        Precision::Time => time,
        Precision::Date => {
            let same_year = local.year() == now.with_timezone(tz).year();
            let day = if same_year {
                local.format("%b %-d").to_string()
            } else {
                local.format("%b %-d, %Y").to_string()
            };
            format!("{day}, {time}")
        }
    }
}

/// `deriveTimelineWindowData`, `collectTimelineItems`, `compareTimelineItems`
/// and `buildDateBuckets`: sessions and calendar events, newest first, grouped
/// by date. Sessions win over events that share a tracking id, and events
/// that already ended are not listed (their sessions are).
#[cfg(test)]
pub fn build<Tz: TimeZone>(
    rows: &[SessionRow],
    events: &[EventRow],
    now: DateTime<Utc>,
    tz: &Tz,
) -> Timeline {
    build_with(
        rows,
        events,
        now,
        tz,
        View {
            group_by: GroupBy::Date,
            order: SortOrder::Newest,
            show_ignored: false,
        },
        |_| false,
    )
}

/// The sidebar's view of the timeline: `groupBy`, `sortOrder`, `showIgnored`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct View {
    pub group_by: GroupBy,
    pub order: SortOrder,
    pub show_ignored: bool,
}

/// `buildTimelineBuckets` with the sidebar's grouping and ordering. Folder
/// grouping lists sessions only (no events, no "more future items") in
/// alphabetical folders with "No folder" last. `ignored` is
/// `useIgnoredEvents().isIgnored`: with `show_ignored` off, ignored events
/// neither list nor count as future items; with it on ("Show Deleted Events")
/// they list like any other event, flagged `Item::ignored`.
pub fn build_with<Tz: TimeZone>(
    rows: &[SessionRow],
    events: &[EventRow],
    now: DateTime<Utc>,
    tz: &Tz,
    view: View,
    ignored: impl Fn(&EventRow) -> bool,
) -> Timeline {
    let View {
        group_by,
        order,
        show_ignored,
    } = view;
    let events: &[EventRow] = if group_by == GroupBy::Folder {
        &[]
    } else {
        events
    };
    let tomorrow_upper_bound = start_of_day_ms(shift(now, 2), tz);
    let mut has_more_future_items = false;
    let mut items: Vec<Item> = Vec::with_capacity(rows.len() + events.len());
    let mut seen_tracking_ids: std::collections::HashSet<String> = Default::default();

    for row in rows {
        let Some(timestamp) = session_timestamp(row, tz) else {
            continue;
        };
        if timestamp.timestamp_millis() >= tomorrow_upper_bound {
            has_more_future_items = true;
            continue;
        }
        let event = SessionEvent::parse(&row.event_json);
        if let Some(tracking_id) = event
            .as_ref()
            .and_then(|event| event.tracking_id.as_deref())
            .filter(|id| !id.is_empty())
            && !seen_tracking_ids.insert(tracking_id.to_string())
        {
            continue;
        }
        let ended_at = event
            .as_ref()
            .and_then(|event| event.ended_at.as_deref())
            .and_then(|value| parse_date(value, tz));
        items.push(Item {
            kind: ItemKind::Session,
            id: row.id.clone(),
            title: row.title.clone(),
            timestamp,
            ended_at,
            locked: row.locked != 0,
            folder_id: row.folder_id.clone(),
            event,
            ignored: false,
        });
    }

    for event in events {
        let is_ignored = ignored(event);
        if is_ignored && !show_ignored {
            continue;
        }
        let started = parse_date(&event.started_at, tz);
        let ended = parse_date(&event.ended_at, tz);
        let window_time = started.or(ended);
        if window_time.is_some_and(|t| t.timestamp_millis() >= tomorrow_upper_bound) {
            has_more_future_items = true;
            continue;
        }
        if !event.tracking_id_event.is_empty()
            && seen_tracking_ids.contains(&event.tracking_id_event)
        {
            continue;
        }
        let Some(time_to_check) = ended.or(started) else {
            continue;
        };
        // `!isPast(timeToCheck)`
        if time_to_check <= now {
            continue;
        }
        let Some(timestamp) = started else {
            continue;
        };
        if !event.tracking_id_event.is_empty() {
            seen_tracking_ids.insert(event.tracking_id_event.clone());
        }
        items.push(Item {
            kind: ItemKind::Event,
            id: event.id.clone(),
            title: event.title.clone(),
            timestamp,
            ended_at: ended,
            locked: false,
            folder_id: String::new(),
            event: Some(SessionEvent {
                tracking_id: Some(event.tracking_id_event.clone()),
                started_at: Some(event.started_at.clone()),
                ended_at: Some(event.ended_at.clone()),
                meeting_link: Some(event.meeting_link.clone()),
                ..SessionEvent::default()
            }),
            ignored: is_ignored,
        });
    }

    // `compareTimelineItems`
    items.sort_by(|left, right| {
        let by_time = match order {
            SortOrder::Newest => right.timestamp.cmp(&left.timestamp),
            SortOrder::Oldest => left.timestamp.cmp(&right.timestamp),
        };
        by_time
            .then_with(|| compare_titles(display_title(&left.title), display_title(&right.title)))
    });

    if group_by == GroupBy::Folder {
        // `buildFolderBuckets`
        let mut buckets: Vec<(String, Bucket)> = Vec::new();
        for item in items {
            let key = normalize_folder_path(&item.folder_id).unwrap_or_default();
            match buckets.iter_mut().find(|(k, _)| *k == key) {
                Some((_, bucket)) => bucket.items.push(item),
                None => {
                    let label = if key.is_empty() {
                        "No folder".to_string()
                    } else {
                        key.clone()
                    };
                    buckets.push((
                        key,
                        Bucket {
                            label,
                            precision: Precision::Date,
                            items: vec![item],
                        },
                    ));
                }
            }
        }
        buckets.sort_by(
            |(left, _), (right, _)| match (left.is_empty(), right.is_empty()) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => left.cmp(right),
            },
        );
        return Timeline {
            buckets: buckets.into_iter().map(|(_, bucket)| bucket).collect(),
            has_more_future_items: false,
        };
    }

    let mut buckets: Vec<(i64, Bucket)> = Vec::new();
    for item in items {
        let (label, sort_key, precision) = bucket_info(item.timestamp, now, tz);
        match buckets.iter_mut().find(|(_, bucket)| bucket.label == label) {
            Some((_, bucket)) => bucket.items.push(item),
            None => buckets.push((
                sort_key,
                Bucket {
                    label,
                    precision,
                    items: vec![item],
                },
            )),
        }
    }
    match order {
        SortOrder::Newest => buckets.sort_by(|left, right| right.0.cmp(&left.0)),
        SortOrder::Oldest => buckets.sort_by(|left, right| left.0.cmp(&right.0)),
    }

    Timeline {
        buckets: buckets.into_iter().map(|(_, bucket)| bucket).collect(),
        has_more_future_items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Same clock as apps/desktop/src/sidebar/timeline/utils/index.test.ts.
    fn now() -> DateTime<Utc> {
        "2024-01-15T12:00:00.000Z".parse().unwrap()
    }

    fn at(value: &str) -> DateTime<Utc> {
        value.parse().unwrap()
    }

    fn row(id: &str, title: &str, created_at: &str, event_started_at: Option<&str>) -> SessionRow {
        SessionRow {
            id: id.into(),
            title: title.into(),
            created_at: created_at.into(),
            event_json: event_started_at
                .map(|s| serde_json::json!({ "started_at": s }).to_string())
                .unwrap_or_default(),
            folder_id: String::new(),
            locked: 0,
        }
    }

    #[test]
    fn folder_grouping_and_oldest_ordering_follow_build_timeline_buckets() {
        let mut a = row("a", "Alpha", "2024-01-15T05:00:00.000Z", None);
        a.folder_id = "Work/Client".into();
        let mut b = row("b", "Beta", "2024-01-14T05:00:00.000Z", None);
        b.folder_id = "Personal".into();
        let c = row("c", "Gamma", "2024-01-13T05:00:00.000Z", None);
        let event = EventRow {
            id: "e".into(),
            tracking_id_event: "t".into(),
            title: "Meeting".into(),
            started_at: "2024-01-15T13:00:00.000Z".into(),
            ended_at: "2024-01-15T14:00:00.000Z".into(),
            ..EventRow::default()
        };

        let folders = build_with(
            &[a.clone(), b.clone(), c.clone()],
            std::slice::from_ref(&event),
            now(),
            &Utc,
            View {
                group_by: GroupBy::Folder,
                order: SortOrder::Newest,
                show_ignored: false,
            },
            |_| false,
        );
        let labels: Vec<&str> = folders
            .buckets
            .iter()
            .map(|bucket| bucket.label.as_str())
            .collect();
        assert_eq!(
            labels,
            ["Personal", "Work/Client", "No folder"],
            "alphabetical with No folder last"
        );
        assert!(!folders.has_more_future_items);
        assert!(
            folders.buckets.iter().all(|bucket| bucket
                .items
                .iter()
                .all(|item| item.kind == ItemKind::Session)),
            "events are not shown when grouping by folder"
        );
        assert_eq!(folders.buckets[0].precision, Precision::Date);

        let oldest = build_with(
            &[a, b, c],
            &[],
            now(),
            &Utc,
            View {
                group_by: GroupBy::Date,
                order: SortOrder::Oldest,
                show_ignored: false,
            },
            |_| false,
        );
        let labels: Vec<&str> = oldest
            .buckets
            .iter()
            .map(|bucket| bucket.label.as_str())
            .collect();
        assert_eq!(labels, ["2 days ago", "Yesterday", "Today"]);

        // Oldest-first Today bucket: the line goes before the first item at or after now.
        let items = vec![
            Item {
                timestamp: at("2024-01-15T05:00:00.000Z"),
                ..oldest.buckets[2].items[0].clone()
            },
            Item {
                timestamp: at("2024-01-15T13:00:00.000Z"),
                ..oldest.buckets[2].items[0].clone()
            },
        ];
        assert_eq!(
            indicator_placement(&items, now(), SortOrder::Oldest),
            IndicatorPlacement::Before { index: 1 }
        );
    }

    #[test]
    fn bucket_info_matches_tauri_fixtures() {
        let (label, _, precision) = bucket_info(at("2024-01-15T05:00:00.000Z"), now(), &Utc);
        assert_eq!((label.as_str(), precision), ("Today", Precision::Time));

        let (label, _, precision) = bucket_info(at("2024-01-10T05:00:00.000Z"), now(), &Utc);
        assert_eq!((label.as_str(), precision), ("5 days ago", Precision::Time));

        let (label, _, precision) = bucket_info(at("2024-03-20T12:00:00.000Z"), now(), &Utc);
        assert_eq!(
            (label.as_str(), precision),
            ("in 2 months", Precision::Date)
        );

        assert_eq!(
            bucket_info(at("2024-01-14T23:59:00Z"), now(), &Utc).0,
            "Yesterday"
        );
        assert_eq!(
            bucket_info(at("2024-01-16T00:00:00Z"), now(), &Utc).0,
            "Tomorrow"
        );
        assert_eq!(
            bucket_info(at("2024-01-08T12:00:00Z"), now(), &Utc).0,
            "a week ago"
        );
        assert_eq!(
            bucket_info(at("2024-01-01T12:00:00Z"), now(), &Utc).0,
            "2 weeks ago"
        );
        assert_eq!(
            bucket_info(at("2024-01-22T12:00:00Z"), now(), &Utc).0,
            "next week"
        );
    }

    #[test]
    fn month_buckets_sort_beyond_week_buckets() {
        let in_4_weeks = bucket_info(at("2024-02-11T12:00:00.000Z"), now(), &Utc);
        let next_month = bucket_info(at("2024-02-13T12:00:00.000Z"), now(), &Utc);
        assert_eq!(in_4_weeks.0, "in 4 weeks");
        assert_eq!(next_month.0, "next month");
        assert!(next_month.1 > in_4_weeks.1);

        let weeks_ago_4 = bucket_info(at("2023-12-19T12:00:00.000Z"), now(), &Utc);
        let month_ago = bucket_info(at("2023-12-17T12:00:00.000Z"), now(), &Utc);
        assert_eq!(weeks_ago_4.0, "4 weeks ago");
        assert_eq!(month_ago.0, "a month ago");
        assert!(month_ago.1 < weeks_ago_4.1);
    }

    #[test]
    fn buckets_sort_most_recent_first_and_prefer_event_start() {
        let rows = [
            row(
                "session-future",
                "Future Session",
                "2024-01-10T12:00:00.000Z",
                Some("2024-01-16T09:00:00.000Z"),
            ),
            row(
                "session-past",
                "Past Session",
                "2024-01-14T09:00:00.000Z",
                None,
            ),
        ];
        let timeline = build(&rows, &[], now(), &Utc);
        let labels: Vec<&str> = timeline.buckets.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(labels, ["Tomorrow", "Yesterday"]);
        assert!(!timeline.has_more_future_items);
    }

    #[test]
    fn sessions_after_tomorrow_are_hidden_and_flagged() {
        let rows = [
            row(
                "tomorrow",
                "Tomorrow late",
                "2024-01-16T23:59:59.000Z",
                None,
            ),
            row("later", "Day after", "2024-01-17T00:00:00.000Z", None),
            row("unparseable", "Broken", "not a date", None),
        ];
        let timeline = build(&rows, &[], now(), &Utc);
        let ids: Vec<&str> = timeline
            .buckets
            .iter()
            .flat_map(|b| b.items.iter().map(|i| i.id.as_str()))
            .collect();
        assert_eq!(ids, ["tomorrow"]);
        assert!(timeline.has_more_future_items);
    }

    #[test]
    fn same_timestamp_ties_break_on_title_with_untitled_fallback() {
        let rows = [
            row("b", "beta", "2024-01-15T09:00:00.000Z", None),
            row("u", "", "2024-01-15T09:00:00.000Z", None),
            row("a", "Alpha", "2024-01-15T09:00:00.000Z", None),
            row("newer", "Zed", "2024-01-15T10:00:00.000Z", None),
        ];
        let timeline = build(&rows, &[], now(), &Utc);
        let ids: Vec<&str> = timeline.buckets[0]
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect();
        assert_eq!(ids, ["newer", "a", "b", "u"]);
    }

    #[test]
    fn indicator_sits_between_future_and_past_or_inside_an_active_item() {
        let rows = [
            row("future", "Future", "2024-01-15T15:00:00Z", None),
            row("past", "Past", "2024-01-15T09:00:00Z", None),
        ];
        let items = build(&rows, &[], now(), &Utc).buckets.remove(0).items;
        assert_eq!(
            indicator_placement(&items, now(), SortOrder::Newest),
            IndicatorPlacement::Before { index: 1 }
        );
        assert_eq!(
            indicator_placement(&items[..1], now(), SortOrder::Newest),
            IndicatorPlacement::After
        );

        let mut active = row(
            "active",
            "Active",
            "2024-01-15T09:00:00Z",
            Some("2024-01-15T11:00:00Z"),
        );
        active.event_json = serde_json::json!({
            "started_at": "2024-01-15T11:00:00Z",
            "ended_at": "2024-01-15T13:00:00Z",
            "meeting_link": "https://zoom.us/j/1"
        })
        .to_string();
        let items = build(&[active], &[], now(), &Utc).buckets.remove(0).items;
        assert_eq!(
            indicator_placement(&items, now(), SortOrder::Newest),
            IndicatorPlacement::Inside {
                index: 0,
                progress: 0.5
            }
        );
        assert_eq!(
            items[0].event.as_ref().unwrap().remote_meeting(),
            Some(RemoteMeeting::Zoom)
        );
    }

    #[test]
    fn empty_event_started_at_drops_the_row_like_the_app() {
        let mut empty = row("e", "Empty start", "2024-01-15T09:00:00Z", None);
        empty.event_json = serde_json::json!({ "started_at": "" }).to_string();
        let no_key = row("k", "No key", "2024-01-15T09:00:00Z", None);
        let mut no_key = no_key;
        no_key.event_json = serde_json::json!({ "tracking_id": "x" }).to_string();
        let timeline = build(&[empty, no_key], &[], now(), &Utc);
        let ids: Vec<&str> = timeline
            .buckets
            .iter()
            .flat_map(|b| b.items.iter().map(|i| i.id.as_str()))
            .collect();
        assert_eq!(ids, ["k"]);
    }

    fn event(id: &str, title: &str, started: &str, ended: &str, tracking: &str) -> EventRow {
        EventRow {
            id: id.into(),
            title: title.into(),
            started_at: started.into(),
            ended_at: ended.into(),
            tracking_id_event: tracking.into(),
            ..EventRow::default()
        }
    }

    #[test]
    fn events_merge_after_sessions_skip_past_and_dedupe_by_tracking_id() {
        let mut backed = row("s-backed", "Backed", "2024-01-15T08:00:00Z", None);
        backed.event_json = serde_json::json!({
            "tracking_id": "track-a",
            "started_at": "2024-01-15T14:00:00Z"
        })
        .to_string();
        let rows = [backed];
        let events = [
            event(
                "e-dup",
                "Dup of backed",
                "2024-01-15T14:00:00Z",
                "2024-01-15T15:00:00Z",
                "track-a",
            ),
            event(
                "e-past",
                "Already over",
                "2024-01-15T09:00:00Z",
                "2024-01-15T10:00:00Z",
                "track-b",
            ),
            event(
                "e-live",
                "Running now",
                "2024-01-15T11:00:00Z",
                "2024-01-15T13:00:00Z",
                "track-c",
            ),
            event(
                "e-later",
                "Tomorrow",
                "2024-01-16T09:00:00Z",
                "2024-01-16T10:00:00Z",
                "track-d",
            ),
            event(
                "e-far",
                "Next week",
                "2024-01-22T09:00:00Z",
                "2024-01-22T10:00:00Z",
                "track-e",
            ),
            event(
                "e-recur-1",
                "Weekly",
                "2024-01-16T12:00:00Z",
                "2024-01-16T13:00:00Z",
                "track-f",
            ),
            event(
                "e-recur-2",
                "Weekly",
                "2024-01-16T12:00:00Z",
                "2024-01-16T13:00:00Z",
                "track-f",
            ),
        ];
        let timeline = build(&rows, &events, now(), &Utc);
        let ids: Vec<(&str, ItemKind)> = timeline
            .buckets
            .iter()
            .flat_map(|b| b.items.iter().map(|i| (i.id.as_str(), i.kind)))
            .collect();
        assert_eq!(
            ids,
            [
                ("e-recur-1", ItemKind::Event),
                ("e-later", ItemKind::Event),
                ("s-backed", ItemKind::Session),
                ("e-live", ItemKind::Event),
            ]
        );
        assert!(timeline.has_more_future_items);
    }

    #[test]
    fn ignored_events_neither_list_nor_count_as_future_items() {
        // `deriveTimelineWindowData` with `showIgnored` off: an ignored event
        // after tomorrow does not set `hasMoreFutureItems`, and an ignored
        // upcoming event leaves the buckets.
        let events = [
            event(
                "e-live",
                "Live",
                "2024-01-15T14:30:00Z",
                "2024-01-15T15:00:00Z",
                "track-live",
            ),
            event(
                "e-far",
                "Far",
                "2024-02-01T10:00:00Z",
                "2024-02-01T11:00:00Z",
                "track-far",
            ),
        ];
        let all = build(&[], &events, now(), &Utc);
        assert_eq!(all.buckets.iter().flat_map(|b| b.items.iter()).count(), 1);
        assert!(all.has_more_future_items);
        let filtered = build_with(
            &[],
            &events,
            now(),
            &Utc,
            View {
                group_by: GroupBy::Date,
                order: SortOrder::Newest,
                show_ignored: false,
            },
            |event| {
                event.tracking_id_event == "track-far" || event.tracking_id_event == "track-live"
            },
        );
        assert_eq!(
            filtered.buckets.iter().flat_map(|b| b.items.iter()).count(),
            0
        );
        assert!(!filtered.has_more_future_items);

        // "Show Deleted Events": ignored events list again, flagged, and the
        // far one counts as a future item like in `deriveTimelineWindowData`.
        let shown = build_with(
            &[],
            &events,
            now(),
            &Utc,
            View {
                group_by: GroupBy::Date,
                order: SortOrder::Newest,
                show_ignored: true,
            },
            |event| {
                event.tracking_id_event == "track-far" || event.tracking_id_event == "track-live"
            },
        );
        let items: Vec<&Item> = shown.buckets.iter().flat_map(|b| b.items.iter()).collect();
        assert_eq!(items.len(), 1);
        assert!(items[0].ignored);
        assert!(shown.has_more_future_items);
    }

    #[test]
    fn folder_paths_normalize_like_the_app() {
        assert_eq!(
            normalize_folder_path("  Work\\Q3/  "),
            Some("Work/Q3".into())
        );
        assert_eq!(normalize_folder_path("Work/"), Some("Work".into()));
        assert_eq!(normalize_folder_path(""), Some(String::new()));
        assert_eq!(normalize_folder_path("/abs"), None);
        assert_eq!(normalize_folder_path("a//b"), None);
        assert_eq!(normalize_folder_path("a/../b"), None);
        assert_eq!(folder_label(""), None);
        assert_eq!(folder_label("Work"), Some("Work".into()));
    }

    #[test]
    fn display_time_follows_bucket_precision() {
        let date = at("2024-01-15T09:05:00.000Z");
        assert_eq!(
            format_display_time(date, Precision::Time, now(), &Utc),
            "9:05 AM"
        );
        assert_eq!(
            format_display_time(date, Precision::Date, now(), &Utc),
            "Jan 15, 9:05 AM"
        );
        assert_eq!(
            format_display_time(at("2023-12-24T18:30:00Z"), Precision::Date, now(), &Utc),
            "Dec 24, 2023, 6:30 PM"
        );
    }

    #[test]
    fn parse_date_accepts_the_stored_shapes() {
        assert!(parse_date("2024-01-15T09:05:00.123Z", &Utc).is_some());
        assert!(parse_date("2024-01-15T09:05:00+09:00", &Utc).is_some());
        assert_eq!(
            parse_date("2024-01-15", &Utc),
            Some(at("2024-01-15T00:00:00Z"))
        );
        assert_eq!(
            parse_date("2024-01-15T09:05:00", &Utc),
            Some(at("2024-01-15T09:05:00Z"))
        );
        assert_eq!(parse_date("", &Utc), None);
        assert_eq!(parse_date("yesterday", &Utc), None);
    }
}
