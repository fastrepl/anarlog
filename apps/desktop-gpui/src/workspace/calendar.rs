use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::{DateTime, Datelike, Days, Local, Months, NaiveDate, NaiveDateTime, Utc};
use chrono_tz::Tz;
use desktop_runtime::{
    CancellationToken, Generation, Reply, Result, RuntimeHandle, ServiceError, SessionId,
};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    Animation, AnimationExt, App, Context, Entity, EventEmitter, FocusHandle, FontWeight, Hsla,
    KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point, Render, ScrollEvent, ScrollHandle,
    SharedString, Subscription, Task, Transformation, Window, anchored, canvas, deferred, div,
    percentage, point, prelude::*, px, rgb, svg,
};
use serde_json::{Value, json};

use super::{
    calendar_accounts::{CalendarSync, SyncStatus},
    calendar_rules::{self, RuleChange, SeriesRules},
    mutations::{failure, statement, transaction},
};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::{Theme, monospace_font, theme},
};

const COMPACT_PAST_DAYS: u64 = 42;
const COMPACT_FUTURE_DAYS: u64 = 42;
const MAX_ITEMS: usize = 1000;
const HEADER_HEIGHT: f32 = 48.;
const WEEKDAY_HEIGHT: f32 = 33.;
// Cell border (1) + vertical padding (2 × 6) + day badge (28) + its bottom margin (4).
const CELL_CHROME: f32 = 45.;
const CHIP_HEIGHT: f32 = 15.;
const ALL_DAY_CHIP_HEIGHT: f32 = 19.;
const CHIP_GAP: f32 = 2.;
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub fn visible_columns(width: f32) -> usize {
    if width >= 700. {
        7
    } else if width >= 400. {
        4
    } else if width >= 200. {
        2
    } else {
        1
    }
}

/// Regions whose week does not start on Monday (CLDR `firstDay` sun, sat or fri).
const NON_MONDAY_REGIONS: &[&str] = &[
    "AE", "AF", "AG", "AS", "BD", "BH", "BR", "BS", "BT", "BW", "BZ", "CA", "CN", "CO", "DJ", "DM",
    "DO", "DZ", "EG", "ET", "GT", "GU", "HK", "HN", "ID", "IL", "IN", "IQ", "IR", "JM", "JO", "JP",
    "KE", "KH", "KR", "KW", "LA", "LY", "MH", "MM", "MO", "MT", "MV", "MX", "MZ", "NI", "NP", "OM",
    "PA", "PE", "PH", "PK", "PR", "PT", "PY", "QA", "SA", "SD", "SG", "SV", "SY", "TH", "TT", "TW",
    "UM", "US", "VE", "VI", "WS", "YE", "ZA", "ZW",
];

/// The first weekday (0 = Sunday, 1 = Monday) for the `week_start` setting, resolved like
/// shipping's `useWeekStartsOn`: an explicit choice wins; otherwise Monday only where the
/// system locale's week starts on Monday.
pub fn week_start(preference: &str, locale: Option<&str>) -> u8 {
    match preference {
        "monday" => 1,
        "sunday" => 0,
        _ => {
            let region = locale.and_then(|locale| {
                locale.split(['-', '_']).skip(1).find(|part| {
                    part.len() == 2 && part.chars().all(|char| char.is_ascii_alphabetic())
                })
            });
            region.map_or(0, |region| {
                u8::from(
                    !NON_MONDAY_REGIONS
                        .iter()
                        .any(|other| other.eq_ignore_ascii_case(region)),
                )
            })
        }
    }
}

/// The days shown for `month`: whole weeks from the one containing the 1st to the one
/// containing the last day.
pub fn month_days(month: NaiveDate, week_start: u8) -> Vec<NaiveDate> {
    let week_start = u32::from(week_start.min(6));
    let first = month.with_day(1).unwrap_or(month);
    let last = first
        .checked_add_months(Months::new(1))
        .and_then(|next| next.pred_opt())
        .unwrap_or(first);
    let lead = (first.weekday().num_days_from_sunday() + 7 - week_start) % 7;
    let trail = (week_start + 6 - last.weekday().num_days_from_sunday()) % 7;
    let start = first - Days::new(lead.into());
    let end = last + Days::new(trail.into());
    start.iter_days().take_while(|day| *day <= end).collect()
}

/// The scrollable strip shown when the calendar is too narrow for a month grid.
pub fn compact_days(start: NaiveDate) -> Vec<NaiveDate> {
    let first = start - Days::new(COMPACT_PAST_DAYS);
    first
        .iter_days()
        .take((COMPACT_PAST_DAYS + COMPACT_FUTURE_DAYS) as usize)
        .collect()
}

fn weekend(day: NaiveDate) -> bool {
    matches!(day.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
}

/// How many chips fit in a day cell, reserving a row for "+N more" when some are hidden.
pub fn visible_count(total: usize, chip: f32, available: Option<f32>) -> usize {
    let Some(available) = available else {
        return total;
    };
    let all = total as f32 * chip + total.saturating_sub(1) as f32 * CHIP_GAP;
    if all <= available {
        return total;
    }
    let (mut count, mut used) = (0, 0.);
    while count < total {
        let next = chip + if count > 0 { CHIP_GAP } else { 0. };
        let more = if total - count - 1 > 0 {
            chip + CHIP_GAP
        } else {
            0.
        };
        if used + next + more > available {
            break;
        }
        used += next;
        count += 1;
    }
    count.max(1).min(total)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    pub use_24_hour_time: bool,
    pub timezone: Option<Tz>,
}

impl Clock {
    fn local(&self, instant: DateTime<Utc>) -> NaiveDateTime {
        match self.timezone {
            Some(timezone) => instant.with_timezone(&timezone).naive_local(),
            None => instant.with_timezone(&Local).naive_local(),
        }
    }

    fn today(&self) -> NaiveDate {
        self.local(Utc::now()).date()
    }

    fn time(&self, at: NaiveDateTime) -> String {
        at.format(if self.use_24_hour_time {
            "%H:%M"
        } else {
            "%-I:%M %p"
        })
        .to_string()
    }

    fn date_time(&self, at: NaiveDateTime) -> String {
        format!("{} {}", at.format("%b %-d, %Y"), self.time(at))
    }
}

/// Stored timestamps are RFC 3339; zone-less values are UTC, as in the shipping app.
fn parse_instant(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.to_utc())
        .ok()
        .or_else(|| {
            ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"]
                .iter()
                .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
                .map(|date| date.and_utc())
        })
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| date.and_utc())
        })
}

pub(super) fn parse_color(value: &str) -> Option<Hsla> {
    let hex = value.trim().strip_prefix('#')?;
    let expanded = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 | 8 => hex[..6].to_owned(),
        _ => return None,
    };
    u32::from_str_radix(&expanded, 16)
        .ok()
        .map(|value| rgb(value).into())
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarItem {
    pub id: Arc<str>,
    /// A note without a calendar event, shown with an outlined marker.
    pub note: bool,
    pub title: Arc<str>,
    pub start: Arc<str>,
    pub end: Arc<str>,
    pub all_day: bool,
    pub color: Arc<str>,
    pub location: Arc<str>,
    pub meeting_link: Arc<str>,
    pub description: Arc<str>,
    pub session: Option<SessionId>,
    /// Folder of the note itself, or of the note linked to the event.
    pub folder: Arc<str>,
    /// Recurring series of an event; empty for one-off events and notes.
    pub series: Arc<str>,
}

impl CalendarItem {
    fn day(&self, clock: &Clock) -> Option<NaiveDate> {
        if self.all_day {
            NaiveDate::parse_from_str(self.start.trim().get(..10)?, "%Y-%m-%d").ok()
        } else {
            parse_instant(&self.start).map(|at| clock.local(at).date())
        }
    }

    fn title(&self) -> SharedString {
        if self.title.trim().is_empty() {
            "Busy".into()
        } else {
            SharedString::from(self.title.clone())
        }
    }

    fn chip_height(&self) -> f32 {
        if self.all_day && !self.note {
            ALL_DAY_CHIP_HEIGHT
        } else {
            CHIP_HEIGHT
        }
    }
}

