use std::collections::VecDeque;

use chrono::{DateTime, Datelike, Local, TimeZone, Utc};
use desktop_runtime::SessionId;

use crate::workspace::navigation::Route;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatMode {
    FloatingClosed,
    FloatingOpen,
    RightPanelOpen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChatEvent {
    Open,
    OpenRightPanel,
    Close,
    Toggle,
}

impl ChatMode {
    pub fn next(self, event: ChatEvent) -> Self {
        use ChatEvent::*;
        match (self, event) {
            (Self::FloatingOpen | Self::RightPanelOpen, Close | Toggle) => Self::FloatingClosed,
            (Self::FloatingOpen | Self::FloatingClosed, OpenRightPanel) => Self::RightPanelOpen,
            (Self::RightPanelOpen | Self::FloatingClosed, Open) => Self::FloatingOpen,
            (Self::FloatingClosed, Toggle) => Self::FloatingOpen,
            (mode, _) => mode,
        }
    }

    pub fn is_open(self) -> bool {
        self != Self::FloatingClosed
    }

    /// Leaving the current note or opening settings/shared notes closes chat, as in the
    /// shipping tab store.
    pub fn after_navigation(self, from: &Route, to: &Route) -> Self {
        if self == Self::FloatingClosed {
            return self;
        }
        match to {
            Route::Settings(_) | Route::SharedSession(_) | Route::SharedPreview(_) => {
                Self::FloatingClosed
            }
            Route::Session(_) if from != to => Self::FloatingClosed,
            _ => self,
        }
    }
}

/// Routes whose main surface carries the floating "Ask anything" button.
pub fn shows_cta(route: &Route) -> bool {
    matches!(route, Route::Empty | Route::Session(_))
}

/// The session used as chat context: kept while visiting other routes, cleared on the
/// empty tab, like shipping's sticky session tab.
pub fn context_session(previous: Option<SessionId>, route: &Route) -> Option<SessionId> {
    match route {
        Route::Session(id) => Some(id.clone()),
        Route::Empty => None,
        _ => previous,
    }
}

const MAX_SENT: usize = 50;

/// Sent prompts recalled with up/down in the composer, newest first and shared across chats.
#[derive(Default)]
pub struct SentHistory {
    entries: VecDeque<String>,
    index: Option<usize>,
    draft: String,
}

impl SentHistory {
    pub fn push(&mut self, text: &str) {
        self.index = None;
        self.draft.clear();
        if self.entries.front().is_some_and(|entry| entry == text) {
            return;
        }
        self.entries.push_front(text.to_owned());
        self.entries.truncate(MAX_SENT);
    }

    /// Returns the text to show, or `None` when the key is not consumed.
    pub fn navigate(&mut self, previous: bool, current: &str) -> Option<String> {
        if previous {
            let next = self.index.map_or(0, |index| index + 1);
            if self.entries.is_empty() {
                return None;
            }
            if next >= self.entries.len() {
                return Some(current.to_owned());
            }
            if self.index.is_none() {
                self.draft = current.to_owned();
            }
            self.index = Some(next);
            return Some(self.entries[next].clone());
        }
        match self.index? {
            0 => {
                self.index = None;
                Some(std::mem::take(&mut self.draft))
            }
            index => {
                self.index = Some(index - 1);
                Some(self.entries[index - 1].clone())
            }
        }
    }

    /// A manual edit leaves history browsing.
    pub fn edited(&mut self) {
        self.index = None;
    }

    pub fn position(&self) -> Option<(usize, usize)> {
        self.index.map(|index| (index + 1, self.entries.len()))
    }
}

pub fn fallback_title(request: &str) -> String {
    const MAX: usize = 50;
    let title = request.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        return "New chat".into();
    }
    if title.chars().count() <= MAX {
        return title;
    }
    let truncated = title.chars().take(MAX - 3).collect::<String>();
    let truncated = truncated.trim_end();
    let prefix = match truncated.rfind(' ') {
        Some(space) if truncated[..space].chars().count() > MAX / 2 => &truncated[..space],
        _ => truncated,
    };
    format!("{prefix}...")
}

