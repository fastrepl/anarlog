use hypr_listener_core::State;
use hypr_transcript::WordState;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Position, Rect},
    text::{Line, Span},
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use super::app::App;
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = Theme::default();
    let [content_area, status_area] =
        Layout::vertical([Constraint::Min(8), Constraint::Length(1)]).areas(frame.area());

    let [notepad_area, sidebar_area] = Layout::horizontal([
        Constraint::Percentage(app.notepad_width_percent()),
        Constraint::Percentage(100 - app.notepad_width_percent()),
    ])
    .areas(content_area);

    draw_notepad(frame, app, notepad_area, &theme);
    draw_sidebar(frame, app, sidebar_area, &theme);
    draw_status_bar(frame, app, status_area, &theme);
}

fn draw_sidebar(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let [metadata_area, meters_area, transcript_area] = Layout::vertical([
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(3),
    ])
    .areas(area);

    draw_sidebar_metadata(frame, app, metadata_area, theme);
    draw_sidebar_meters(frame, app, meters_area, theme);
    draw_transcript(frame, app, transcript_area, theme);
}

fn draw_sidebar_metadata(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let elapsed = app.elapsed();
    let secs = elapsed.as_secs();
    let time_str = format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    );

    let state_style = match app.state {
        State::Active if app.degraded.is_some() => theme.status_degraded,
        State::Active => theme.status_active,
        State::Finalizing => theme.status_degraded,
        State::Inactive => theme.status_inactive,
    };

    let mut lines = vec![
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(&app.status, state_style),
        ]),
        Line::from(format!("Time: {time_str}")),
        Line::from(format!("Words: {}", app.words.len())),
    ];

    if let Some(err) = app.errors.last() {
        lines.push(Line::default());
        lines.push(Line::from(vec![Span::styled("Last error", theme.error)]));
        lines.push(Line::from(vec![Span::styled(err, theme.muted)]));
    }

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Session ")
        .padding(Padding::new(1, 1, 0, 0));

    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_sidebar_meters(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(theme.border)
        .title(" Audio ")
        .padding(Padding::new(1, 1, 0, 0));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let horizontal_padding = inner.width.min(2) as usize;
    let waveform_width = (inner.width as usize).saturating_sub(horizontal_padding * 2);
    if waveform_width == 0 {
        return;
    }

    let active_width = ((waveform_width as f64) * 0.78).round() as usize;
    let active_width = active_width.clamp(8, waveform_width);
    let side_gutter = (waveform_width.saturating_sub(active_width)) / 2;
    let waveform = build_waveform_spans(app, active_width, theme);

    let mut lines = (0..inner.height)
        .map(|_| Line::from(" ".repeat(inner.width as usize)))
        .collect::<Vec<_>>();

    let middle_row = (inner.height / 2) as usize;
    let mut middle_spans = Vec::with_capacity(3);
    middle_spans.push(Span::raw(" ".repeat(horizontal_padding)));
    middle_spans.push(Span::raw(" ".repeat(side_gutter)));
    middle_spans.extend(waveform);
    middle_spans.push(Span::raw(
        " ".repeat(waveform_width.saturating_sub(active_width + side_gutter)),
    ));
    middle_spans.push(Span::raw(" ".repeat(horizontal_padding)));
    lines[middle_row] = Line::from(middle_spans);

    if app.mic_muted {
        lines[0] = Line::from(Span::styled("mic muted", theme.muted));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn build_waveform_spans(app: &App, width: usize, theme: &Theme) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let mic = app.mic_history.iter().copied().collect::<Vec<_>>();
    let speaker = app.speaker_history.iter().copied().collect::<Vec<_>>();
    let sample_count = mic.len().max(speaker.len());

    if sample_count == 0 {
        return vec![Span::raw(" ".repeat(width))];
    }

    let mut combined = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let mic_value = aligned_sample(&mic, sample_count, i);
        let speaker_value = aligned_sample(&speaker, sample_count, i);
        combined.push(mic_value.max(speaker_value).min(1000));
    }

    let mut spans = Vec::with_capacity(width);
    for x in 0..width {
        let raw_value = column_energy(&combined, x, width) as f64;
        let envelope = edge_envelope(x, width);
        let value = (raw_value * envelope).round() as u64;
        let mut style = theme.waveform_normal;

        let normalized = (value as f64 / 1000.0).clamp(0.0, 1.0);
        let level = if value == 0 {
            0
        } else {
            (normalized.powf(0.55) * 8.0).round().clamp(1.0, 8.0) as u8
        };

        if level == 0 || envelope < 0.22 {
            style = theme.waveform_silent;
        } else if level >= 6 && envelope > 0.7 {
            style = theme.waveform_hot;
        }

        if app.mic_muted {
            style = theme.waveform_silent;
        }

        spans.push(Span::styled(level_char(level).to_string(), style));
    }

    spans
}

fn edge_envelope(x: usize, width: usize) -> f64 {
    if width <= 1 {
        return 1.0;
    }

    let center = (width - 1) as f64 / 2.0;
    let distance = ((x as f64) - center).abs() / center.max(1.0);
    if distance <= 0.62 {
        return 1.0;
    }

    let t = ((distance - 0.62) / 0.38).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - smooth
}