/// Places items on their start day and orders each day like the shipping calendar:
/// events (all-day first, then by start), then notes by creation time.
pub fn by_day(items: &[CalendarItem], clock: &Clock) -> HashMap<NaiveDate, Vec<CalendarItem>> {
    let mut days = HashMap::<NaiveDate, Vec<CalendarItem>>::new();
    for item in items {
        if let Some(day) = item.day(clock) {
            days.entry(day).or_default().push(item.clone());
        }
    }
    for items in days.values_mut() {
        items.sort_by_cached_key(|item| {
            let start = parse_instant(&item.start);
            (
                item.note,
                !item.all_day,
                start.is_none(),
                start,
                item.title.clone(),
                item.id.clone(),
            )
        });
    }
    days
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalendarPage {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub items: Arc<[CalendarItem]>,
    pub overflow: bool,
    pub rules: Arc<SeriesRules>,
}

fn field(row: &Value, key: &str) -> Arc<str> {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}

const ITEM_QUERY: &str = "SELECT 0 AS note, e.id, substr(e.title,1,4096) AS title, e.started_at AS start,
        e.ended_at AS end, e.is_all_day AS all_day, COALESCE(c.color,'') AS color,
        substr(e.location,1,4096) AS location, substr(e.meeting_link,1,4096) AS meeting_link,
        substr(e.description,1,16384) AS description,
        (SELECT s.id FROM sessions s WHERE s.event_id = e.id AND s.deleted_at IS NULL ORDER BY s.created_at,s.id LIMIT 1) AS session_id,
        COALESCE((SELECT s.folder_path FROM sessions s WHERE s.event_id = e.id AND s.deleted_at IS NULL ORDER BY s.created_at,s.id LIMIT 1),'') AS folder,
        e.recurrence_series_id AS series
    FROM events e LEFT JOIN calendars c ON c.id = e.calendar_id AND c.deleted_at IS NULL
    WHERE e.deleted_at IS NULL AND e.started_at >= ?1 AND e.started_at < ?2
        AND NOT EXISTS(
            SELECT 1 FROM app_settings a,
                json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
            WHERE (a.id = 'ignored_events' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
                OR (a.id = 'ignored_recurring_series' AND json_extract(ignored.value,'$.id') = e.recurrence_series_id)
        )
    UNION ALL
    SELECT 1, s.id, substr(s.title,1,4096), s.created_at, '', 0, '', '', '', '', s.id, s.folder_path, ''
    FROM sessions s
    WHERE s.deleted_at IS NULL AND s.title <> '' AND s.event_json = ''
        AND NOT EXISTS(SELECT 1 FROM events e WHERE e.id = s.event_id AND e.deleted_at IS NULL)
        AND s.created_at >= ?1 AND s.created_at < ?2
    ORDER BY start, id LIMIT 1001";

/// Stored timestamps are UTC, so the query is padded by a day on each side and the
/// exact local day is decided in `by_day`.
fn query_bounds(from: NaiveDate, to: NaiveDate) -> [Value; 2] {
    [
        json!((from - Days::new(1)).to_string()),
        json!((to + Days::new(2)).to_string()),
    ]
}

pub fn load_calendar(
    runtime: &RuntimeHandle,
    from: NaiveDate,
    to: NaiveDate,
    cancel: CancellationToken,
) -> Result<Reply<CalendarPage>> {
    runtime.read(cancel, move |services| async move {
        let rows = services
            .executor
            .execute(ITEM_QUERY.into(), query_bounds(from, to).to_vec())
            .await
            .map_err(failure)?;
        let rules = services
            .executor
            .execute(calendar_rules::QUERY.into(), vec![])
            .await
            .map_err(failure)?;
        let items = rows
            .iter()
            .take(MAX_ITEMS)
            .map(|row| CalendarItem {
                id: field(row, "id"),
                note: row["note"] == 1,
                title: field(row, "title"),
                start: field(row, "start"),
                end: field(row, "end"),
                all_day: row["all_day"] == 1,
                color: field(row, "color"),
                location: field(row, "location"),
                meeting_link: field(row, "meeting_link"),
                description: field(row, "description"),
                session: row["session_id"].as_str().map(|id| SessionId(id.into())),
                folder: field(row, "folder"),
                series: field(row, "series"),
            })
            .collect();
        Ok(CalendarPage {
            from,
            to,
            items,
            overflow: rows.len() > MAX_ITEMS,
            rules: Arc::new(SeriesRules::from_rows(&rules)),
        })
    })
}

pub struct CalendarOpen(pub SessionId);

pub fn open_event(
    runtime: &RuntimeHandle,
    event_id: Arc<str>,
    viewer: Option<Arc<str>>,
) -> Result<Reply<SessionId>> {
    runtime.submit(move |services| async move {
        let executor = &services.executor;
        let rows = executor.execute("SELECT * FROM events WHERE id=? AND deleted_at IS NULL".into(),vec![json!(event_id)]).await.map_err(failure)?;
        let event = rows.first().ok_or(ServiceError::Conflict)?;
        let participants: Value = serde_json::from_str(event["participants_json"].as_str().unwrap_or("[]")).map_err(failure)?;
        let participants = participants.as_array().ok_or_else(|| failure("Invalid event participants"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let tracking = event["tracking_id_event"].as_str().unwrap_or("");
        let provider=event["provider"].as_str().unwrap_or("");
        let event_json = json!({
            "tracking_id":tracking,"calendar_id":event["calendar_id"],"title":event["title"],
            "started_at":event["started_at"],"ended_at":event["ended_at"],"is_all_day":event["is_all_day"]==1,
            "has_recurrence_rules":event["has_recurrence_rules"]==1,"location":event["location"],
            "meeting_link":event["meeting_link"],"description":event["description"],"recurrence_series_id":event["recurrence_series_id"]
        });
        transaction(executor,vec![
            statement("INSERT INTO sessions(id,workspace_id,owner_user_id,title,started_at,ended_at,event_id,external_event_id,external_provider,series_id,event_json)
                SELECT ?1,NULLIF((SELECT json_extract(value_json,'$.workspace_id') FROM app_settings WHERE id='cloudsync_workspace_binding'),''),
                    COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active=1),NULLIF(NULLIF(?6,''),'00000000-0000-0000-0000-000000000000'),NULLIF((SELECT json_extract(value_json,'$.workspace_id') FROM app_settings WHERE id='cloudsync_workspace_binding'),'')),
                    title,started_at,ended_at,id,tracking_id_event,provider,recurrence_series_id,?2
                FROM events WHERE id=?3 AND deleted_at IS NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE deleted_at IS NULL AND (event_id=?3 OR (?4<>'' AND external_event_id=?4 AND external_provider=?5)))".into(),
                vec![json!(id),json!(event_json.to_string()),json!(event_id),json!(tracking),json!(provider),json!(viewer)],None),
            statement("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body,created_by,updated_by) SELECT id,workspace_id,id,'note','prosemirror_json','{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',owner_user_id,owner_user_id FROM sessions WHERE id=?".into(),vec![json!(id)],None),
        ]).await?;
        let rows = executor.execute("SELECT id FROM sessions WHERE deleted_at IS NULL AND (event_id=?1 OR (?2<>'' AND external_event_id=?2 AND external_provider=?3)) ORDER BY created_at,id LIMIT 1".into(),vec![json!(event_id),json!(tracking),json!(provider)]).await.map_err(failure)?;
        let created = id.clone();
        let id = rows.first().and_then(|row| row["id"].as_str()).ok_or(ServiceError::Conflict)?.to_owned();
        let mut statements = Vec::new();
        // A new note of a recurring meeting goes into the series' auto-add folder, if it exists.
        let series = event["recurrence_series_id"].as_str().unwrap_or("");
        if id == created && !series.is_empty() {
            let rules = executor.execute(calendar_rules::QUERY.into(), vec![]).await.map_err(failure)?;
            if let Some(folder) = SeriesRules::from_rows(&rules).folder(series) {
                statements.push(statement("UPDATE sessions SET folder_path=?1, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?2 AND EXISTS(SELECT 1 FROM folders WHERE path=?1 AND deleted_at IS NULL)".into(), vec![json!(folder), json!(id)], None));
            }
        }
        let mut emails = std::collections::HashSet::new();
        for person in participants {
            if person["is_current_user"]==true { continue; }
            let email = person["email"].as_str().unwrap_or("").trim().to_lowercase();
            if email.is_empty() || !emails.insert(email.clone()) { continue; }
            let human = uuid::Uuid::new_v4().to_string();
            let name = person["name"].as_str().filter(|name| !name.is_empty()).unwrap_or(&email);
            statements.push(statement("INSERT INTO humans(id,workspace_id,owner_user_id,name,email) SELECT ?1,workspace_id,owner_user_id,?2,?3 FROM sessions WHERE id=?4 AND NOT EXISTS(SELECT 1 FROM humans WHERE lower(email)=?3 AND deleted_at IS NULL)".into(),vec![json!(human),json!(name),json!(email),json!(id)],None));
            statements.push(statement("INSERT INTO session_participants(id,workspace_id,owner_user_id,session_id,human_id,display_name,email,source)
                SELECT ?1,s.workspace_id,s.owner_user_id,s.id,h.id,?2,?3,'auto' FROM sessions s JOIN humans h ON lower(h.email)=?3 AND h.deleted_at IS NULL
                WHERE s.id=?4 AND h.id IS NOT s.owner_user_id
                AND NOT EXISTS(SELECT 1 FROM humans owner WHERE owner.id=s.owner_user_id AND lower(owner.email)=?3 AND owner.deleted_at IS NULL)
                AND NOT EXISTS(SELECT 1 FROM session_participants p WHERE p.session_id=s.id AND p.deleted_at IS NULL AND (p.human_id=h.id OR lower(p.email)=?3))
                ORDER BY h.id LIMIT 1".into(),vec![json!(uuid::Uuid::new_v4().to_string()),json!(name),json!(email),json!(id)],None));
        }
        transaction(executor,statements).await?;
        Ok(id.into())
    })
}

#[derive(Clone, Debug, PartialEq)]
enum Popover {
    Item(NaiveDate, Arc<str>),
    More(NaiveDate),
}

impl Popover {
    fn day(&self) -> NaiveDate {
        match self {
            Self::Item(day, _) | Self::More(day) => *day,
        }
    }
}

pub struct CalendarView {
    runtime: RuntimeHandle,
    clock: Clock,
    today: NaiveDate,
    week_start: u8,
    columns: usize,
    /// First day of the month shown in the month grid.
    month: NaiveDate,
    /// The day the compact strip is centred on; the strip spans six weeks either side.
    compact_start: NaiveDate,
    compact_scroll: ScrollHandle,
    compact_recentre: bool,
    width: f32,
    body_height: Option<f32>,
    page: Option<Arc<CalendarPage>>,
    days: Arc<HashMap<NaiveDate, Vec<CalendarItem>>>,
    cancel: CancellationToken,
    generation: Generation,
    watch_cancel: CancellationToken,
    watch_range: Option<(NaiveDate, NaiveDate)>,
    popover: Option<Popover>,
    dismissed: Option<Popover>,
    opening: bool,
    panel: Panel,
    /// Set when the folder picker closes, so a changed folder can offer auto-add (shipping
    /// `pickerClosed` and `folderAtPickerOpen`).
    picker_closed: bool,
    folder_at_open: Arc<str>,
    /// A note's right-click menu, at the pointer.
    menu: Option<(Point<Pixels>, SessionId)>,
    focus: FocusHandle,
    sync: Entity<CalendarSync>,
    snap: Option<Task<()>>,
    folders: Option<Arc<FolderList>>,
    folder_search: Entity<TextInput>,
    filing: bool,
    refreshing: Generation,
    showing_refresh: bool,
    message: String,
    pub(super) viewer: Option<Arc<str>>,
    _search: Subscription,
    _sync: Subscription,
}

impl EventEmitter<CalendarOpen> for CalendarView {}
impl EventEmitter<super::library::OpenNote> for CalendarView {}

/// What the event popover shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Main,
    /// The folder picker, for this note or (`rule`) for the series' future notes.
    Folders {
        rule: bool,
    },
    /// The series' auto-add folder and its actions.
    Manage,
}

/// Requests the folders route ("See all folders").
pub struct CalendarFolders;
impl EventEmitter<CalendarFolders> for CalendarView {}

/// Folder paths the shipping picker offers: explicit folders plus folders that only exist
/// on notes or attachments. `known` marks rows in `folders`.
const FOLDER_QUERY: &str = "SELECT path, 1 AS known FROM folders WHERE deleted_at IS NULL AND path <> ''
    UNION SELECT folder_path, 0 FROM sessions WHERE deleted_at IS NULL AND folder_path <> ''
    UNION SELECT folder_path, 0 FROM folder_attachments WHERE deleted_at IS NULL AND folder_path <> ''
    LIMIT 2000";

#[derive(Debug, Default, PartialEq)]
pub struct FolderList {
    pub paths: Vec<Arc<str>>,
    known: std::collections::HashSet<Arc<str>>,
}

impl FolderList {
    /// Normalizes paths, adds their ancestors and sorts them, like the shipping picker.
    pub fn new(rows: impl IntoIterator<Item = (String, bool)>) -> Self {
        let mut paths = std::collections::BTreeSet::<Arc<str>>::new();
        let mut known = std::collections::HashSet::new();
        for (path, explicit) in rows {
            let Ok(path) = anlg_fs_sync_core::normalize_folder_path(&path) else {
                continue;
            };
            if path.is_empty() {
                continue;
            }
            if explicit {
                known.insert(Arc::from(path.as_str()));
            }
            let mut prefix = String::new();
            for segment in path.split('/') {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(segment);
                paths.insert(prefix.as_str().into());
            }
        }
        Self {
            paths: paths.into_iter().collect(),
            known,
        }
    }

    fn known(&self, path: &str) -> bool {
        self.known.contains(path)
    }
}

fn folder_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

impl CalendarView {
    pub fn new(runtime: RuntimeHandle, sync: Entity<CalendarSync>, cx: &mut Context<Self>) -> Self {
        let clock = Clock::default();
        let today = clock.today();
        let folder_search = cx.new(|cx| TextInput::new("Search or create folder", cx).inline());
        let search = cx.subscribe(&folder_search, |_, _, _: &InputEvent, cx| cx.notify());
        let sync_subscription = cx.observe(&sync, |_, _, cx| cx.notify());
        Self {
            runtime,
            clock,
            today,
            week_start: 0,
            columns: 7,
            month: today.with_day(1).unwrap_or(today),
            compact_start: today,
            compact_scroll: ScrollHandle::new(),
            compact_recentre: true,
            width: 0.,
            body_height: None,
            page: None,
            days: Arc::default(),
            cancel: CancellationToken::new(),
            generation: Generation::default(),
            watch_cancel: CancellationToken::new(),
            watch_range: None,
            popover: None,
            dismissed: None,
            opening: false,
            panel: Panel::Main,
            picker_closed: false,
            folder_at_open: "".into(),
            menu: None,
            focus: cx.focus_handle(),
            sync,
            snap: None,
            folders: None,
            folder_search,
            filing: false,
            refreshing: Generation::default(),
            showing_refresh: false,
            message: String::new(),
            viewer: None,
            _search: search,
            _sync: sync_subscription,
        }
    }

    pub fn set_week_start(&mut self, week_start: u8, cx: &mut Context<Self>) {
        if self.week_start != week_start.min(6) {
            self.week_start = week_start.min(6);
            if self.watch_range.is_some() {
                self.load(cx);
            }
        }
    }

    pub fn set_clock(&mut self, clock: Clock, cx: &mut Context<Self>) {
        if self.clock != clock {
            self.clock = clock;
            if let Some(page) = &self.page {
                self.days = Arc::new(by_day(&page.items, &self.clock));
            }
            cx.notify();
        }
    }

    pub fn suspend(&mut self) {
        self.cancel.cancel();
        self.generation.advance();
        self.watch_cancel.cancel();
        self.watch_range = None;
        self.popover = None;
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.load(cx);
    }

    fn visible_days(&self) -> Vec<NaiveDate> {
        if self.columns == 7 {
            month_days(self.month, self.week_start)
        } else {
            compact_days(self.compact_start)
        }
    }

    fn watch_range(&mut self, from: NaiveDate, to: NaiveDate, cx: &mut Context<Self>) {
        if self.watch_range == Some((from, to)) {
            return;
        }
        self.watch_range = Some((from, to));
        self.watch_cancel.cancel();
        self.watch_cancel = CancellationToken::new();
        let cancel = self.watch_cancel.clone();
        let reply = self
            .runtime
            .watch_query(ITEM_QUERY.into(), query_bounds(from, to).to_vec());
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if !cancel.is_cancelled() {
                            this.message = format!(
                                "Calendar watch failed: {error}. Use refresh for stored changes."
                            );
                            this.watch_range = None;
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                watch.snapshots.borrow_and_update();
                if cancel.is_cancelled() {
                    break;
                }
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if this
                    .update(cx, |this, cx| {
                        if let Some(error) = error {
                            this.message =
                                format!("Calendar watch failed: {error}. Previous dates retained.");
                            cx.notify();
                        } else {
                            this.refresh(cx);
                        }
                    })
                    .is_err()
                    || watch.terminal_error().is_some()
                {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.refresh(cx);
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let days = self.visible_days();
        let (Some(&from), Some(&to)) = (days.first(), days.last()) else {
            return;
        };
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = load_calendar(&self.runtime, from, to, self.cancel.clone());
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        this.watch_range(page.from, page.to, cx);
                        this.message = if page.overflow {
                            "Only the first 1,000 items in this range are shown.".into()
                        } else {
                            String::new()
                        };
                        if this.page.as_deref() != Some(&page) {
                            this.days = Arc::new(by_day(&page.items, &this.clock));
                            this.page = Some(Arc::new(page));
                        }
                    }
                    Err(error) => {
                        this.message =
                            format!("Calendar load failed: {error}. Previous dates retained.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn compact_day_width(&self) -> f32 {
        self.width / self.columns.max(1) as f32
    }

    /// Left edge of the `index`th compact column. Edges are whole pixels so that rounding
    /// fractional widths over six weeks of columns cannot drift the strip off a day boundary.
    fn compact_left(&self, index: usize) -> f32 {
        (index as f32 * self.compact_day_width()).round()
    }

    /// The first fully visible day of the compact strip, from its scroll position.
    fn compact_visible_start(&self) -> NaiveDate {
        let width = self.compact_day_width();
        if self.compact_recentre || width <= 0. {
            return self.compact_start;
        }
        let total = (COMPACT_PAST_DAYS + COMPACT_FUTURE_DAYS) as usize;
        let index = (-f32::from(self.compact_scroll.offset().x) / width)
            .round()
            .max(0.) as usize;
        compact_days(self.compact_start)[index.min(total.saturating_sub(self.columns))]
    }

    /// Scrolls the compact strip so `start` is its first visible day.
    fn scroll_compact_to(&self, start: NaiveDate) {
        let first = self.compact_start - Days::new(COMPACT_PAST_DAYS);
        let index = (start - first).num_days().max(0) as usize;
        self.compact_scroll
            .set_offset(point(px(-self.compact_left(index)), px(0.)));
    }

    fn step(&mut self, direction: i32, cx: &mut Context<Self>) {
        self.popover = None;
        if self.columns == 7 {
            self.month = if direction < 0 {
                self.month - Months::new(1)
            } else {
                self.month + Months::new(1)
            };
        } else {
            let shift = Days::new(self.columns as u64);
            let start = self.compact_visible_start();
            self.compact_start = if direction < 0 {
                start - shift
            } else {
                start + shift
            };
            self.compact_recentre = true;
        }
        self.load(cx);
    }

    fn go_to_today(&mut self, cx: &mut Context<Self>) {
        let today = self.clock.today();
        self.today = today;
        self.popover = None;
        self.month = today.with_day(1).unwrap_or(today);
        self.compact_start = today;
        self.compact_recentre = true;
        self.load(cx);
    }

    /// Shipping's refresh: a calendar sync now, with 1.5 seconds of feedback.
    fn manual_refresh(&mut self, cx: &mut Context<Self>) {
        let generation = self.refreshing.advance();
        self.showing_refresh = true;
        self.sync.update(cx, |sync, cx| {
            sync.cancel_debounced(cx);
            sync.schedule(cx);
        });
        self.load(cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.refreshing == generation {
                    this.showing_refresh = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn toggle_popover(&mut self, popover: Popover, window: &mut Window, cx: &mut Context<Self>) {
        // Clicking the open popover's trigger first dismisses it via mouse-down-out; keep it closed.
        if self.dismissed.as_ref() == Some(&popover) {
            return;
        }
        self.popover = Some(popover);
        self.panel = Panel::Main;
        self.picker_closed = false;
        self.menu = None;
        // Escape closes the popover, so keys must reach the calendar.
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn dismiss_popover(&mut self, cx: &mut Context<Self>) {
        self.dismissed = self.popover.take();
        self.panel = Panel::Main;
        self.picker_closed = false;
        cx.notify();
    }

    fn rules(&self) -> Arc<SeriesRules> {
        self.page
            .as_ref()
            .map(|page| page.rules.clone())
            .unwrap_or_default()
    }

    /// The folder picker closed after a choice ("See all folders" included).
    fn close_picker(&mut self) {
        self.picker_closed = true;
        self.panel = Panel::Main;
    }

    fn open_folders(
        &mut self,
        item: &CalendarItem,
        rule: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.filing {
            return;
        }
        self.folder_at_open = item.folder.clone();
        self.picker_closed = false;
        self.panel = Panel::Folders { rule };
        self.folder_search
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.focus_view(&self.folder_search, window);
        let reply = self
            .runtime
            .read(CancellationToken::new(), |services| async move {
                let rows = services
                    .executor
                    .execute(FOLDER_QUERY.into(), vec![])
                    .await
                    .map_err(failure)?;
                Ok(Arc::new(FolderList::new(rows.iter().map(|row| {
                    (field(row, "path").to_string(), row["known"] == 1)
                }))))
            });
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(folders) => this.folders = Some(folders),
                    Err(error) => this.message = format!("Could not load folders: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Files the event's note (creating it first when needed) like the shipping picker:
    /// choosing the current folder takes the note out of it.
    fn file_item(
        &mut self,
        item: &CalendarItem,
        path: Arc<str>,
        create: bool,
        cx: &mut Context<Self>,
    ) {
        self.close_picker();
        let folder: Arc<str> = if path == item.folder { "".into() } else { path };
        if self.filing || (folder.is_empty() && item.session.is_none()) {
            cx.notify();
            return;
        }
        self.filing = true;
        cx.notify();
        let runtime = self.runtime.clone();
        let session = item.session.clone();
        let created = session
            .is_none()
            .then(|| open_event(&runtime, item.id.clone(), self.viewer.clone()));
        let new_folder = (create && !folder.is_empty()).then(|| {
            let mut draft = super::mutations::blank(super::ports::Catalog::Folders);
            if let Some(field) = draft.fields.iter_mut().find(|field| field.key == "path") {
                field.value = folder.to_string();
            }
            super::mutations::dispatch(
                &runtime,
                super::mutations::Command::Save(draft),
                self.viewer.clone(),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = async {
                if let Some(reply) = new_folder {
                    reply?.receive().await?;
                }
                let id = match (session, created) {
                    (Some(id), _) => id,
                    (None, Some(reply)) => reply?.receive().await?,
                    (None, None) => return Err(ServiceError::Conflict),
                };
                super::notes::dispatch(
                    &runtime,
                    super::notes::NoteCommand::Move {
                        ids: vec![id].into(),
                        folder,
                    },
                )?
                .receive()
                .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.filing = false;
                if let Err(error) = result {
                    this.message = format!("Could not file the event note: {error}");
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The rule picker: choosing the series' folder again stops auto-adding.
    fn choose_rule_folder(
        &mut self,
        item: &CalendarItem,
        path: Arc<str>,
        create: bool,
        cx: &mut Context<Self>,
    ) {
        self.close_picker();
        let current = self.rules().folder(&item.series).cloned();
        let change = if current.as_ref() == Some(&path) {
            RuleChange::Clear
        } else {
            RuleChange::Set(path.clone())
        };
        let new_folder = (create && matches!(change, RuleChange::Set(_))).then(|| {
            let mut draft = super::mutations::blank(super::ports::Catalog::Folders);
            if let Some(field) = draft.fields.iter_mut().find(|field| field.key == "path") {
                field.value = path.to_string();
            }
            super::mutations::dispatch(
                &self.runtime,
                super::mutations::Command::Save(draft),
                self.viewer.clone(),
            )
        });
        self.change_rule(item, change, new_folder, cx);
    }

    fn change_rule(
        &mut self,
        item: &CalendarItem,
        change: RuleChange,
        before: Option<Result<Reply<super::ports::CatalogRow>>>,
        cx: &mut Context<Self>,
    ) {
        if item.series.is_empty() {
            return;
        }
        let runtime = self.runtime.clone();
        let series = item.series.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = async {
                if let Some(reply) = before {
                    reply?.receive().await?;
                }
                calendar_rules::update(&runtime, series, change)?
                    .receive()
                    .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    // Rules are not part of the watched rows, so the watch stays quiet.
                    Ok(()) => this.load(cx),
                    Err(error) => this.message = format!("Could not update auto-add: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn snap_compact(&mut self, cx: &mut Context<Self>) {
        // Shipping's strip uses `snap-x snap-mandatory`: settle on the nearest whole day.
        self.snap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(150))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.columns != 7 && !this.compact_recentre {
                    let start = this.compact_visible_start();
                    this.scroll_compact_to(start);
                    cx.notify();
                }
            });
        }));
    }

    fn open_item(&mut self, item: &CalendarItem, cx: &mut Context<Self>) {
        if let Some(session) = item.session.clone() {
            self.popover = None;
            cx.emit(CalendarOpen(session));
            return;
        }
        if self.opening {
            return;
        }
        let reply = open_event(&self.runtime, item.id.clone(), self.viewer.clone());
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                match result {
                    Ok(id) => {
                        this.popover = None;
                        cx.emit(CalendarOpen(id));
                        this.load(cx);
                    }
                    Err(error) => this.message = format!("Could not open event: {error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Drop for CalendarView {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.watch_cancel.cancel();
    }
}

fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

fn primary_button(
    id: &'static str,
    label: &'static str,
    busy: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .min_h(px(32.))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded(px(8.))
        .bg(colors.primary)
        .text_color(colors.primary_foreground)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |style| style.bg(colors.primary.opacity(0.9)))
        .when(busy, |button| {
            button
                .opacity(0.5)
                .child(spinner(id, colors.primary_foreground))
        })
        .child(label)
}

fn secondary_button(
    id: &'static str,
    label: &'static str,
    disabled: bool,
    colors: Theme,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .min_h(px(32.))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .rounded(px(8.))
        .bg(colors.secondary)
        .text_color(colors.secondary_foreground)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |style| style.bg(colors.secondary.opacity(0.8)))
        .when(disabled, |button| button.opacity(0.5))
        .child(label)
}

/// Shipping's `CircleNotch` spinner (size-3.5).
fn spinner(id: &'static str, color: Hsla) -> impl IntoElement {
    icon("chat/LoaderCircleIcon.svg", 14., color).with_animation(
        SharedString::from(format!("{id}-spinner")),
        Animation::new(Duration::from_secs(1)).repeat(),
        |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
    )
}

fn separator(colors: Theme) -> gpui::Div {
    div().h(px(1.)).flex_shrink_0().bg(colors.accent)
}

/// The shipping app's "app" popover: a chrome ring around a rounded panel.
fn floating(width: f32, colors: Theme, content: impl IntoElement) -> gpui::Div {
    div()
        .w(px(width))
        .p(px(2.))
        .rounded(px(22.))
        .border_1()
        .border_color(colors.app_floating_border)
        .bg(colors.app_floating_chrome)
        .text_color(colors.popover_foreground)
        .shadow_lg()
        .child(
            div()
                .rounded(px(20.))
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_panel)
                .overflow_hidden()
                .child(content),
        )
}

fn meeting_domain(link: &str) -> Option<String> {
    let url = url::Url::parse(link).ok()?;
    url.host_str()
        .map(|host| host.strip_prefix("www.").unwrap_or(host).to_owned())
}

impl CalendarView {
    fn chip(
        &self,
        day: NaiveDate,
        item: &CalendarItem,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let color = parse_color(&item.color).unwrap_or_else(|| rgb(0x888888).into());
        let popover = Popover::Item(day, item.id.clone());
        let selected = self.popover.as_ref() == Some(&popover);
        let start = parse_instant(&item.start).map(|at| self.clock.time(self.clock.local(at)));
        let chip = div()
            .id(SharedString::from(format!(
                "calendar-item:{day}:{}",
                item.id
            )))
            .relative()
            .w_full()
            .flex_shrink_0()
            .text_xs()
            .line_height(px(CHIP_HEIGHT))
            .rounded(px(4.))
            .cursor_pointer()
            .hover(|style| style.opacity(0.8))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_popover(popover.clone(), window, cx)
            }))
            .when_some(
                item.session.clone().filter(|_| item.note),
                |chip, session| {
                    chip.on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            this.menu = Some((event.position, session.clone()));
                            this.focus.focus(window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                },
            );
        let chip = if item.all_day && !item.note {
            chip.px(px(6.))
                .py(px(2.))
                .truncate()
                .bg(color)
                .text_color(colors.primary_foreground)
                .child(item.title())
        } else {
            let marker = div()
                .w(px(if item.note { 4. } else { 2.5 }))
                .flex_shrink_0()
                .self_stretch()
                .rounded(px(8.));
            chip.flex()
                .items_center()
                .gap_1()
                .pl(px(2.))
                .child(if item.note {
                    marker.border_1().border_color(colors.border)
                } else {
                    marker.bg(color)
                })
                .child(div().min_w_0().truncate().child(item.title()))
                .when_some(start, |chip, start| {
                    chip.child(
                        div()
                            .ml_auto()
                            .flex_shrink_0()
                            .font_family(monospace_font(cx))
                            .text_color(colors.muted_foreground)
                            .child(start),
                    )
                })
        };
        chip.when(selected, |chip| {
            chip.child(self.anchor(item.chip_height(), self.item_popover(item, colors, cx)))
        })
    }

    fn anchor(&self, below: f32, content: impl IntoElement) -> gpui::Div {
        div()
            .absolute()
            .left_0()
            .top(px(below + 4.))
            .child(deferred(
                anchored()
                    .snap_to_window_with_margin(px(16.))
                    .child(content),
            ))
    }

    fn item_popover(
        &self,
        item: &CalendarItem,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let open = {
            let item = item.clone();
            cx.listener(move |this, _, _, cx| this.open_item(&item, cx))
        };
        let rules = self.rules();
        let rule = rules.folder(&item.series).cloned();
        // Shipping falls back to the details once the rule it manages is gone.
        let panel = match self.panel {
            Panel::Manage if rule.is_none() => Panel::Main,
            panel => panel,
        };
        let auto_add = panel == Panel::Main
            && !item.series.is_empty()
            && rule.is_none()
            && self.picker_closed
            && !item.folder.is_empty()
            && item.folder != self.folder_at_open
            && !rules.declined.contains(&item.series);
        let content = if let (Panel::Folders { rule: for_rule }, false) = (panel, item.note) {
            self.folder_list(
                item,
                for_rule.then(|| rule.clone().unwrap_or_default()),
                colors,
                cx,
            )
        } else if let (Some(rule), Panel::Manage, false) = (rule.as_ref(), panel, item.note) {
            self.manage_view(item, rule, colors, cx)
        } else if auto_add && !item.note {
            self.auto_add_view(item, colors, cx)
        } else if item.note {
            let created =
                parse_instant(&item.start).map(|at| self.clock.date_time(self.clock.local(at)));
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(
                    div()
                        .text_base()
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.foreground)
                        .child(item.title()),
                )
                .child(separator(colors))
                .when_some(created, |view, created| {
                    view.child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child(created),
                    )
                })
                .child(
                    primary_button("calendar-open-note", "Open note", false, colors).on_click(open),
                )
        } else {
            let when = parse_instant(&item.start).map(|start| {
                let start = self.clock.local(start);
                let mut text = self.clock.date_time(start);
                if let Some(end) = parse_instant(&item.end).map(|end| self.clock.local(end)) {
                    text.push_str(" to ");
                    text.push_str(&if end.date() == start.date() {
                        self.clock.time(end)
                    } else {
                        self.clock.date_time(end)
                    });
                }
                text
            });
            let location = (!item.location.trim().is_empty()
                && url::Url::parse(&item.location).is_err())
            .then(|| item.location.clone());
            let link = (!item.meeting_link.trim().is_empty()).then(|| item.meeting_link.clone());
            let muted_row = |icon_path: &'static str| {
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child(icon(icon_path, 16., colors.muted_foreground))
            };
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .child(
                    div()
                        .min_w_0()
                        .text_base()
                        .line_height(px(24.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.foreground)
                        .child(if item.title.trim().is_empty() {
                            SharedString::from("Busy")
                        } else {
                            SharedString::from(item.title.clone())
                        }),
                )
                .child(separator(colors))
                .when_some(location, |view, location| {
                    view.child(
                        muted_row("workspace/MapPinIcon.svg")
                            .child(div().min_w_0().child(SharedString::from(location))),
                    )
                })
                .when_some(link, |view, link| {
                    let target = link.to_string();
                    view.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(muted_row("workspace/Video01Icon.svg").child(
                                div().min_w_0().truncate().child(
                                    meeting_domain(&link).unwrap_or_else(|| "Meeting link".into()),
                                ),
                            ))
                            .child(
                                div()
                                    .id("calendar-join")
                                    .h(px(28.))
                                    .px_2()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .rounded(px(8.))
                                    .bg(colors.primary)
                                    .text_color(colors.primary_foreground)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .cursor_pointer()
                                    .hover(move |style| style.bg(colors.primary.opacity(0.9)))
                                    .child("Join")
                                    .on_click(move |_, _, cx: &mut App| cx.open_url(&target)),
                            ),
                    )
                })
                .when_some(when, |view, when| {
                    view.child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child(when),
                    )
                })
                .when(!item.description.trim().is_empty(), |view| {
                    view.child(separator(colors)).child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child(SharedString::from(item.description.clone())),
                    )
                })
                .child(self.folder_button(item, rule.is_some(), colors, cx))
                .child(
                    primary_button("calendar-open-note", "Open note", self.opening, colors)
                        .on_click(open),
                )
        };
        div()
            .id("calendar-popover")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.dismiss_popover(cx)))
            .child(floating(
                if item.note { 280. } else { 320. },
                colors,
                content,
            ))
    }

    fn folder_button(
        &self,
        item: &CalendarItem,
        has_rule: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let folder = icon(
            "workspace/Folder01Icon.svg",
            14.,
            colors.secondary_foreground,
        );
        let target = item.clone();
        div()
            .id("calendar-add-folder")
            .w_full()
            .min_h(px(32.))
            .px_2()
            .flex()
            .items_center()
            .gap_2()
            .rounded(px(8.))
            .bg(colors.secondary)
            .text_color(colors.secondary_foreground)
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .hover(move |style| style.bg(colors.secondary.opacity(0.8)))
            .child(if self.filing {
                spinner("calendar-filing", colors.secondary_foreground).into_any_element()
            } else {
                folder.into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(if item.folder.is_empty() {
                        SharedString::from("Add to folder")
                    } else {
                        SharedString::from(folder_name(&item.folder).to_owned())
                    }),
            )
            .child(icon(
                "ArrowDown01Icon.svg",
                14.,
                colors.secondary_foreground,
            ))
            .on_click(cx.listener(move |this, _, window, cx| {
                if this.filing {
                    return;
                }
                if has_rule {
                    this.panel = Panel::Manage;
                    cx.notify();
                } else {
                    this.open_folders(&target, false, window, cx);
                }
            }))
    }

    fn back_link(
        &self,
        id: &'static str,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .line_height(px(16.))
            .text_color(colors.muted_foreground)
            .cursor_pointer()
            .hover(move |style| style.text_color(colors.foreground))
            .child(icon(
                "workspace/ArrowLeft01Icon.svg",
                14.,
                colors.muted_foreground,
            ))
            .child("Back")
            .on_click(cx.listener(|this, _, _, cx| {
                this.panel = Panel::Main;
                cx.notify();
            }))
    }

    /// Shipping's "manage" view of a series with an auto-add folder.
    fn manage_view(
        &self,
        item: &CalendarItem,
        rule: &Arc<str>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let row = |label: gpui::AnyElement, value: SharedString| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_sm()
                .line_height(px(20.))
                .child(label)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(colors.foreground)
                        .child(value),
                )
        };
        let (occurrence, for_rule, stop) = (item.clone(), item.clone(), item.clone());
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(self.back_link("calendar-manage-back", colors, cx))
            .child(row(
                div()
                    .text_color(colors.muted_foreground)
                    .child("This note")
                    .into_any_element(),
                if item.folder.is_empty() {
                    "Unfiled".into()
                } else {
                    folder_name(&item.folder).to_owned().into()
                },
            ))
            .child(row(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_color(colors.muted_foreground)
                    .child(icon(
                        "workspace/RepeatIcon.svg",
                        14.,
                        colors.muted_foreground,
                    ))
                    .child("Future meetings")
                    .into_any_element(),
                folder_name(rule).to_owned().into(),
            ))
            .child(
                secondary_button("calendar-move-note", "Move this note", self.filing, colors)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_folders(&occurrence, false, window, cx)
                    })),
            )
            .child(
                secondary_button(
                    "calendar-change-rule",
                    "Change auto-add folder",
                    self.filing,
                    colors,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_folders(&for_rule, true, window, cx)
                })),
            )
            .child(
                secondary_button("calendar-stop-rule", "Stop auto-add", false, colors).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.panel = Panel::Main;
                        this.change_rule(&stop, RuleChange::Clear, None, cx);
                    }),
                ),
            )
    }

    /// Offered after a recurring meeting's note is filed into a folder.
    fn auto_add_view(
        &self,
        item: &CalendarItem,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let (accept, decline) = (item.clone(), item.clone());
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(colors.foreground)
                    .child(icon("workspace/RepeatIcon.svg", 16., colors.foreground))
                    .child("Auto-add future meetings?"),
            )
            .child(
                div()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(colors.muted_foreground)
                    .child("Automatically put all future instances of this recurring meeting into a folder."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        primary_button("calendar-auto-add", "Auto-add", false, colors)
                            .flex_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.picker_closed = false;
                                let folder = accept.folder.clone();
                                this.change_rule(&accept, RuleChange::Set(folder), None, cx);
                            })),
                    )
                    .child(
                        secondary_button("calendar-auto-add-no", "No", false, colors)
                            .flex_1()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.picker_closed = false;
                                this.change_rule(&decline, RuleChange::Decline, None, cx);
                            })),
                    ),
            )
    }

    /// `rule` holds the series' current folder when the picker chooses it instead of this
    /// note's folder.
    fn folder_list(
        &self,
        item: &CalendarItem,
        rule: Option<Arc<str>>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let current = rule.clone().unwrap_or_else(|| item.folder.clone());
        let for_rule = rule.is_some();
        let typed = self.folder_search.read(cx).buffer.text.trim().to_owned();
        let query = typed.to_lowercase();
        let name = anlg_fs_sync_core::normalize_folder_path(&typed).ok();
        let list = self.folders.clone().unwrap_or_default();
        let folders = list
            .paths
            .iter()
            .filter(|path| path.to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        // Like the shipping command list, the current folder starts highlighted, else the first.
        let highlighted = folders
            .iter()
            .find(|path| **path == current)
            .or(folders.first())
            .cloned();
        let creatable = name
            .as_deref()
            .filter(|name| !name.is_empty())
            .filter(|name| !list.paths.iter().any(|path| &**path == *name))
            .map(Arc::<str>::from);
        let empty = if query.is_empty() {
            "No folders yet."
        } else if name.is_none() {
            "Enter a valid folder name."
        } else {
            "No folders found."
        };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .child(
                self.back_link("calendar-folder-back", colors, cx)
                    .px_2()
                    .py_1(),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .rounded(px(20.))
                            .border_1()
                            .border_color(colors.app_floating_border)
                            .bg(colors.app_floating_panel)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h(px(32.))
                                    .px(px(10.))
                                    .flex()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(colors.border)
                                    .child(
                                        icon("Search01Icon.svg", 16., colors.foreground)
                                            .mr_2()
                                            .opacity(0.5),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_sm()
                                            .child(self.folder_search.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .id("calendar-folder-list")
                                    .max_h(px(300.))
                                    .overflow_y_scroll()
                                    .p_2()
                                    .when(
                                        self.folders.is_some()
                                            && folders.is_empty()
                                            && creatable.is_none(),
                                        |list| {
                                            list.child(
                                                div()
                                                    .px(px(10.))
                                                    .py_2()
                                                    .text_sm()
                                                    .text_color(colors.muted_foreground)
                                                    .child(empty),
                                            )
                                        },
                                    )
                                    .children(folders.into_iter().map(|path| {
                                        let selected_path = path == current;
                                        let selected = highlighted.as_ref() == Some(&path);
                                        let target = item.clone();
                                        let chosen = path.clone();
                                        let create = !list.known(&path);
                                        div()
                                            .id(SharedString::from(format!(
                                                "calendar-folder:{path}"
                                            )))
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .px_2()
                                            .py(px(6.))
                                            .rounded(px(14.))
                                            .text_sm()
                                            .line_height(px(20.))
                                            .text_color(colors.foreground)
                                            .cursor_pointer()
                                            .when(selected, |row| row.bg(colors.accent))
                                            .hover(move |style| style.bg(colors.accent))
                                            .child(
                                                icon(
                                                    "workspace/Folder01Icon.svg",
                                                    16.,
                                                    colors.foreground,
                                                )
                                                .opacity(0.7),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .truncate()
                                                    .child(SharedString::from(path.clone())),
                                            )
                                            .when(selected_path, |row| {
                                                row.child(icon(
                                                    "workspace/CheckIcon.svg",
                                                    16.,
                                                    colors.foreground,
                                                ))
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if for_rule {
                                                    this.choose_rule_folder(
                                                        &target,
                                                        chosen.clone(),
                                                        create,
                                                        cx,
                                                    )
                                                } else {
                                                    this.file_item(
                                                        &target,
                                                        chosen.clone(),
                                                        create,
                                                        cx,
                                                    )
                                                }
                                            }))
                                    }))
                                    .when_some(creatable, |list, name| {
                                        let target = item.clone();
                                        list.child(
                                            div()
                                                .id("calendar-folder-create")
                                                .flex()
                                                .items_center()
                                                .gap_2()
                                                .px_2()
                                                .py(px(6.))
                                                .rounded(px(14.))
                                                .text_sm()
                                                .line_height(px(20.))
                                                .text_color(colors.foreground)
                                                .cursor_pointer()
                                                .hover(move |style| style.bg(colors.accent))
                                                .child(icon(
                                                    "workspace/PlusIcon.svg",
                                                    16.,
                                                    colors.foreground,
                                                ))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .truncate()
                                                        .child(format!("Create \"{name}\"")),
                                                )
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    if for_rule {
                                                        this.choose_rule_folder(
                                                            &target,
                                                            name.clone(),
                                                            true,
                                                            cx,
                                                        )
                                                    } else {
                                                        this.file_item(
                                                            &target,
                                                            name.clone(),
                                                            true,
                                                            cx,
                                                        )
                                                    }
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .id("calendar-all-folders")
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_1()
                            .px_3()
                            .py(px(6.))
                            .text_xs()
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.muted_foreground)
                            .cursor_pointer()
                            .hover(move |style| {
                                style.bg(colors.accent).text_color(colors.foreground)
                            })
                            .child("See all folders")
                            .child(icon(
                                "workspace/ArrowRight01Icon.svg",
                                14.,
                                colors.muted_foreground,
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_picker();
                                this.popover = None;
                                cx.emit(CalendarFolders);
                                cx.notify();
                            })),
                    ),
            )
    }

    /// Choosing an item here swaps the list for its details, anchored at the same "+N more".
    fn more_popover(
        &self,
        day: NaiveDate,
        items: &[CalendarItem],
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let rows = items
            .iter()
            .map(|item| self.chip(day, item, colors, cx))
            .collect::<Vec<_>>();
        div()
            .id("calendar-more-popover")
            .occlude()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.dismiss_popover(cx)))
            .child(floating(
                220.,
                colors,
                div()
                    .id("calendar-more-list")
                    .max_h(px(296.))
                    .overflow_y_scroll()
                    .p_2()
                    .child(
                        div()
                            .mb_2()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.foreground)
                            .child(day.format("%b %-d, %Y").to_string()),
                    )
                    .child(div().flex().flex_col().gap(px(CHIP_GAP)).children(rows)),
            ))
    }

    fn day_cell(
        &self,
        day: NaiveDate,
        in_month: bool,
        cell_height: Option<f32>,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let items = self.days.get(&day).map(Vec::as_slice).unwrap_or_default();
        let today = day == self.today;
        let chip = items
            .first()
            .map(CalendarItem::chip_height)
            .unwrap_or(CHIP_HEIGHT);
        let shown = visible_count(
            items.len(),
            chip,
            cell_height.map(|height| height - CELL_CHROME),
        );
        let hidden = items.len() - shown;
        let popover_here = self.popover.as_ref().filter(|popover| popover.day() == day);
        // An item opened from the "+N more" list stays anchored there.
        let detached = popover_here.and_then(|popover| match popover {
            Popover::Item(_, id) => items[shown..].iter().find(|item| &item.id == id),
            Popover::More(_) => None,
        });
        let weekend = weekend(day);
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .p(px(6.))
            .border_r_1()
            .border_b_1()
            .border_color(colors.border)
            .when(weekend, |cell| cell.bg(colors.muted))
            .child(
                div().flex().flex_shrink_0().justify_end().child(
                    div()
                        .mb_1()
                        .size(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(8.))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if today {
                            colors.background
                        } else if !in_month {
                            colors.muted_foreground.opacity(0.7)
                        } else if weekend {
                            colors.muted_foreground
                        } else {
                            colors.foreground
                        })
                        .when(today, |badge| badge.bg(colors.foreground))
                        .child(day.day().to_string()),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(CHIP_GAP))
                    .overflow_hidden()
                    .children(
                        items[..shown]
                            .iter()
                            .map(|item| self.chip(day, item, colors, cx)),
                    )
                    .when(hidden > 0, |list| {
                        let popover = Popover::More(day);
                        list.child(
                            div()
                                .id(SharedString::from(format!("calendar-more:{day}")))
                                .relative()
                                .flex_shrink_0()
                                .pl_1()
                                .text_xs()
                                .line_height(px(16.))
                                .text_color(colors.muted_foreground)
                                .cursor_pointer()
                                .child(format!("+{hidden} more"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.toggle_popover(popover.clone(), window, cx)
                                }))
                                .when_some(detached, |more, item| {
                                    more.child(
                                        self.anchor(16., self.item_popover(item, colors, cx)),
                                    )
                                })
                                .when(popover_here == Some(&Popover::More(day)), |more| {
                                    more.child(
                                        self.anchor(16., self.more_popover(day, items, colors, cx)),
                                    )
                                }),
                        )
                    }),
            )
    }

    fn header(&self, title: String, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let segment = |id: &'static str| {
            div()
                .id(id)
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(move |style| style.bg(colors.accent))
        };
        let divider = || div().w(px(1.)).h_full().bg(colors.accent);
        div()
            .h(px(HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.foreground)
                            .child(title),
                    )
                    // Shipping shows its spinner while a sync is scheduled or running.
                    .child(
                        if self.showing_refresh || self.sync.read(cx).status() != SyncStatus::Idle {
                            div()
                                .id("calendar-syncing")
                                .size(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(crate::meeting::widgets::spinner(
                                    "calendar-syncing",
                                    12.,
                                    colors.muted_foreground,
                                ))
                                .into_any_element()
                        } else {
                            div()
                                .id("calendar-refresh")
                                .size(px(24.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(8.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(colors.accent))
                                .child(icon("workspace/RefreshIcon.svg", 14., colors.foreground))
                                .on_click(cx.listener(|this, _, _, cx| this.manual_refresh(cx)))
                                .into_any_element()
                        },
                    ),
            )
            .child(
                div()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .child(
                        segment("calendar-previous")
                            .w(px(28.))
                            .child(icon(
                                "workspace/ArrowLeft01Icon.svg",
                                14.,
                                colors.foreground,
                            ))
                            .on_click(cx.listener(|this, _, _, cx| this.step(-1, cx))),
                    )
                    .child(divider())
                    .child(
                        segment("calendar-today")
                            .px_2()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors.foreground)
                            .child("Today")
                            .on_click(cx.listener(|this, _, _, cx| this.go_to_today(cx))),
                    )
                    .child(divider())
                    .child(
                        segment("calendar-next")
                            .w(px(28.))
                            .child(icon(
                                "workspace/ArrowRight01Icon.svg",
                                14.,
                                colors.foreground,
                            ))
                            .on_click(cx.listener(|this, _, _, cx| this.step(1, cx))),
                    ),
            )
    }

    /// Shipping's native note menu (`SessionChip`): reveal the note's folder or delete it.
    fn note_menu(
        &self,
        position: Point<Pixels>,
        session: SessionId,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let reveal = if cfg!(target_os = "macos") {
            "Show in Finder"
        } else {
            "Show in folder"
        };
        let items = [
            super::calendar_sidebar::MenuItem::new(reveal),
            super::calendar_sidebar::MenuItem::new("Delete Note").after_separator(),
        ];
        super::calendar_sidebar::context_menu(
            "calendar-note-menu",
            position,
            items,
            colors,
            cx.listener(move |this, index: &usize, _, cx| {
                this.menu = None;
                let id = session.clone();
                cx.emit(if *index == 0 {
                    super::library::OpenNote::Reveal(id)
                } else {
                    super::library::OpenNote::Delete(vec![id].into())
                });
                cx.notify();
            }),
            cx.listener(|this, _: &(), _, cx| {
                this.menu = None;
                cx.notify();
            }),
        )
    }

    fn weekday_label(&self, label: &'static str, colors: Theme) -> gpui::Div {
        div()
            .py_2()
            .flex()
            .justify_center()
            .text_xs()
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if matches!(label, "Sat" | "Sun") {
                colors.muted_foreground
            } else {
                colors.foreground
            })
            .child(label)
    }

    fn month_view(&self, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let days = month_days(self.month, self.week_start);
        let rows = days.len() / 7;
        let row_height = self.body_height.map(|height| height / rows.max(1) as f32);
        let labels = (0..7).map(|index| WEEKDAYS[(index + usize::from(self.week_start)) % 7]);
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(colors.border)
                    .children(labels.enumerate().map(|(index, label)| {
                        self.weekday_label(label, colors)
                            .flex_1()
                            .min_w_0()
                            .when(index < 6, |label| {
                                label.border_r_1().border_color(colors.border)
                            })
                    })),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .children(days.chunks(7).map(|week| {
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .children(week.iter().map(|day| {
                                let in_month = day.month() == self.month.month();
                                self.day_cell(*day, in_month, row_height, colors, cx)
                            }))
                    }))
                    .child(self.measure_body(cx)),
            )
    }

    fn compact_view(&self, colors: Theme, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let cell_height = self.body_height.map(|height| height - WEEKDAY_HEIGHT);
        let days = compact_days(self.compact_start);
        div()
            .id("calendar-compact")
            .relative()
            .flex_1()
            .min_h_0()
            .overflow_x_scroll()
            .track_scroll(&self.compact_scroll)
            .on_scroll(cx.listener(|this, _: &ScrollEvent, _, cx| this.snap_compact(cx)))
            .child(
                div()
                    .h_full()
                    .flex()
                    .w(px(self.compact_left(days.len())))
                    .children(days.into_iter().enumerate().map(|(index, day)| {
                        div()
                            .w(px(self.compact_left(index + 1) - self.compact_left(index)))
                            .flex_shrink_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                self.weekday_label(
                                    WEEKDAYS[day.weekday().num_days_from_sunday() as usize],
                                    colors,
                                )
                                .flex_shrink_0()
                                .border_r_1()
                                .border_b_1()
                                .border_color(colors.border),
                            )
                            .child(self.day_cell(day, true, cell_height, colors, cx))
                    })),
            )
            .child(self.measure_body(cx))
    }

    /// Records the height of the day area so cells can decide how many chips fit.
    fn measure_body(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let known = self.body_height;
        canvas(
            move |bounds, window, cx| {
                let height = f32::from(bounds.size.height);
                if known != Some(height) {
                    window.defer(cx, move |_, cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.body_height = Some(height);
                            cx.notify();
                        });
                    });
                }
            },
            |_, (), _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