/// `6:33 PM`, or with the day: `Today 6:33 PM`, `Mar 4 6:33 PM`, `Mar 4, 2025 6:33 PM`.
pub fn timestamp_label<Tz: TimeZone>(at: DateTime<Utc>, now: DateTime<Tz>, with_day: bool) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let at = at.with_timezone(&now.timezone());
    let time = at.format("%-I:%M %p").to_string();
    if !with_day {
        return time;
    }
    let day = if at.date_naive() == now.date_naive() {
        "Today".to_owned()
    } else if now.date_naive().pred_opt() == Some(at.date_naive()) {
        "Yesterday".to_owned()
    } else if at.year() == now.year() {
        at.format("%b %-d").to_string()
    } else {
        at.format("%b %-d, %Y").to_string()
    };
    format!("{day} {time}")
}

pub fn local_timestamp(at: DateTime<Utc>, with_day: bool) -> String {
    timestamp_label(at, Local::now(), with_day)
}

/// date-fns `formatDistanceToNow(date, { addSuffix: true })`, used by the history menu.
pub fn distance_to_now(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - at).num_seconds();
    let (past, seconds) = (seconds >= 0, seconds.unsigned_abs());
    let minutes = (seconds as f64 / 60.).round() as u64;
    let plural = |count: u64, unit: &str| {
        if count == 1 {
            format!("1 {unit}")
        } else {
            format!("{count} {unit}s")
        }
    };
    let months = minutes / 43_200;
    let distance = match minutes {
        0..=0 if seconds < 30 => "less than a minute".into(),
        0..=1 => "1 minute".into(),
        2..=44 => plural(minutes, "minute"),
        45..=89 => "about 1 hour".into(),
        90..=1_439 => format!(
            "about {}",
            plural((minutes as f64 / 60.).round() as u64, "hour")
        ),
        1_440..=2_519 => "1 day".into(),
        2_520..=43_199 => plural((minutes as f64 / 1_440.).round() as u64, "day"),
        43_200..=86_399 => format!(
            "about {}",
            plural((minutes as f64 / 43_200.).round() as u64, "month")
        ),
        _ if months < 12 => plural(months, "month"),
        _ => {
            let years = months / 12;
            match months % 12 {
                0..=2 => format!("about {}", plural(years, "year")),
                3..=8 => format!("over {}", plural(years, "year")),
                _ => format!("almost {}", plural(years + 1, "year")),
            }
        }
    };
    if past {
        format!("{distance} ago")
    } else {
        format!("in {distance}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, FixedOffset};

    use super::*;

    #[test]
    fn mode_transitions_match_shipping_chat_states() {
        use ChatEvent::*;
        use ChatMode::*;
        assert_eq!(FloatingClosed.next(Toggle), FloatingOpen);
        assert_eq!(FloatingOpen.next(Toggle), FloatingClosed);
        assert_eq!(FloatingOpen.next(OpenRightPanel), RightPanelOpen);
        assert_eq!(RightPanelOpen.next(Open), FloatingOpen);
        assert_eq!(RightPanelOpen.next(Toggle), FloatingClosed);
        assert_eq!(FloatingClosed.next(Close), FloatingClosed);
        assert_eq!(FloatingOpen.next(Open), FloatingOpen);
    }

    #[test]
    fn navigation_closes_chat_when_leaving_the_note_or_opening_settings() {
        let note = Route::Session(SessionId("a".into()));
        let other = Route::Session(SessionId("b".into()));
        let open = ChatMode::FloatingOpen;
        assert_eq!(open.after_navigation(&note, &note), open);
        assert_eq!(
            open.after_navigation(&note, &other),
            ChatMode::FloatingClosed
        );
        assert_eq!(
            ChatMode::RightPanelOpen.after_navigation(&Route::Empty, &Route::settings("app")),
            ChatMode::FloatingClosed
        );
        assert_eq!(open.after_navigation(&note, &Route::Calendar), open);
        assert_eq!(
            ChatMode::FloatingClosed.after_navigation(&note, &other),
            ChatMode::FloatingClosed
        );
    }

    #[test]
    fn context_session_sticks_outside_notes_and_clears_on_the_empty_tab() {
        let id = SessionId("a".into());
        let current = context_session(None, &Route::Session(id.clone()));
        assert_eq!(current, Some(id.clone()));
        assert_eq!(context_session(current.clone(), &Route::Calendar), Some(id));
        assert_eq!(context_session(current, &Route::Empty), None);
    }

    #[test]
    fn sent_history_recalls_newest_first_and_restores_the_draft() {
        let mut history = SentHistory::default();
        assert_eq!(history.navigate(true, "draft"), None);
        history.push("first");
        history.push("second");
        history.push("second");
        assert_eq!(history.navigate(true, "draft").as_deref(), Some("second"));
        assert_eq!(history.position(), Some((1, 2)));
        assert_eq!(history.navigate(true, "second").as_deref(), Some("first"));
        assert_eq!(history.navigate(true, "first").as_deref(), Some("first"));
        assert_eq!(history.navigate(false, "first").as_deref(), Some("second"));
        assert_eq!(history.navigate(false, "second").as_deref(), Some("draft"));
        assert_eq!(history.position(), None);
        assert_eq!(history.navigate(false, "draft"), None);
    }

    #[test]
    fn fallback_title_collapses_whitespace_and_truncates_at_a_word() {
        assert_eq!(fallback_title("  \n "), "New chat");
        assert_eq!(
            fallback_title("What  did\nwe decide?"),
            "What did we decide?"
        );
        let long = "Summarize every decision from the quarterly planning meeting with owners";
        let title = fallback_title(long);
        assert_eq!(title, "Summarize every decision from the quarterly...");
        assert!(title.chars().count() <= 50);
    }

    #[test]
    fn timestamps_use_relative_days_and_include_old_years() {
        let zone = FixedOffset::east_opt(9 * 3600).unwrap();
        let now = zone.with_ymd_and_hms(2026, 9, 1, 0, 30, 0).unwrap();
        let today = zone.with_ymd_and_hms(2026, 9, 1, 0, 15, 0).unwrap();
        let yesterday = zone.with_ymd_and_hms(2026, 8, 31, 23, 45, 0).unwrap();
        let older = zone.with_ymd_and_hms(2025, 3, 4, 18, 33, 0).unwrap();
        let utc = |at: DateTime<FixedOffset>| at.with_timezone(&Utc);
        assert_eq!(timestamp_label(utc(today), now, true), "Today 12:15 AM");
        assert_eq!(
            timestamp_label(utc(yesterday), now, true),
            "Yesterday 11:45 PM"
        );
        assert_eq!(
            timestamp_label(utc(older), now, true),
            "Mar 4, 2025 6:33 PM"
        );
        assert_eq!(timestamp_label(utc(older), now, false), "6:33 PM");
    }

    #[test]
    fn distance_matches_date_fns_buckets() {
        let now = Utc::now();
        let ago = |duration: Duration| distance_to_now(now - duration, now);
        assert_eq!(ago(Duration::seconds(10)), "less than a minute ago");
        assert_eq!(ago(Duration::minutes(5)), "5 minutes ago");
        assert_eq!(ago(Duration::minutes(50)), "about 1 hour ago");
        assert_eq!(ago(Duration::hours(3)), "about 3 hours ago");
        assert_eq!(ago(Duration::hours(30)), "1 day ago");
        assert_eq!(ago(Duration::days(6)), "6 days ago");
        assert_eq!(ago(Duration::days(40)), "about 1 month ago");
        assert_eq!(ago(Duration::days(400)), "about 1 year ago");
    }
}
