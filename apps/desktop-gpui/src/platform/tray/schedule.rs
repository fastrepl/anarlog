//! Shipping's tray agenda and menu-bar title (`plugins/tray/src/schedule.rs`).

use std::sync::Arc;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const DISPLAY_HORIZON_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
const MAX_AGENDA_LABEL_WIDTH: usize = 24;
const MAX_MENU_BAR_LABEL_WIDTH: usize = 30;

/// One timed calendar event published to the tray (`TrayScheduleEvent`).
#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleEvent {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub meeting_link: Option<Arc<str>>,
    pub starts_at_ms: f64,
    pub ends_at_ms: Option<f64>,
    /// Local midnight of the event's day, and of the day before it.
    pub day_start_ms: f64,
    pub previous_day_start_ms: f64,
    pub time_label: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgendaEvent {
    pub id: Arc<str>,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgendaSection {
    pub label: &'static str,
    pub events: Vec<AgendaEvent>,
}

/// At most three remaining events, grouped under "Today" and "Tomorrow".
pub fn agenda_sections(
    events: &[ScheduleEvent],
    now_ms: f64,
    show_events: bool,
) -> Vec<AgendaSection> {
    if !show_events {
        return Vec::new();
    }
    let mut sections: Vec<AgendaSection> = Vec::new();
    for (event, label) in events
        .iter()
        .filter(|event| {
            event
                .ends_at_ms
                .map_or(event.starts_at_ms > now_ms, |end| end > now_ms)
        })
        .filter_map(|event| day_label(event, now_ms).map(|label| (event, label)))
        .take(3)
    {
        if sections.last().is_none_or(|section| section.label != label) {
            sections.push(AgendaSection {
                label,
                events: Vec::new(),
            });
        }
        if let Some(section) = sections.last_mut() {
            section.events.push(AgendaEvent {
                id: event.id.clone(),
                label: agenda_label(event),
            });
        }
    }
    sections
}

fn day_label(event: &ScheduleEvent, now_ms: f64) -> Option<&'static str> {
    if !event.day_start_ms.is_finite() || !event.previous_day_start_ms.is_finite() {
        return None;
    }
    if now_ms >= event.day_start_ms {
        Some("Today")
    } else if now_ms >= event.previous_day_start_ms {
        Some("Tomorrow")
    } else {
        None
    }
}

/// The text beside the tray icon: the recording's title, else the active or next event.
pub fn menu_bar_title(
    events: &[ScheduleEvent],
    now_ms: f64,
    show_events: bool,
    recording: bool,
    recording_title: Option<&str>,
) -> Option<String> {
    if recording {
        let title = recording_title?.trim();
        return (!title.is_empty()).then(|| compact(title, MAX_MENU_BAR_LABEL_WIDTH));
    }
    if !show_events {
        return None;
    }
    if let Some(event) = active_event(events, now_ms) {
        let suffix = format!(
            " • {} left",
            duration(event.ends_at_ms.unwrap_or(now_ms) - now_ms)
        );
        return Some(bar_label(&event.title, &suffix));
    }
    let event = upcoming_event(events, now_ms)?;
    let suffix = format!(" • in {}", duration(event.starts_at_ms - now_ms));
    Some(bar_label(&event.title, &suffix))
}

fn active_event(events: &[ScheduleEvent], now_ms: f64) -> Option<&ScheduleEvent> {
    events
        .iter()
        .filter(|event| {
            event.starts_at_ms.is_finite()
                && event.starts_at_ms <= now_ms
                && event
                    .ends_at_ms
                    .is_some_and(|end| end.is_finite() && end > now_ms)
        })
        .min_by(|a, b| {
            a.ends_at_ms
                .unwrap_or(0.)
                .total_cmp(&b.ends_at_ms.unwrap_or(0.))
        })
}

fn upcoming_event(events: &[ScheduleEvent], now_ms: f64) -> Option<&ScheduleEvent> {
    events
        .iter()
        .filter(|event| {
            event.starts_at_ms.is_finite()
                && event.starts_at_ms > now_ms
                && event.starts_at_ms - now_ms <= DISPLAY_HORIZON_MS
        })
        .min_by(|a, b| a.starts_at_ms.total_cmp(&b.starts_at_ms))
}

fn bar_label(title: &str, suffix: &str) -> String {
    let limit = MAX_MENU_BAR_LABEL_WIDTH.saturating_sub(suffix.width());
    format!("{}{suffix}", compact(title, limit))
}

fn compact(title: &str, max_width: usize) -> String {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = if title.is_empty() {
        "Untitled event".to_string()
    } else {
        title
    };
    if title.width() <= max_width {
        return title;
    }
    if max_width <= 1 {
        return "…".into();
    }
    let mut compact = String::new();
    let mut width = 0;
    for character in title.chars() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width >= max_width {
            break;
        }
        compact.push(character);
        width += character_width;
    }
    format!("{compact}…")
}

