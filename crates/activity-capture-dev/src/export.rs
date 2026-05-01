use std::{
    fs,
    io::{self, Write},
    ops::RangeInclusive,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use chrono::Local;
use hypr_activity_capture::{ActivityScreenshotCapture, Event, Transition};
use hypr_screen_core::CaptureSubject;
use serde::Serialize;

use crate::{
    event_row::{DetailField, EventRow},
    vlm::InferenceResult,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExportScope {
    Selection,
    Session,
}

impl ExportScope {
    fn label(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::Session => "session",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RawRecord {
    captured_at: SystemTime,
    status: String,
    app_name: String,
    summary: String,
    details: Vec<RawDetailField>,
    raw: RawPayload,
}

impl RawRecord {
    pub(crate) fn from_transition(row: &EventRow, transition: Transition) -> Self {
        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::Transition {
                previous: transition.previous,
                current: transition.current,
            },
        }
    }

    pub(crate) fn screenshot(
        row: &EventRow,
        capture: &ActivityScreenshotCapture,
        saved_path: Option<&Path>,
    ) -> Self {
        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::Screenshot {
                fingerprint: capture.fingerprint.clone(),
                reason: format!("{:?}", capture.reason),
                scheduled_at_ms: capture.scheduled_at_ms,
                captured_at_ms: capture.captured_at_ms,
                pid: capture.target.pid,
                app_name: capture.target.app_name.clone(),
                title: capture.target.title.clone(),
                image_width: capture.image.width,
                image_height: capture.image.height,
                image_bytes_len: capture.image.image_bytes.len(),
                subject: screenshot_subject(&capture.image.subject),
                saved_path: saved_path.map(|path| path.display().to_string()),
            },
        }
    }

    pub(crate) fn vlm(row: &EventRow, result: &InferenceResult) -> Self {
        let (response_text, error) = match &result.response {
            Ok(response) => (Some(response.clone()), None),
            Err(error) => (None, Some(error.clone())),
        };

        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::VlmInference {
                finished_at_ms: unix_ms(result.finished_at),
                latency_ms: result.latency().as_millis().min(u64::MAX as u128) as u64,
                fingerprint: result.screenshot.fingerprint.clone(),
                pid: result.screenshot.target.pid,
                app_name: result.screenshot.target.app_name.clone(),
                title: result.screenshot.target.title.clone(),
                screenshot_path: result.screenshot_path.display().to_string(),
                model_name: result.model_name.clone(),
                prompt: result.prompt.clone(),
                response_text,
                error,
            },
        }
    }

    pub(crate) fn placeholder(row: &EventRow, reason: impl Into<String>) -> Self {
        Self {
            captured_at: row.captured_at,
            status: row.status.label().to_string(),
            app_name: row.app_name.clone(),
            summary: row.summary.clone(),
            details: row.details.iter().map(RawDetailField::from).collect(),
            raw: RawPayload::Placeholder {
                reason: reason.into(),
            },
        }
    }

    pub(crate) fn pretty_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|error| {
            format!(
                "{{\"error\":\"failed to serialize selected record\",\"detail\":\"{}\"}}",
                error
            )
        })
    }
}

#[derive(Debug, Clone, Serialize)]
struct RawDetailField {
    label: String,
    value: String,
}

