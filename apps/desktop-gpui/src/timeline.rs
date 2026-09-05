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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Time,
    Date,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub timestamp: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub locked: bool,
    pub folder_id: String,
    pub event: Option<SessionEvent>,
}

/// The parts of `sessions.event_json` the main window reads. Fields stay
/// `Option` because the app's `??` fallbacks only fire for missing keys, not
/// for empty strings.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct SessionEvent {
    pub tracking_id: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub meeting_link: Option<String>,
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

pub fn indicator_placement(items: &[Item], now: DateTime<Utc>) -> IndicatorPlacement {
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
    match items
        .iter()
        .position(|item| item.timestamp.timestamp_millis() < now_ms)
    {
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

/// `deriveTimelineWindowData` + `collectTimelineItems` + `compareTimelineItems`
/// + `buildDateBuckets` for sessions, newest first, grouped by date.
pub fn build<Tz: TimeZone>(rows: &[SessionRow], now: DateTime<Utc>, tz: &Tz) -> Timeline {
    let tomorrow_upper_bound = start_of_day_ms(shift(now, 2), tz);
    let mut has_more_future_items = false;
    let mut items: Vec<Item> = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(timestamp) = session_timestamp(row, tz) else {
            continue;
        };
        if timestamp.timestamp_millis() >= tomorrow_upper_bound {
            has_more_future_items = true;
            continue;
        }
        let event = SessionEvent::parse(&row.event_json);
        let ended_at = event
            .as_ref()
            .and_then(|event| event.ended_at.as_deref())
            .and_then(|value| parse_date(value, tz));
        items.push(Item {
            id: row.id.clone(),
            title: row.title.clone(),
            timestamp,
            ended_at,
            locked: row.locked != 0,
            folder_id: row.folder_id.clone(),
            event,
        });
    }

    items.sort_by(|left, right| {
        right
            .timestamp
            .cmp(&left.timestamp)
            .then_with(|| compare_titles(display_title(&left.title), display_title(&right.title)))
    });

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
    buckets.sort_by(|left, right| right.0.cmp(&left.0));

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
        let timeline = build(&rows, now(), &Utc);
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
        let timeline = build(&rows, now(), &Utc);
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
        let timeline = build(&rows, now(), &Utc);
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
        let items = build(&rows, now(), &Utc).buckets.remove(0).items;
        assert_eq!(
            indicator_placement(&items, now()),
            IndicatorPlacement::Before { index: 1 }
        );
        assert_eq!(
            indicator_placement(&items[..1], now()),
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
        let items = build(&[active], now(), &Utc).buckets.remove(0).items;
        assert_eq!(
            indicator_placement(&items, now()),
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
        let timeline = build(&[empty, no_key], now(), &Utc);
        let ids: Vec<&str> = timeline
            .buckets
            .iter()
            .flat_map(|b| b.items.iter().map(|i| i.id.as_str()))
            .collect();
        assert_eq!(ids, ["k"]);
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