fn agenda_label(event: &ScheduleEvent) -> String {
    let start = event
        .time_label
        .split('–')
        .next()
        .unwrap_or_default()
        .trim();
    let suffix = format!(" · {start}");
    let limit = MAX_AGENDA_LABEL_WIDTH.saturating_sub(suffix.width());
    format!("{}{suffix}", compact(&event.title, limit))
}

fn duration(diff_ms: f64) -> String {
    let seconds = (diff_ms / 1000.0).floor().max(1.0) as u64;
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    match (minutes / 60, minutes % 60) {
        (hours, 0) => format!("{hours}h"),
        (hours, minutes) => format!("{hours}h {minutes}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(title: &str, starts_at_ms: f64, ends_at_ms: Option<f64>) -> ScheduleEvent {
        ScheduleEvent {
            id: title.to_lowercase().replace(' ', "-").into(),
            title: title.into(),
            meeting_link: None,
            starts_at_ms,
            ends_at_ms,
            day_start_ms: 0.0,
            previous_day_start_ms: -86_400_000.0,
            time_label: "9:00 AM – 9:30 AM".into(),
        }
    }

    #[test]
    fn menu_bar_counts_down_the_active_then_the_nearest_event_and_recording_replaces_it() {
        let now = 1_000_000.0;
        let upcoming = vec![
            event("Tomorrow", now + DISPLAY_HORIZON_MS + 1.0, None),
            event("Design sync", now + 17.0 * 60.0 * 1000.0, None),
            event("Standup", now + 5.0 * 60.0 * 1000.0, None),
        ];
        assert_eq!(
            menu_bar_title(&upcoming, now, true, false, None).as_deref(),
            Some("Standup • in 5m")
        );
        let active = vec![
            event(
                "Active meeting",
                now - 1_000.0,
                Some(now + (10.0 * 60.0 + 59.0) * 1000.0),
            ),
            event("Next meeting", now + 30_000.0, None),
        ];
        assert_eq!(
            menu_bar_title(&active, now, true, false, None).as_deref(),
            Some("Active meeting • 10m left")
        );
        assert_eq!(menu_bar_title(&upcoming, now, false, false, None), None);
        assert_eq!(
            menu_bar_title(&upcoming, now, false, true, Some("Customer call")).as_deref(),
            Some("Customer call")
        );
        assert_eq!(menu_bar_title(&upcoming, now, true, true, Some("  ")), None);
    }

    #[test]
    fn menu_bar_titles_are_capped_by_display_width() {
        let now = 1_000_000.0;
        for (title, starts_in_ms, expected) in [
            (
                "  Sprint   retrospective and planning  ",
                (17.0 * 60.0 + 20.0) * 60.0 * 1000.0,
                "Sprint retrospec… • in 17h 20m",
            ),
            (
                "[실뻘한] char 정치현 대표님 컨콜",
                29.0 * 60.0 * 1000.0,
                "[실뻘한] char 정치현… • in 29m",
            ),
        ] {
            let label = menu_bar_title(
                &[event(title, now + starts_in_ms, None)],
                now,
                true,
                false,
                None,
            )
            .unwrap();
            assert_eq!(label, expected);
            assert_eq!(label.width(), MAX_MENU_BAR_LABEL_WIDTH);
        }
    }

    #[test]
    fn agenda_groups_at_most_three_remaining_events_and_relabels_after_midnight() {
        let now = 1_000_000.0;
        let mut events = vec![
            event("Active", now - 1_000.0, Some(now + 60_000.0)),
            event("Finished", now - 60_000.0, Some(now - 1.0)),
            event("Next", now + 60_000.0, Some(now + 120_000.0)),
            event("Tomorrow one", now + 120_000.0, Some(now + 180_000.0)),
            event("Tomorrow two", now + 180_000.0, Some(now + 240_000.0)),
        ];
        for event in &mut events[2..] {
            event.day_start_ms = now + 1.0;
            event.previous_day_start_ms = now - 86_400_000.0;
        }
        let sections = agenda_sections(&events, now, true);
        let labels = sections
            .iter()
            .map(|section| {
                (
                    section.label,
                    section
                        .events
                        .iter()
                        .map(|event| event.label.as_str())
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                ("Today", vec!["Active · 9:00 AM"]),
                ("Tomorrow", vec!["Next · 9:00 AM", "Tomorrow one · 9:00 AM"]),
            ]
        );
        assert!(agenda_sections(&events, now, false).is_empty());
        assert_eq!(
            agenda_sections(&events[2..3], now + 1.0, true)[0].label,
            "Today"
        );
    }
}
