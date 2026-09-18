use std::io::{self, Write};
use std::sync::LazyLock;

use regex::Regex;
use sentry::protocol::{Context, Event, Stacktrace, Value};

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").expect("Invalid regex")
});

static IP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("Invalid regex"));
static IPV6_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}\b").expect("Invalid regex")
});
static UUID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
        .expect("Invalid regex")
});
static SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer\s+|api[_ -]?key[=:]\s*|token[=:]\s*)[a-z0-9._~+/=-]{8,}")
        .expect("Invalid regex")
});

const SAFE_TAGS: &[&str] = &[
    "anarlog.error.stage",
    "anarlog.operation",
    "anarlog.session.type",
    "anarlog.stt.provider.name",
    "anarlog.surface",
    "error.code",
    "error.type",
    "http.response.status_code",
    "service.name",
    "service.namespace",
];

fn safe_identifier(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()
        && value.len() <= 128
        && !looks_like_absolute_path(value)
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_.:/-".contains(character)))
    .then_some(value)
}

fn safe_frame_symbol(value: String) -> Option<String> {
    (!value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_:.$<>-[]".contains(&byte)))
    .then_some(value)
}

fn safe_tag_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => safe_identifier(value).map(str::to_owned),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn looks_like_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('\\')
        || matches!(value.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
}

fn tracing_location_key(event: &Event<'_>) -> Option<String> {
    let Context::Other(location) = event.contexts.get("Rust Tracing Location")? else {
        return None;
    };
    let module_path = location
        .get("module_path")?
        .as_str()
        .and_then(safe_identifier)?;
    let line = location.get("line")?.as_u64()?;
    Some(format!("{module_path}:{line}"))
}

fn redact_sensitive_text(value: &str, home_dir: Option<&str>) -> String {
    let mut redacted = value.to_string();
    if let Some(home) = home_dir {
        redacted = redacted.replace(home, "[HOME]");
    }
    redacted = EMAIL_REGEX
        .replace_all(&redacted, "[EMAIL_REDACTED]")
        .into_owned();
    redacted = IP_REGEX
        .replace_all(&redacted, "[IP_REDACTED]")
        .into_owned();
    redacted = IPV6_REGEX
        .replace_all(&redacted, "[IP_REDACTED]")
        .into_owned();
    redacted = UUID_REGEX
        .replace_all(&redacted, "[ID_REDACTED]")
        .into_owned();
    SECRET_REGEX
        .replace_all(&redacted, "$1[SECRET_REDACTED]")
        .into_owned()
}

pub fn redact_text(value: &str) -> String {
    let home_dir = dirs::home_dir().map(|path| path.to_string_lossy().into_owned());
    redact_sensitive_text(value, home_dir.as_deref())
}

fn sanitize_stacktrace(stacktrace: &mut Stacktrace) {
    for frame in &mut stacktrace.frames {
        frame.function = frame.function.take().and_then(safe_frame_symbol);
        frame.filename = Some("source".to_string());
        frame.abs_path = None;
        frame.module = None;
        frame.package = None;
        frame.symbol = None;
        frame.pre_context.clear();
        frame.context_line = None;
        frame.post_context.clear();
        frame.vars.clear();
        frame.addr_mode = None;
    }
}

fn sanitize_context(key: &str, context: &mut Context, home_dir: Option<&str>) -> bool {
    match context {
        Context::Device(device) => {
            device.name = None;
            device.other.clear();
        }
        Context::Os(os) => os.other.clear(),
        Context::Runtime(runtime) => runtime.other.clear(),
        Context::App(app) => {
            app.device_app_hash = None;
            app.other.clear();
        }
        Context::Browser(browser) => browser.other.clear(),
        Context::Trace(trace) => {
            trace.description = None;
            trace.data.clear();
        }
        Context::Gpu(gpu) => gpu.other.clear(),
        Context::Other(values) if key == "Rust Tracing Location" => {
            values.retain(|field, value| match field.as_str() {
                "module_path" => {
                    let Value::String(text) = value else {
                        return false;
                    };
                    *text = redact_sensitive_text(text, home_dir);
                    true
                }
                "line" => matches!(value, Value::Number(_)),
                _ => false,
            });
            return !values.is_empty();
        }
        Context::Other(values) if key == "anarlog.session" => {
            values.retain(|field, value| match field.as_str() {
                "anarlog.session.onboarding" => matches!(value, Value::Bool(_)),
                "anarlog.session.transcription_mode" => {
                    value.as_str().and_then(safe_identifier).is_some()
                }
                _ => false,
            });
            return !values.is_empty();
        }
        _ => return false,
    }
    true
}

fn sanitize_sentry_event_with_home(
    mut event: Event<'static>,
    home_dir: Option<&str>,
) -> Event<'static> {
    let default_fingerprint = Event::default().fingerprint;
    let operation = event
        .message
        .as_deref()
        .and_then(safe_identifier)
        .map(str::to_owned);
    let location = tracing_location_key(&event);
    if event.fingerprint != default_fingerprint
        && !event
            .fingerprint
            .iter()
            .all(|part| safe_identifier(part).is_some())
    {
        event.fingerprint = default_fingerprint.clone();
    }
    event.culprit = None;
    event.transaction = None;
    event.message = None;
    event.logentry = None;
    event.server_name = None;
    event.request = None;
    let safe_extra_tags = event
        .extra
        .iter()
        .filter_map(|(key, value)| {
            if !SAFE_TAGS.contains(&key.as_str()) {
                return None;
            }
            safe_tag_value(value).map(|value| (key.clone(), value))
        })
        .collect::<Vec<_>>();
    event.extra.clear();
    event.tags.extend(safe_extra_tags);

    event.user = None;

    for breadcrumb in &mut event.breadcrumbs {
        breadcrumb.message = None;
        breadcrumb.data.retain(|key, value| {
            SAFE_TAGS.contains(&key.as_str()) && safe_tag_value(value).is_some()
        });
    }
    for exception in &mut event.exception {
        exception.value = Some(format!("{} captured", exception.ty));
        if let Some(mechanism) = &mut exception.mechanism {
            mechanism.description = None;
            mechanism.help_link = None;
            mechanism.data.clear();
        }
        if let Some(stacktrace) = &mut exception.stacktrace {
            sanitize_stacktrace(stacktrace);
        }
        if let Some(stacktrace) = &mut exception.raw_stacktrace {
            sanitize_stacktrace(stacktrace);
        }
    }
    if let Some(stacktrace) = &mut event.stacktrace {
        sanitize_stacktrace(stacktrace);
    }
    for thread in &mut event.threads {
        if let Some(stacktrace) = &mut thread.stacktrace {
            sanitize_stacktrace(stacktrace);
        }
        if let Some(stacktrace) = &mut thread.raw_stacktrace {
            sanitize_stacktrace(stacktrace);
        }
    }
    if let Some(template) = &mut event.template {
        template.filename = Some("source".to_string());
        template.abs_path = None;
        template.pre_context.clear();
        template.context_line = None;
        template.post_context.clear();
    }
    event
        .contexts
        .retain(|key, context| sanitize_context(key, context, home_dir));
    event
        .tags
        .retain(|key, _| SAFE_TAGS.contains(&key.as_str()));

    if event.exception.is_empty() && event.stacktrace.is_none() {
        let logger = event
            .logger
            .as_deref()
            .and_then(safe_identifier)
            .map(str::to_owned);
        let grouping_key = operation
            .clone()
            .or(location)
            .or(logger)
            .unwrap_or_else(|| "native_error".to_string());
        event.message = Some(operation.unwrap_or_else(|| format!("native_error:{grouping_key}")));
        event
            .tags
            .insert("anarlog.operation".to_string(), grouping_key.clone());
        if event.fingerprint == default_fingerprint {
            event.fingerprint = vec!["native_error".into(), grouping_key.into()].into();
        }
    }

    event
}