fn column_energy(values: &[u64], x: usize, width: usize) -> u64 {
    if values.is_empty() || width == 0 {
        return 0;
    }

    let sample_count = values.len();
    let start = x * sample_count / width;
    let mut end = (x + 1) * sample_count / width;
    if end <= start {
        end = (start + 1).min(sample_count);
    }

    let mut max_value = 0u64;
    let mut sum = 0u64;
    let mut count = 0u64;

    for value in &values[start..end] {
        max_value = max_value.max(*value);
        sum += *value;
        count += 1;
    }

    let avg = if count == 0 { 0 } else { sum / count };
    let raw = (max_value * 7 + avg * 3) / 10;

    let left = if start > 0 {
        values[start - 1]
    } else {
        values[start]
    };
    let right = if end < sample_count {
        values[end]
    } else {
        values[sample_count - 1]
    };

    ((raw * 6) + left + right) / 8
}

fn aligned_sample(values: &[u64], sample_count: usize, index: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let offset = sample_count.saturating_sub(values.len());
    if index < offset {
        0
    } else {
        values[index - offset]
    }
}

fn level_char(level: u8) -> char {
    match level {
        0 => ' ',
        1 => '\u{2581}',
        2 => '\u{2582}',
        3 => '\u{2583}',
        4 => '\u{2584}',
        5 => '\u{2585}',
        6 => '\u{2586}',
        7 => '\u{2587}',
        _ => '\u{2588}',
    }
}

fn draw_transcript(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let mut spans: Vec<Span> = Vec::new();
    let mut transcript_text = String::new();

    for word in &app.words {
        let style = match word.state {
            WordState::Final => theme.transcript_final,
            WordState::Pending => theme.transcript_pending,
        };
        spans.push(Span::styled(word.text.clone(), style));
        spans.push(Span::raw(" "));
        transcript_text.push_str(&word.text);
        transcript_text.push(' ');
    }

    if !app.partials.is_empty() {
        for partial in &app.partials {
            spans.push(Span::styled(partial.text.clone(), theme.transcript_partial));
            spans.push(Span::raw(" "));
            transcript_text.push_str(&partial.text);
            transcript_text.push(' ');
        }
    }

    if spans.is_empty() {
        let empty_message = if app.can_accept_audio_drop() {
            "Drop an audio file to transcribe..."
        } else {
            "Waiting for speech..."
        };
        spans.push(Span::styled(empty_message, theme.placeholder));
        transcript_text.push_str(empty_message);
    }

    let text = vec![Line::from(spans)];

    let border_style = if app.transcript_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Transcript ")
        .padding(Padding::new(1, 1, 0, 0));

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });

    let visible_lines = area.height.saturating_sub(2) as usize;
    let content_width = area.width.saturating_sub(4) as usize;
    let line_count = wrapped_line_count(&transcript_text, content_width);
    let max_scroll = line_count
        .saturating_sub(visible_lines)
        .min(u16::MAX as usize) as u16;
    app.update_transcript_max_scroll(max_scroll);

    let paragraph = paragraph.scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);

    let mut scrollbar_state =
        ScrollbarState::new(line_count.max(1)).position(app.scroll_offset as usize);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    frame.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

fn draw_notepad(frame: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    let border_style = if app.memo_focused() {
        theme.border_focused
    } else {
        theme.border
    };

    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Notepad ");

    let view = app.memo_view(
        area.height.saturating_sub(2) as usize,
        area.width.saturating_sub(2) as usize,
    );

    let lines = if app.memo_is_empty() && !app.memo_focused() {
        vec![Line::from(vec![Span::styled(
            "press [m] to start writing notes...",
            theme.placeholder,
        )])]
    } else {
        view.lines.into_iter().map(Line::from).collect::<Vec<_>>()
    };

    frame.render_widget(Paragraph::new(lines).block(block), area);

    if app.memo_focused() {
        let inner_max_x = area.x + area.width.saturating_sub(2);
        let inner_max_y = area.y + area.height.saturating_sub(2);
        let x = (area.x + 1 + view.cursor_col).min(inner_max_x);
        let y = (area.y + 1 + view.cursor_row).min(inner_max_y);
        frame.set_cursor_position(Position { x, y });
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let word_count = app.words.len();

    let line = if app.memo_focused() {
        Line::from(vec![
            Span::styled(" [esc]", theme.shortcut_key),
            Span::raw(" transcript  "),
            Span::styled("[tab]", theme.shortcut_key),
            Span::raw(" toggle  "),
            Span::styled("[ctrl+left/right]", theme.shortcut_key),
            Span::raw(" panes  "),
            Span::styled("[ctrl+u]", theme.shortcut_key),
            Span::raw(" clear note  "),
            Span::styled("[ctrl+c]", theme.shortcut_key),
            Span::raw(" quit  "),
        ])
    } else {
        Line::from(vec![
            Span::styled(" [q]", theme.shortcut_key),
            Span::raw(" quit  "),
            Span::styled("[j/k]", theme.shortcut_key),
            Span::raw(" transcript  "),
            Span::styled("[m]", theme.shortcut_key),
            Span::raw(" notepad  "),
            Span::styled("[tab]", theme.shortcut_key),
            Span::raw(" toggle  "),
            Span::styled("[ctrl+left/right]", theme.shortcut_key),
            Span::raw(" panes  "),
            Span::styled(format!("{word_count} words"), theme.muted),
        ])
    };

    frame.render_widget(Paragraph::new(line), area);
}

fn wrapped_line_count(text: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }

    let mut lines = 0usize;
    for line in text.split('\n') {
        let chars = line.chars().count().max(1);
        lines += chars.div_ceil(width);
    }

    lines.max(1)
}