impl From<&DetailField> for RawDetailField {
    fn from(detail: &DetailField) -> Self {
        Self {
            label: detail.label.clone(),
            value: detail.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RawPayload {
    Transition {
        previous: Option<Event>,
        current: Option<Event>,
    },
    Placeholder {
        reason: String,
    },
    Screenshot {
        fingerprint: String,
        reason: String,
        scheduled_at_ms: i64,
        captured_at_ms: i64,
        pid: u32,
        app_name: String,
        title: Option<String>,
        image_width: u32,
        image_height: u32,
        image_bytes_len: usize,
        subject: ScreenshotSubjectPayload,
        saved_path: Option<String>,
    },
    VlmInference {
        finished_at_ms: u64,
        latency_ms: u64,
        fingerprint: String,
        pid: u32,
        app_name: String,
        title: Option<String>,
        screenshot_path: String,
        model_name: String,
        prompt: String,
        response_text: Option<String>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ScreenshotSubjectPayload {
    Window {
        id: u32,
        pid: u32,
        app_name: String,
        title: String,
        rect: ScreenshotRectPayload,
    },
    Display {
        id: u32,
        name: String,
        rect: ScreenshotRectPayload,
        is_primary: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
struct ScreenshotRectPayload {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
struct ExportBundle {
    exported_at: SystemTime,
    scope: &'static str,
    range: ExportRange,
    records: Vec<ExportedRecord>,
}

#[derive(Debug, Serialize)]
struct ExportRange {
    start_index: usize,
    end_index: usize,
    count: usize,
}

#[derive(Debug, Serialize)]
struct ExportedRecord {
    index: usize,
    #[serde(flatten)]
    record: RawRecord,
}

pub(crate) fn copy_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<usize> {
    let (json, count) = serialize_records(records, range, scope)?;
    copy_to_clipboard(&json)?;
    Ok(count)
}

pub(crate) fn save_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<PathBuf> {
    let (json, _) = serialize_records(records, range, scope)?;
    let file_name = format!(
        "activity-capture-{}-{}.json",
        scope.label(),
        Local::now().format("%Y%m%d-%H%M%S")
    );

    let directory = export_directory()?;
    fs::create_dir_all(&directory)?;
    let path = unique_path(&directory, &file_name);
    fs::write(&path, json)?;
    Ok(path)
}

pub(crate) fn save_screenshot_image(capture: &ActivityScreenshotCapture) -> io::Result<PathBuf> {
    let app_slug: String = capture
        .target
        .app_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let extension = screenshot_extension(&capture.image.mime_type);
    let file_name = format!(
        "screenshot-{}-{}.{}",
        Local::now().format("%H%M%S"),
        app_slug,
        extension,
    );

    let directory = export_directory()?;
    fs::create_dir_all(&directory)?;
    let path = unique_path(&directory, &file_name);
    fs::write(&path, &capture.image.image_bytes)?;
    Ok(path)
}

fn serialize_records(
    records: &[RawRecord],
    range: RangeInclusive<usize>,
    scope: ExportScope,
) -> io::Result<(String, usize)> {
    let start = *range.start();
    let end = *range.end();
    let slice = records
        .get(start..=end)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid export range"))?;

    let bundle = ExportBundle {
        exported_at: SystemTime::now(),
        scope: scope.label(),
        range: ExportRange {
            start_index: start,
            end_index: end,
            count: slice.len(),
        },
        records: slice
            .iter()
            .enumerate()
            .map(|(offset, record)| ExportedRecord {
                index: start + offset,
                record: record.clone(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&bundle)
        .map(|json| (json, slice.len()))
        .map_err(io::Error::other)
}

fn unique_path(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let stem = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("activity-capture-export");
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");

    for suffix in 2.. {
        let candidate = directory.join(format!("{stem}-{suffix}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("infinite suffix range should always find a free path")
}

fn screenshot_subject(subject: &CaptureSubject) -> ScreenshotSubjectPayload {
    match subject {
        CaptureSubject::Window(window) => ScreenshotSubjectPayload::Window {
            id: window.id,
            pid: window.pid,
            app_name: window.app_name.clone(),
            title: window.title.clone(),
            rect: screenshot_rect(window.rect),
        },
        CaptureSubject::Display(display) => ScreenshotSubjectPayload::Display {
            id: display.id,
            name: display.name.clone(),
            rect: screenshot_rect(display.rect),
            is_primary: display.is_primary,
        },
    }
}

fn screenshot_rect(rect: hypr_screen_core::CaptureRect) -> ScreenshotRectPayload {
    ScreenshotRectPayload {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn screenshot_extension(mime_type: &str) -> &'static str {
    match mime_type {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/jpeg" => "jpg",
        _ => "img",
    }
}

fn unix_ms(value: SystemTime) -> u64 {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

fn export_directory() -> io::Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("out"))
}

fn copy_to_clipboard(contents: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        return copy_with_command("pbcopy", &[], contents);
    }

    #[cfg(target_os = "windows")]
    {
        return copy_with_command("cmd", &["/C", "clip"], contents);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let candidates = [
            ("wl-copy", Vec::<&str>::new()),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
        ];

        let mut last_error = None;
        for (command, args) in candidates {
            match copy_with_command(command, &args, contents) {
                Ok(()) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        return Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no supported clipboard command found (tried wl-copy, xclip, xsel)",
            )
        }));
    }

    #[allow(unreachable_code)]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "clipboard export is not supported on this platform",
    ))
}

fn copy_with_command(command: &str, args: &[&str], contents: &str) -> io::Result<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err(io::Error::other("clipboard stdin is unavailable"));
    };
    stdin.write_all(contents.as_bytes())?;
    drop(stdin);

    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{command} failed with status {status}"
        )))
    }
}
