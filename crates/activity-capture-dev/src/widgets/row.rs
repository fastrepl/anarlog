use ratatui::text::{Line, Span};

use crate::{
    event_row::EventRow,
    formatting::{compact, format_timestamp},
    theme::Theme,
    ui::{APP_COLUMN_MAX_WIDTH, APP_COLUMN_MIN_WIDTH, APP_PREVIEW_LIMIT, STATUS_COLUMN_WIDTH},
};

pub(super) fn app_width(events: &[EventRow]) -> usize {
    events
        .iter()
        .map(|row| compact(&row.app_name, APP_PREVIEW_LIMIT).chars().count())
        .fold(APP_COLUMN_MIN_WIDTH, usize::max)
        .min(APP_COLUMN_MAX_WIDTH)
}

pub(super) fn row_line(row: &EventRow, app_width: usize, theme: Theme) -> Line<'static> {
    let app_name = compact(&row.app_name, APP_PREVIEW_LIMIT);
    let status = row.status.label();

    Line::from(vec![
        Span::styled(format_timestamp(row.captured_at), theme.timestamp()),
        Span::raw("  "),
        Span::styled(format!("{app_name:app_width$}"), theme.app(&row.app_name)),
        Span::raw("  "),
        Span::styled(
            format!("{status:STATUS_COLUMN_WIDTH$}"),
            theme.status(row.status),
        ),
        Span::raw("  "),
        Span::styled(format!("[{}]", row.context), theme.label()),
        Span::raw("  "),
        Span::raw(row.summary.clone()),
    ])
}

pub(super) fn selected_row(
    events: &[EventRow],
    selected_index: Option<usize>,
) -> Option<&EventRow> {
    selected_index.and_then(|index| events.get(index))
}
