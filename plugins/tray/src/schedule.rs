const DISPLAY_HORIZON_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
const MAX_TITLE_CHARS: usize = 24;

#[derive(Debug, Clone, serde::Deserialize, specta::Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrayScheduleEvent {
    pub title: String,
    pub starts_at_ms: f64,
    pub ends_at_ms: Option<f64>,
    pub day_label: Option<String>,
    pub time_label: String,
}

#[derive(Debug, PartialEq)]
pub struct TrayAgendaSection {
    pub label: String,
    pub events: Vec<String>,
}

pub fn agenda_sections(
    events: &[TrayScheduleEvent],
    now_ms: f64,
    show_events: bool,
) -> Vec<TrayAgendaSection> {
    if !show_events {
        return Vec::new();
    }

    let mut sections: Vec<TrayAgendaSection> = Vec::new();

    for event in events
        .iter()
        .filter(|event| {
            event.day_label.is_some()
                && event
                    .ends_at_ms
                    .map_or(event.starts_at_ms > now_ms, |end_ms| end_ms > now_ms)
        })
        .take(3)
    {
        let label = event.day_label.as_ref().unwrap();
        if sections
            .last()
            .is_none_or(|section| section.label != *label)
        {
            sections.push(TrayAgendaSection {
                label: label.clone(),
                events: Vec::new(),
            });
        }

        sections.last_mut().unwrap().events.push(format!(
            "{}  {}",
            event.time_label,
            compact_agenda_title(&event.title)
        ));
    }

    sections
}

pub fn menu_bar_title(
    events: &[TrayScheduleEvent],
    now_ms: f64,
    show_events: bool,
) -> Option<String> {
    if !show_events {
        return None;
    }

    let active = events
        .iter()
        .filter(|event| {
            event.starts_at_ms.is_finite()
                && event.starts_at_ms <= now_ms
                && event
                    .ends_at_ms
                    .is_some_and(|end_ms| end_ms.is_finite() && end_ms > now_ms)
        })
        .min_by(|left, right| {
            left.ends_at_ms
                .partial_cmp(&right.ends_at_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some(event) = active {
        return Some(format!(
            "{} • {} left",
            compact_title(&event.title),
            duration_label(event.ends_at_ms.unwrap_or(now_ms) - now_ms)
        ));
    }

    let event = events
        .iter()
        .filter(|event| {
            event.starts_at_ms.is_finite()
                && event.starts_at_ms > now_ms
                && event.starts_at_ms - now_ms <= DISPLAY_HORIZON_MS
        })
        .min_by(|left, right| {
            left.starts_at_ms
                .partial_cmp(&right.starts_at_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;

    Some(format!(
        "{} • in {}",
        compact_title(&event.title),
        duration_label(event.starts_at_ms - now_ms)
    ))
}

fn compact_title(title: &str) -> String {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = if title.is_empty() {
        "Untitled event".to_string()
    } else {
        title
    };

    if title.chars().count() <= MAX_TITLE_CHARS {
        return title;
    }

    format!(
        "{}…",
        title.chars().take(MAX_TITLE_CHARS - 1).collect::<String>()
    )
}

fn compact_agenda_title(title: &str) -> String {
    const MAX_AGENDA_TITLE_CHARS: usize = 32;
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = if title.is_empty() {
        "Untitled event".to_string()
    } else {
        title
    };

    if title.chars().count() <= MAX_AGENDA_TITLE_CHARS {
        return title;
    }

    format!(
        "{}…",
        title
            .chars()
            .take(MAX_AGENDA_TITLE_CHARS - 1)
            .collect::<String>()
    )
}

fn duration_label(diff_ms: f64) -> String {
    let total_seconds = (diff_ms / 1000.0).floor().max(1.0) as u64;

    if total_seconds < 60 {
        return format!("{total_seconds}s");
    }

    let total_minutes = total_seconds / 60;
    if total_minutes < 60 {
        return format!("{total_minutes}m");
    }

    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if minutes == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(title: &str, starts_at_ms: f64, ends_at_ms: Option<f64>) -> TrayScheduleEvent {
        TrayScheduleEvent {
            title: title.to_string(),
            starts_at_ms,
            ends_at_ms,
            day_label: Some("Today".to_string()),
            time_label: "9:00 AM – 9:30 AM".to_string(),
        }
    }

    #[test]
    fn shows_the_nearest_event_within_a_day() {
        let now = 1_000_000.0;
        let events = vec![
            event("Tomorrow", now + DISPLAY_HORIZON_MS + 1.0, None),
            event("Design sync", now + 17.0 * 60.0 * 1000.0, None),
            event("Standup", now + 5.0 * 60.0 * 1000.0, None),
        ];

        assert_eq!(
            menu_bar_title(&events, now, true),
            Some("Standup • in 5m".to_string())
        );
    }

    #[test]
    fn shows_an_active_event_before_the_next_event() {
        let now = 1_000_000.0;
        let events = vec![
            event(
                "Active meeting",
                now - 1_000.0,
                Some(now + (10.0 * 60.0 + 59.0) * 1000.0),
            ),
            event("Next meeting", now + 30_000.0, None),
        ];

        assert_eq!(
            menu_bar_title(&events, now, true),
            Some("Active meeting • 10m left".to_string())
        );
    }

    #[test]
    fn formats_long_titles_and_countdowns_compactly() {
        let now = 1_000_000.0;
        let events = vec![event(
            "  Sprint   retrospective and planning  ",
            now + (17.0 * 60.0 + 20.0) * 60.0 * 1000.0,
            None,
        )];

        assert_eq!(
            menu_bar_title(&events, now, true),
            Some("Sprint retrospective an… • in 17h 20m".to_string())
        );
    }

    #[test]
    fn clears_the_title_without_an_active_or_upcoming_event() {
        let now = 1_000_000.0;
        let events = vec![event("Finished", now - 60_000.0, Some(now - 1.0))];

        assert_eq!(menu_bar_title(&events, now, true), None);
    }

    #[test]
    fn hides_events_when_the_menu_option_is_disabled() {
        let now = 1_000_000.0;
        let events = vec![event("Standup", now + 5.0 * 60.0 * 1000.0, None)];

        assert_eq!(menu_bar_title(&events, now, false), None);
    }

    #[test]
    fn groups_at_most_three_remaining_events_for_today_and_tomorrow() {
        let now = 1_000_000.0;
        let mut events = vec![
            event("Active", now - 1_000.0, Some(now + 60_000.0)),
            event("Finished", now - 60_000.0, Some(now - 1.0)),
            event("Next", now + 60_000.0, Some(now + 120_000.0)),
            event("Tomorrow one", now + 120_000.0, Some(now + 180_000.0)),
            event("Tomorrow two", now + 180_000.0, Some(now + 240_000.0)),
        ];
        events[2].day_label = Some("Tomorrow".to_string());
        events[3].day_label = Some("Tomorrow".to_string());
        events[4].day_label = Some("Tomorrow".to_string());

        assert_eq!(
            agenda_sections(&events, now, true),
            vec![
                TrayAgendaSection {
                    label: "Today".to_string(),
                    events: vec!["9:00 AM – 9:30 AM  Active".to_string()],
                },
                TrayAgendaSection {
                    label: "Tomorrow".to_string(),
                    events: vec![
                        "9:00 AM – 9:30 AM  Next".to_string(),
                        "9:00 AM – 9:30 AM  Tomorrow one".to_string(),
                    ],
                },
            ]
        );
    }
}
