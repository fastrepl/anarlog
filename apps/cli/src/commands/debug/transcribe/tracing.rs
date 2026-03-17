use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tracing_subscriber::EnvFilter;

const DEFAULT_CAP: usize = 2000;

pub struct TracingCapture {
    lines: Mutex<VecDeque<String>>,
}

impl TracingCapture {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            lines: Mutex::new(VecDeque::with_capacity(DEFAULT_CAP)),
        })
    }

    fn push(&self, line: String) {
        let mut lines = self.lines.lock().unwrap();
        if lines.len() >= DEFAULT_CAP {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    pub fn drain_lines(&self) -> Vec<Line<'static>> {
        let mut lines = self.lines.lock().unwrap();
        lines.drain(..).map(|s| stylize_log_line(&s)).collect()
    }
}

fn stylize_log_line(raw: &str) -> Line<'static> {
    // tracing_subscriber fmt output: "2026-03-16T09:45:03.056609Z  INFO target: message fields"
    // We want: "09:45:03  INFO target: message fields" with colors.

    let mut rest = raw;
    let mut spans = Vec::new();

    // Parse timestamp — look for the 'Z' or ' ' that ends it.
    // ISO 8601: "2026-03-16T09:45:03.056609Z"
    if rest.len() > 20 && rest.as_bytes().get(10) == Some(&b'T') {
        if let Some(ts_end) = rest.find('Z') {
            let full_ts = &rest[..ts_end + 1];
            // Extract just HH:MM:SS from "2026-03-16T09:45:03.056609Z"
            let time_part = if let Some(t_pos) = full_ts.find('T') {
                let after_t = &full_ts[t_pos + 1..];
                // Take HH:MM:SS (8 chars), skip fractional seconds
                if after_t.len() >= 8 {
                    &after_t[..8]
                } else {
                    after_t
                }
            } else {
                full_ts
            };
            spans.push(Span::styled(
                time_part.to_string(),
                Style::new().fg(Color::DarkGray),
            ));
            rest = &rest[ts_end + 1..];
        }
    }

    // Skip whitespace between timestamp and level
    let trimmed = rest.trim_start();
    let skipped = rest.len() - trimmed.len();
    if skipped > 0 {
        spans.push(Span::raw(" "));
        rest = trimmed;
    }

    // Parse level: INFO, WARN, ERROR, DEBUG, TRACE
    let levels = ["ERROR", "WARN", "INFO", "DEBUG", "TRACE"];
    let mut found_level = false;
    for level in &levels {
        if rest.starts_with(level) {
            let style = match *level {
                "ERROR" => Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                "WARN" => Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                "INFO" => Style::new().fg(Color::Green),
                "DEBUG" => Style::new().fg(Color::Blue),
                "TRACE" => Style::new().fg(Color::DarkGray),
                _ => Style::new(),
            };
            spans.push(Span::styled(level.to_string(), style));
            rest = &rest[level.len()..];
            found_level = true;
            break;
        }
    }

    if !found_level {
        // Not a standard log line, return as-is
        spans.push(Span::raw(rest.to_string()));
        return Line::from(spans);
    }

    // Parse " target: message"
    let rest_str = rest.to_string();
    if let Some(colon_pos) = rest_str.find(':') {
        let target = &rest_str[..colon_pos + 1];
        let message = &rest_str[colon_pos + 1..];
        spans.push(Span::styled(
            target.to_string(),
            Style::new().fg(Color::DarkGray),
        ));
        spans.push(Span::raw(message.to_string()));
    } else {
        spans.push(Span::raw(rest_str));
    }

    Line::from(spans)
}

struct TracingWriter {
    capture: Arc<TracingCapture>,
    line_buf: Vec<u8>,
}

impl TracingWriter {
    fn new(capture: Arc<TracingCapture>) -> Self {
        Self {
            capture,
            line_buf: Vec::new(),
        }
    }
}

impl Write for TracingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &b in buf {
            if b == b'\n' {
                let line = String::from_utf8_lossy(&self.line_buf).to_string();
                self.capture.push(line);
                self.line_buf.clear();
            } else {
                self.line_buf.push(b);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.line_buf.is_empty() {
            let line = String::from_utf8_lossy(&self.line_buf).to_string();
            self.capture.push(line);
            self.line_buf.clear();
        }
        Ok(())
    }
}

struct TracingMakeWriter(Arc<TracingCapture>);

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for TracingMakeWriter {
    type Writer = TracingWriter;

    fn make_writer(&'a self) -> Self::Writer {
        TracingWriter::new(Arc::clone(&self.0))
    }
}

pub fn init_capture(capture: Arc<TracingCapture>) {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_writer(TracingMakeWriter(capture))
        .with_ansi(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).ok();
}