impl Render for CalendarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        self.today = self.clock.today();
        let entity = cx.entity().downgrade();
        let columns = self.columns;
        let width = self.width;
        if self.columns != 7 && self.compact_recentre && self.width > 0. {
            self.scroll_compact_to(self.compact_start);
            self.compact_recentre = false;
        }
        let title = if self.columns == 7 {
            self.month.format("%B %Y").to_string()
        } else {
            self.compact_visible_start().format("%B %Y").to_string()
        };
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape"
                    && (this.popover.is_some() || this.menu.is_some())
                {
                    this.menu = None;
                    this.dismiss_popover(cx);
                    this.dismissed = None;
                    cx.stop_propagation();
                }
            }))
            // Clears a dismissal from the previous click before this one is dispatched.
            .capture_any_mouse_down(cx.listener(|this, _, _, _| this.dismissed = None))
            .child(self.header(title, colors, cx))
            .map(|view| {
                if self.columns == 7 {
                    view.child(self.month_view(colors, cx))
                } else {
                    view.child(self.compact_view(colors, cx))
                }
            })
            .when_some(self.menu.clone(), |view, (position, session)| {
                view.child(self.note_menu(position, session, colors, cx))
            })
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .px_3()
                        .py_1()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(self.message.clone()),
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let measured = f32::from(bounds.size.width);
                        let next = visible_columns(measured);
                        if next != columns || measured != width {
                            window.defer(cx, move |_, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    let compact_changed = (this.columns == 7) != (next == 7);
                                    // Shipping's strip snaps to whole days; keep the first visible
                                    // day when only the width changes.
                                    let keep =
                                        (this.columns == next && next != 7 && this.width > 0.)
                                            .then(|| this.compact_visible_start());
                                    if this.columns != next {
                                        this.compact_recentre = true;
                                    }
                                    this.columns = next;
                                    this.width = measured;
                                    if let Some(start) = keep {
                                        this.scroll_compact_to(start);
                                    }
                                    if compact_changed {
                                        this.body_height = None;
                                        this.load(cx);
                                    }
                                    cx.notify();
                                });
                            });
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_start_prefers_the_setting_then_the_locale_region() {
        assert_eq!(week_start("monday", Some("en-US")), 1);
        assert_eq!(week_start("sunday", Some("de-DE")), 0);
        assert_eq!(week_start("", Some("de-DE")), 1);
        assert_eq!(week_start("", Some("en_US")), 0);
        assert_eq!(week_start("", Some("zh-Hant-TW")), 0);
        assert_eq!(week_start("", Some("ar-EG")), 0);
        assert_eq!(week_start("", Some("ko")), 0);
        assert_eq!(week_start("", None), 0);
    }

    fn date(value: &str) -> NaiveDate {
        NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap()
    }

    fn item(id: &str, start: &str, all_day: bool, note: bool) -> CalendarItem {
        CalendarItem {
            id: id.into(),
            note,
            title: id.into(),
            start: start.into(),
            end: "".into(),
            all_day,
            color: "#4f46e5".into(),
            location: "".into(),
            meeting_link: "".into(),
            description: "".into(),
            session: None,
            folder: "".into(),
            series: "".into(),
        }
    }

    #[test]
    fn shipping_calendar_breakpoints_are_exact() {
        for (width, cols) in [
            (0., 1),
            (199., 1),
            (200., 2),
            (399., 2),
            (400., 4),
            (699., 4),
            (700., 7),
        ] {
            assert_eq!(visible_columns(width), cols);
        }
    }

    #[test]
    fn month_grid_covers_whole_weeks_from_the_configured_start() {
        let sunday = month_days(date("2026-11-01"), 0);
        assert_eq!(sunday.first(), Some(&date("2026-11-01")));
        assert_eq!(sunday.last(), Some(&date("2026-12-05")));
        assert_eq!(sunday.len(), 35);
        let monday = month_days(date("2026-11-01"), 1);
        assert_eq!(monday.first(), Some(&date("2026-10-26")));
        assert_eq!(monday.last(), Some(&date("2026-12-06")));
        assert_eq!(monday.len() % 7, 0);
    }

    #[test]
    fn items_land_on_their_local_start_day_in_shipping_order() {
        let clock = Clock {
            use_24_hour_time: false,
            timezone: Some(chrono_tz::America::Los_Angeles),
        };
        let items = [
            item("note", "2026-11-17T01:00:00Z", false, true),
            item("late", "2026-11-17T02:00:00Z", false, false),
            item("early", "2026-11-16T17:00:00Z", false, false),
            item("holiday", "2026-11-16", true, false),
            item("next-day", "2026-11-17T09:00:00Z", false, false),
        ];
        let days = by_day(&items, &clock);
        let order = days[&date("2026-11-16")]
            .iter()
            .map(|item| &*item.id)
            .collect::<Vec<_>>();
        assert_eq!(order, ["holiday", "early", "late", "note"]);
        assert_eq!(&*days[&date("2026-11-17")][0].id, "next-day");
    }

    #[test]
    fn folder_picker_offers_implicit_folders_and_their_parents() {
        let list = FolderList::new([
            ("Clients/Acme".to_owned(), false),
            ("Research".to_owned(), true),
            ("/Research/".to_owned(), false),
        ]);
        assert_eq!(
            list.paths.iter().map(|path| &**path).collect::<Vec<_>>(),
            ["Clients", "Clients/Acme", "Research"]
        );
        assert!(list.known("Research"));
        assert!(!list.known("Clients/Acme"));
    }

    #[tokio::test]
    async fn new_notes_of_a_recurring_meeting_go_into_its_auto_add_folder() {
        let root = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: root.path().join("rules.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        runtime
            .submit(|services| async move {
                for sql in [
                    "INSERT INTO folders(id,path) VALUES ('f','Weekly')",
                    "INSERT INTO events(id,tracking_id_event,calendar_id,title,started_at,ended_at,recurrence_series_id) VALUES
                        ('sync-1','t1','c','Sync','2026-11-16T09:00:00Z','2026-11-16T10:00:00Z','series'),
                        ('other','t3','c','Other','2026-11-23T09:00:00Z','2026-11-23T10:00:00Z','gone')",
                    r#"INSERT INTO app_settings(id,value_json) VALUES ('auto_add_series_folders','[{"series_id":"series","folder_path":"Weekly"},{"series_id":"gone","folder_path":"Missing"}]')"#,
                ] {
                    services.executor.execute(sql.into(), vec![]).await.map_err(failure)?;
                }
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        let folder = |event: &'static str| {
            let runtime = runtime.clone();
            async move {
                let id = open_event(&runtime, event.into(), None)
                    .unwrap()
                    .receive()
                    .await
                    .unwrap();
                runtime
                    .submit(move |services| async move {
                        let rows = services
                            .executor
                            .execute(
                                "SELECT folder_path FROM sessions WHERE id = ?".into(),
                                vec![json!(id.0)],
                            )
                            .await
                            .map_err(failure)?;
                        Ok(field(&rows[0], "folder_path"))
                    })
                    .unwrap()
                    .receive()
                    .await
                    .unwrap()
            }
        };
        assert_eq!(&*folder("sync-1").await, "Weekly");
        // A rule pointing at a folder that no longer exists leaves the note unfiled.
        assert_eq!(&*folder("other").await, "");
        runtime
            .submit(|services| async move {
                services
                    .executor
                    .execute("UPDATE sessions SET folder_path=''".into(), vec![])
                    .await
                    .map_err(failure)?;
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        // Only a newly created note is filed.
        assert_eq!(&*folder("sync-1").await, "");
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn overflowing_days_reserve_a_row_for_more() {
        assert_eq!(visible_count(3, 15., None), 3);
        assert_eq!(visible_count(3, 15., Some(49.)), 3);
        assert_eq!(visible_count(4, 15., Some(49.)), 2);
        assert_eq!(visible_count(5, 15., Some(10.)), 1);
        assert_eq!(visible_count(0, 15., Some(10.)), 0);
    }
}
