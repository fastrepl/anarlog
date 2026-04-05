use std::time::SystemTime;

use hypr_activity_capture::{Event, Snapshot, Transition};

use crate::{
    formatting::{compact, compact_url, format_timestamp},
    styles::Styles,
};

const APP_PREVIEW_LIMIT: usize = 18;
const APP_COLUMN_MIN_WIDTH: usize = 14;
const TITLE_PREVIEW_LIMIT: usize = 56;
const URL_PREVIEW_LIMIT: usize = 72;
const TEXT_PREVIEW_LIMIT: usize = 40;
const DIFF_PREVIEW_LIMIT: usize = 28;

#[derive(Debug, Clone)]
pub struct DisplayState {
    styles: Styles,
    app_width: usize,
}

impl DisplayState {
    pub fn new(styles: Styles) -> Self {
        Self {
            styles,
            app_width: APP_COLUMN_MIN_WIDTH,
        }
    }

    pub fn print_transition(&mut self, transition: &Transition) {
        match (transition.previous.as_ref(), transition.current.as_ref()) {
            (None, Some(current)) => {
                self.print_focus(current.snapshot.captured_at, &current.snapshot, None)
            }
            (Some(previous), Some(current)) if same_app(previous, current) => {
                self.print_update(&previous.snapshot, &current.snapshot);
            }
            (Some(previous), Some(current)) => self.print_focus(
                current.snapshot.captured_at,
                &current.snapshot,
                Some(previous.snapshot.app_name.as_str()),
            ),
            (Some(previous), None) => {
                self.print_idle(
                    previous.snapshot.captured_at,
                    Some(previous.snapshot.app_name.as_str()),
                );
            }
            (None, None) => {}
        }
    }

    pub fn print_focus(
        &mut self,
        captured_at: SystemTime,
        snapshot: &Snapshot,
        previous_app: Option<&str>,
    ) {
        let details = focus_details(snapshot, previous_app);
        self.print_line(captured_at, &snapshot.app_name, "focus", &details);
    }

    pub fn print_idle(&mut self, captured_at: SystemTime, previous_app: Option<&str>) {
        let details = previous_app
            .map(|value| format!("from={}", compact(value, APP_PREVIEW_LIMIT)))
            .unwrap_or_else(|| "-".to_string());
        self.print_line(captured_at, "-", "idle", &details);
    }

    fn print_update(&mut self, previous: &Snapshot, current: &Snapshot) {
        let details = update_details(previous, current);
        self.print_line(current.captured_at, &current.app_name, "update", &details);
    }

    fn print_line(&mut self, captured_at: SystemTime, app_name: &str, status: &str, details: &str) {
        let app_name = compact(app_name, APP_PREVIEW_LIMIT);
        self.app_width = self.app_width.max(app_name.chars().count());

        println!(
            "{}  {}  {}  {}",
            format_timestamp(captured_at),
            self.styles.app(&app_name, self.app_width),
            self.styles.status(status),
            details,
        );
    }
}

fn same_app(previous: &Event, current: &Event) -> bool {
    previous.snapshot.bundle_id == current.snapshot.bundle_id
        || previous.snapshot.app_name == current.snapshot.app_name
}

fn focus_details(snapshot: &Snapshot, previous_app: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(previous_app) = previous_app {
        parts.push(format!("from={}", compact(previous_app, APP_PREVIEW_LIMIT)));
    }
    if let Some(title) = snapshot.window_title.as_deref() {
        parts.push(format!("title={}", display_value("title", title)));
    }
    if let Some(url) = snapshot.url.as_deref() {
        parts.push(format!("url={}", display_value("url", url)));
    } else if let Some(text) = primary_text(snapshot) {
        parts.push(format!("text={}", display_value("text", text)));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join("  ")
    }
}

fn update_details(previous: &Snapshot, current: &Snapshot) -> String {
    let mut changes = Vec::new();

    push_change(
        &mut changes,
        "title",
        previous.window_title.as_deref(),
        current.window_title.as_deref(),
    );
    push_change(
        &mut changes,
        "url",
        previous.url.as_deref(),
        current.url.as_deref(),
    );
    push_change(
        &mut changes,
        "text",
        primary_text(previous),
        primary_text(current),
    );

    if changes.is_empty() {
        "metadata changed".to_string()
    } else {
        changes.join("  ")
    }
}

fn primary_text(snapshot: &Snapshot) -> Option<&str> {
    snapshot
        .text_anchor_text
        .as_deref()
        .or(snapshot.text_anchor_selected_text.as_deref())
        .or(snapshot.visible_text.as_deref())
}

fn push_change(
    changes: &mut Vec<String>,
    label: &'static str,
    before: Option<&str>,
    after: Option<&str>,
) {
    let before_display = before.map(|value| diff_value(label, value));
    let after_display = after.map(|value| diff_value(label, value));

    if before_display == after_display {
        return;
    }

    match (before_display, after_display) {
        (Some(before), Some(after)) => changes.push(format!("{label}:{before} -> {after}")),
        (None, Some(after)) => changes.push(format!("{label}:+{after}")),
        (Some(before), None) => changes.push(format!("{label}:-{before}")),
        (None, None) => {}
    }
}

fn display_value(label: &str, value: &str) -> String {
    match label {
        "url" => compact_url(value, URL_PREVIEW_LIMIT),
        "text" => compact(value, TEXT_PREVIEW_LIMIT),
        _ => compact(value, TITLE_PREVIEW_LIMIT),
    }
}

fn diff_value(label: &str, value: &str) -> String {
    match label {
        "url" => compact_url(value, DIFF_PREVIEW_LIMIT * 2),
        _ => compact(value, DIFF_PREVIEW_LIMIT),
    }
}