pub fn sanitize_sentry_event(event: Event<'static>) -> Option<Event<'static>> {
    if anlg_user_error::should_drop_sentry_event(&event) {
        return None;
    }

    let home_dir = dirs::home_dir().map(|path| path.to_string_lossy().into_owned());
    Some(sanitize_sentry_event_with_home(event, home_dir.as_deref()))
}

pub struct RedactingWriter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
    home_dir: Option<String>,
}

impl<W: Write> RedactingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(8192),
            home_dir: dirs::home_dir().map(|p| p.to_string_lossy().into_owned()),
        }
    }

    #[cfg(test)]
    fn with_home_dir(inner: W, home_dir: Option<String>) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(8192),
            home_dir,
        }
    }

    fn redact_line(&self, line: &str) -> String {
        if anlg_user_error::is_user_error_text(line) {
            return String::new();
        }
        redact_sensitive_text(line, self.home_dir.as_deref())
    }

    fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let line = String::from_utf8_lossy(&self.buffer);
        let redacted = self.redact_line(&line);
        self.inner.write_all(redacted.as_bytes())?;

        self.buffer.clear();
        Ok(())
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut last_newline_pos = 0;

        for (i, &byte) in buf.iter().enumerate() {
            if byte == b'\n' {
                self.buffer.extend_from_slice(&buf[last_newline_pos..=i]);
                self.flush_buffer()?;
                last_newline_pos = i + 1;
            }
        }

        if last_newline_pos < buf.len() {
            self.buffer.extend_from_slice(&buf[last_newline_pos..]);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Breadcrumb, Exception, Frame, LogEntry, OsContext, Request, User};
    use serde_json::json;
    use std::io::Write;

    fn assert_redaction(home: Option<&str>, input: &str, expected: &str) {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut output, home.map(String::from));
            writeln!(writer, "{}", input).unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result.trim(), expected, "input: {}", input);
    }

    macro_rules! redact_test {
        ($name:ident, home = $home:expr, $input:literal => $expected:literal) => {
            #[test]
            fn $name() {
                assert_redaction($home, $input, $expected);
            }
        };
    }

    redact_test!(redact_home_linux, home = Some("/home/johndoe"), "/home/johndoe/documents/file.txt" => "[HOME]/documents/file.txt");
    redact_test!(redact_home_linux_multiple, home = Some("/home/alice"), "/home/alice/file and /home/alice/other" => "[HOME]/file and [HOME]/other");
    redact_test!(redact_home_macos, home = Some("/Users/janedoe"), "/Users/janedoe/projects/app" => "[HOME]/projects/app");
    redact_test!(redact_home_windows, home = Some(r"C:\Users\johndoe"), r"C:\Users\johndoe\Desktop\file.txt" => r"[HOME]\Desktop\file.txt");
    redact_test!(redact_other_user_paths_preserved, home = Some("/home/alice"), "/home/bob/documents/file.txt" => "/home/bob/documents/file.txt");
    redact_test!(redact_email_single, home = None, "Contact: user@example.com for help" => "Contact: [EMAIL_REDACTED] for help");
    redact_test!(redact_email_multiple, home = None, "From alice@test.org to bob@example.com" => "From [EMAIL_REDACTED] to [EMAIL_REDACTED]");
    redact_test!(redact_email_complex, home = None, "Email: john.doe+tag@sub.example.co.uk" => "Email: [EMAIL_REDACTED]");
    redact_test!(redact_ip_single, home = None, "Connected to 192.168.1.1 successfully" => "Connected to [IP_REDACTED] successfully");
    redact_test!(redact_ip_multiple, home = None, "From 10.0.0.1 to 192.168.0.100" => "From [IP_REDACTED] to [IP_REDACTED]");
    redact_test!(redact_ip_localhost, home = None, "Listening on 127.0.0.1:8080" => "Listening on [IP_REDACTED]:8080");
    redact_test!(redact_mixed_content, home = Some("/home/alice"), "User alice@test.com at /home/alice connected from 192.168.1.50" => "User [EMAIL_REDACTED] at [HOME] connected from [IP_REDACTED]");
    redact_test!(redact_uuid, home = None, "session 550e8400-e29b-41d4-a716-446655440000 failed" => "session [ID_REDACTED] failed");
    redact_test!(redact_bearer, home = None, "authorization Bearer abcdefghijklmnop" => "authorization Bearer [SECRET_REDACTED]");
    redact_test!(redact_no_sensitive_data, home = None, "Application started successfully" => "Application started successfully");

    #[test]
    fn writer_drops_user_account_errors() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut output, None);
            writeln!(writer, "provider rejected request: insufficient_quota").unwrap();
            writer.flush().unwrap();
        }
        assert!(output.is_empty());
    }

    #[test]
    fn writer_buffers_partial_lines() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut output, Some("/home/user".into()));
            writer.write_all(b"test /home/").unwrap();
            writer.write_all(b"user/file\n").unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, "test [HOME]/file\n");
    }

    #[test]
    fn writer_handles_empty_input() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"").unwrap();
            writer.flush().unwrap();
        }
        assert_eq!(String::from_utf8(output).unwrap(), "");
    }

    #[test]
    fn writer_handles_only_newlines() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"\n\n\n").unwrap();
            writer.flush().unwrap();
        }
        assert_eq!(String::from_utf8(output).unwrap(), "\n\n\n");
    }

    #[test]
    fn writer_handles_interleaved_writes() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"line1 ").unwrap();
            writer.write_all(b"user@test.com").unwrap();
            writer.write_all(b" end\n").unwrap();
            writer.write_all(b"line2\n").unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, "line1 [EMAIL_REDACTED] end\nline2\n");
    }

    #[test]
    fn multiline_redaction() {
        let mut output = Vec::new();
        {
            let mut writer =
                RedactingWriter::with_home_dir(&mut output, Some("/home/testuser".into()));
            writeln!(writer, "User logged in from /home/testuser/app").unwrap();
            writeln!(writer, "Email: user@example.com").unwrap();
            writeln!(writer, "Connection from 192.168.1.100").unwrap();
            writer.flush().unwrap();
        }
        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("[HOME]/app"));
        assert!(content.contains("[EMAIL_REDACTED]"));
        assert!(content.contains("[IP_REDACTED]"));
        assert!(!content.contains("/home/testuser"));
        assert!(!content.contains("user@example.com"));
        assert!(!content.contains("192.168.1.100"));
    }

    #[test]
    fn sentry_event_removes_payloads_and_redacts_paths() {
        let mut event = Event {
            message: Some(
                "failed to read /Users/alice/private.wav for alice@example.com".to_string(),
            ),
            logentry: Some(LogEntry {
                message: "private note title".to_string(),
                params: vec![json!("private transcript")],
            }),
            request: Some(Request {
                data: Some("private transcript".to_string()),
                ..Default::default()
            }),
            user: Some(User {
                id: Some("pseudonymous-id".to_string()),
                email: Some("alice@example.com".to_string()),
                username: Some("alice".to_string()),
                ..Default::default()
            }),
            breadcrumbs: vec![Breadcrumb {
                message: Some("private note".to_string()),
                data: [("requestBody".to_string(), json!("private transcript"))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            }]
            .into(),
            exception: vec![Exception {
                ty: "IoError".to_string(),
                value: Some("/Users/alice/private.wav".to_string()),
                stacktrace: Some(Stacktrace {
                    frames: vec![Frame {
                        abs_path: Some("/Users/alice/private.wav".to_string()),
                        vars: [("note".to_string(), json!("private transcript"))]
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }]
            .into(),
            tags: [
                ("service.name".to_string(), "desktop".to_string()),
                ("error.type".to_string(), "file_read_failed".to_string()),
                ("private-note".to_string(), "meeting plans".to_string()),
            ]
            .into_iter()
            .collect(),
            contexts: [
                (
                    "os".to_string(),
                    OsContext {
                        name: Some("macOS".to_string()),
                        other: [("private-note".to_string(), json!("meeting plans"))]
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                ),
                (
                    "custom".to_string(),
                    Context::Other(
                        [("transcript".to_string(), json!("private transcript"))]
                            .into_iter()
                            .collect(),
                    ),
                ),
            ]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        event
            .extra
            .insert("requestBody".to_string(), json!("private transcript"));

        let event = sanitize_sentry_event_with_home(event, Some("/Users/alice"));

        assert!(event.message.is_none());
        assert!(event.logentry.is_none());
        assert!(event.request.is_none());
        assert!(event.extra.is_empty());
        assert!(event.user.is_none());
        assert!(event.breadcrumbs[0].message.is_none());
        assert!(event.breadcrumbs[0].data.is_empty());
        assert_eq!(
            event.exception[0].value.as_deref(),
            Some("IoError captured")
        );
        let frame = &event.exception[0].stacktrace.as_ref().unwrap().frames[0];
        assert_eq!(frame.filename.as_deref(), Some("source"));
        assert!(frame.abs_path.is_none());
        assert!(frame.vars.is_empty());
        assert_eq!(
            event.tags,
            [
                ("error.type".to_string(), "file_read_failed".to_string()),
                ("service.name".to_string(), "desktop".to_string()),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(event.contexts.len(), 1);
        let Context::Os(os) = &event.contexts["os"] else {
            panic!("expected os context");
        };
        assert!(os.other.is_empty());
    }

    #[test]
    fn sentry_event_preserves_safe_native_grouping_data() {
        let event = Event {
            message: Some("model_download_error".to_string()),
            logger: Some("model_downloader".to_string()),
            fingerprint: vec!["native_error".into(), "model_download".into()].into(),
            contexts: [(
                "Rust Tracing Location".to_string(),
                Context::Other(
                    [
                        (
                            "module_path".to_string(),
                            Value::String("model_downloader::download_task".to_string()),
                        ),
                        (
                            "file".to_string(),
                            Value::String("/Users/alice/src/download_task.rs".to_string()),
                        ),
                        ("line".to_string(), Value::Number(51.into())),
                        (
                            "private".to_string(),
                            Value::String("meeting plans".to_string()),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let event = sanitize_sentry_event_with_home(event, Some("/Users/alice"));

        assert_eq!(event.message.as_deref(), Some("model_download_error"));
        assert_eq!(
            event.fingerprint.as_ref(),
            ["native_error", "model_download"]
        );
        assert_eq!(
            event.tags.get("anarlog.operation").map(String::as_str),
            Some("model_download_error")
        );
        let Context::Other(location) = &event.contexts["Rust Tracing Location"] else {
            panic!("expected tracing location context");
        };
        assert!(!location.contains_key("file"));
        assert!(!location.contains_key("private"));
    }

    #[test]
    fn sentry_event_dropped_when_caused_by_user_account_state() {
        let event = Event {
            message: Some(
                "[Object {\"data\": Object {\"error\": Object {\"message\": String(\"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.\")}}}]"
                    .to_string(),
            ),
            ..Default::default()
        };

        assert!(sanitize_sentry_event(event).is_none());
    }

    #[test]
    fn sentry_event_dropped_when_archived_or_webview_console() {
        let from_console = Event {
            message: Some(
                r#"[String("[runBatch] error handling batch response"), Object {}]"#.to_string(),
            ),
            logger: Some("tauri_plugin_tracing::ext".to_string()),
            ..Default::default()
        };
        let from_archived = Event {
            message: Some("batch transcription failed".to_string()),
            ..Default::default()
        };

        assert!(sanitize_sentry_event(from_console).is_none());
        assert!(sanitize_sentry_event(from_archived).is_none());
    }

    #[test]
    fn sentry_event_preserves_only_safe_provider_error_metadata() {
        let mut event = Event {
            message: Some("stream_provider_error".to_string()),
            breadcrumbs: vec![Breadcrumb {
                message: Some("provider rejected private transcript".to_string()),
                data: [
                    ("error.type".to_string(), json!("invalid_request_error")),
                    ("error.code".to_string(), json!("invalid_value")),
                    ("error.message".to_string(), json!("private transcript")),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };
        event.extra.extend([
            ("error.code".to_string(), json!(400)),
            ("anarlog.stt.provider.name".to_string(), json!("openai")),
            ("error.message".to_string(), json!("private transcript")),
        ]);

        let event = sanitize_sentry_event_with_home(event, None);

        assert_eq!(
            event.tags.get("error.code").map(String::as_str),
            Some("400")
        );
        assert_eq!(
            event
                .tags
                .get("anarlog.stt.provider.name")
                .map(String::as_str),
            Some("openai")
        );
        assert!(!event.tags.contains_key("error.message"));
        assert!(event.breadcrumbs[0].message.is_none());
        assert_eq!(
            event.breadcrumbs[0].data,
            [
                ("error.code".to_string(), json!("invalid_value")),
                ("error.type".to_string(), json!("invalid_request_error")),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn sentry_event_does_not_group_by_absolute_paths() {
        for message in ["/Users/alice/private.wav", "C:/Users/alice/private.wav"] {
            let event = Event {
                message: Some(message.to_string()),
                logger: Some("file_reader".to_string()),
                ..Default::default()
            };

            let event = sanitize_sentry_event_with_home(event, Some("/Users/alice"));

            assert_eq!(event.message.as_deref(), Some("native_error:file_reader"));
            assert_eq!(event.fingerprint.as_ref(), ["native_error", "file_reader"]);
            assert!(!event.tags["anarlog.operation"].contains("alice"));
        }
    }

    #[test]
    fn sentry_event_groups_redacted_messages_by_source_location() {
        let event = Event {
            message: Some("failed for alice@example.com".to_string()),
            logger: Some("model_downloader".to_string()),
            contexts: [(
                "Rust Tracing Location".to_string(),
                Context::Other(
                    [
                        (
                            "module_path".to_string(),
                            Value::String("model_downloader::download_task".to_string()),
                        ),
                        ("line".to_string(), Value::Number(51.into())),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        let event = sanitize_sentry_event_with_home(event, None);

        assert_eq!(
            event.message.as_deref(),
            Some("native_error:model_downloader::download_task:51")
        );
        assert_eq!(
            event.fingerprint.as_ref(),
            ["native_error", "model_downloader::download_task:51"]
        );
        assert!(!event.message.as_deref().unwrap().contains("alice"));
    }
}
