use std::io::IsTerminal;
use std::time::Duration;

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::error::{CliError, CliResult};
use crate::tui::{InlineViewport, InputAction};

use super::{hotkey, service};

const HIGHLIGHT: Color = Color::Rgb(0xFD, 0xE6, 0xAE);

#[derive(Clone, Copy, PartialEq)]
enum DaemonStatus {
    Ready,
    Degraded,
    NotInstalled,
}

struct ScreenState {
    selected: usize,
    status: DaemonStatus,
    accessibility: bool,
    input_monitoring: bool,
    reason: Option<String>,
}

fn detect_status() -> (DaemonStatus, Option<String>) {
    let status = service::query();
    let local_blocker = hotkey::current_blocker();
    let reason = status
        .reason()
        .or_else(|| local_blocker.map(|error| error.message().to_string()));

    if status.running && local_blocker.is_none() {
        (DaemonStatus::Ready, None)
    } else if status.installed || status.bootstrapped {
        (DaemonStatus::Degraded, reason)
    } else {
        (DaemonStatus::NotInstalled, reason)
    }
}

fn check_accessibility() -> bool {
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }
    unsafe { AXIsProcessTrusted() }
}

fn check_input_monitoring() -> bool {
    hotkey::input_monitoring_granted()
}

fn render_lines(state: &ScreenState) -> Vec<Line<'static>> {
    let (status_dot, status_text, status_color) = match state.status {
        DaemonStatus::Ready => ("●", "Ready", Color::Green),
        DaemonStatus::Degraded => ("◐", "Degraded", Color::Yellow),
        DaemonStatus::NotInstalled => ("○", "Not installed", Color::DarkGray),
    };

    let acc_span = if state.accessibility {
        Span::styled("✓", Style::default().fg(Color::Green))
    } else {
        Span::styled("✗", Style::default().fg(Color::Red))
    };

    let input_span = if state.input_monitoring {
        Span::styled("✓", Style::default().fg(Color::Green))
    } else {
        Span::styled("✗", Style::default().fg(Color::Red))
    };

    let install_label = if state.status != DaemonStatus::NotInstalled {
        "Reinstall"
    } else {
        "Install"
    };

    let actions: &[&str] = &[install_label, "Uninstall"];

    let line0 = Line::from(vec![
        Span::styled(
            format!("{status_dot} {status_text}"),
            Style::default().fg(status_color),
        ),
        Span::raw("  "),
        Span::styled("input ", Style::default().fg(Color::DarkGray)),
        input_span,
        Span::raw("  "),
        Span::styled("access ", Style::default().fg(Color::DarkGray)),
        acc_span,
    ]);

    let mut action_spans = Vec::new();
    for (i, label) in actions.iter().enumerate() {
        let is_selected = i == state.selected;
        let is_disabled = i == 1 && state.status == DaemonStatus::NotInstalled;

        let marker = if is_selected { "> " } else { "  " };
        let style = if is_disabled {
            Style::default().fg(Color::Rgb(0x44, 0x44, 0x44))
        } else if is_selected {
            Style::default().fg(HIGHLIGHT)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        action_spans.push(Span::styled(format!("{marker}{label}"), style));
        if i < actions.len() - 1 {
            action_spans.push(Span::raw("  "));
        }
    }

    let line1 = Line::from(vec![
        Span::styled("reason ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.reason.clone().unwrap_or_else(|| "none".to_string()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let line2 = Line::from(vec![
        Span::styled("hotkey ", Style::default().fg(Color::DarkGray)),
        Span::styled("⌥⌥", Style::default().fg(Color::White)),
        Span::styled(" double-tap", Style::default().fg(Color::DarkGray)),
    ]);

    let line3 = Line::from(action_spans);

    vec![line0, line1, line2, line3]
}

pub fn run() -> CliResult<()> {
    if !std::io::stderr().is_terminal() {
        return Err(CliError::operation_failed(
            "shortcut",
            "interactive mode requires a terminal; use `char shortcut install` or `char shortcut status`".to_string(),
        ));
    }

    let (status, reason) = detect_status();
    let mut state = ScreenState {
        selected: 0,
        status,
        accessibility: check_accessibility(),
        input_monitoring: check_input_monitoring(),
        reason,
    };

    let mut viewport = InlineViewport::stderr_interactive(6, None, true)
        .map_err(|e| CliError::operation_failed("create viewport", e.to_string()))?;

    viewport.draw(&render_lines(&state));

    let mut refresh_counter: u32 = 0;

    loop {
        std::thread::sleep(Duration::from_millis(30));

        // Refresh permission/status every ~1s (33 * 30ms)
        refresh_counter += 1;
        if refresh_counter >= 33 {
            refresh_counter = 0;
            let (status, reason) = detect_status();
            state.status = status;
            state.accessibility = check_accessibility();
            state.input_monitoring = check_input_monitoring();
            state.reason = reason;
        }

        for action in viewport.poll_input() {
            match action {
                InputAction::Up | InputAction::Down => {
                    state.selected = if state.selected == 0 { 1 } else { 0 };
                }
                InputAction::Confirm => {
                    viewport
                        .clear()
                        .map_err(|e| CliError::operation_failed("clear viewport", e.to_string()))?;

                    match state.selected {
                        0 => return super::install::run(),
                        1 => {
                            if state.status == DaemonStatus::NotInstalled {
                                continue;
                            }
                            return super::uninstall::run();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        viewport.draw(&render_lines(&state));
    }
}
