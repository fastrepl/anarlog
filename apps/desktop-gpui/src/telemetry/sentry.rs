use std::io::{self, Write};
use std::sync::{Arc, LazyLock};

use anlg_desktop_identity::Channel;
use regex::Regex;
use sentry::integrations::tracing::EventFilter;
use sentry::protocol::{Context, Event, Stacktrace, Value};
use tracing_subscriber::fmt::MakeWriter;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").expect("valid regex")
});
static IP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("valid regex"));
static IPV6_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}\b").expect("valid regex")
});
static UUID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b")
        .expect("valid regex")
});
static SECRET_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(bearer\s+|api[_ -]?key[=:]\s*|token[=:]\s*)[a-z0-9._~+/=-]{8,}")
        .expect("valid regex")
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
    "release_channel",
    "service.name",
    "service.namespace",
];

fn parse_dsn(raw: Option<&str>) -> Option<::sentry::types::Dsn> {
    raw.map(str::trim)
        .filter(|dsn| !dsn.is_empty())
        .and_then(|dsn| dsn.parse::<::sentry::types::Dsn>().ok())
}

pub(super) fn init(channel: Option<Channel>) -> Option<::sentry::ClientInitGuard> {
    let channel = channel?;
    if std::env::var_os("ANARLOG_DISABLE_SENTRY").is_some() {
        return None;
    }
    let dsn = parse_dsn(option_env!("SENTRY_DSN"))?;
    let release = format!("anarlog-desktop@{}", crate::platform::package::VERSION);
    let client = ::sentry::init((
        dsn,
        ::sentry::ClientOptions {
            release: Some(release.into()),
            traces_sample_rate: 0.0,
            auto_session_tracking: false,
            before_send: Some(Arc::new(|event| {
                super::crash_reporting_enabled()
                    .then(|| sanitize_sentry_event(event))
                    .flatten()
            })),
            before_breadcrumb: Some(Arc::new(|breadcrumb| {
                super::crash_reporting_enabled().then_some(breadcrumb)
            })),
            ..Default::default()
        },
    ));
    ::sentry::configure_scope(|scope| {
        scope.set_tag("service.namespace", "anarlog");
        scope.set_tag("service.name", "desktop");
        scope.set_tag("anarlog.surface", "desktop-gpui");
        scope.set_tag("release_channel", channel.name());
    });
    Some(client)
}

pub(super) fn tracing_layer<S>() -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    ::sentry::integrations::tracing::layer().event_filter(|metadata| {
        if metadata.target().starts_with("desktop_gpui::telemetry") {
            return EventFilter::Ignore;
        }
        match *metadata.level() {
            tracing::Level::ERROR => EventFilter::Event,
            tracing::Level::WARN | tracing::Level::INFO => EventFilter::Breadcrumb,
            _ => EventFilter::Ignore,
        }
    })
}

fn looks_like_absolute_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('\\')
        || matches!(value.as_bytes(), [drive, b':', b'/' | b'\\', ..] if drive.is_ascii_alphabetic())
}

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
    if !event.debug_meta.images.is_empty() {
        for image in &mut event.debug_meta.to_mut().images {
            match image {
                ::sentry::protocol::DebugImage::Apple(apple) => {
                    apple.name = basename_only(&apple.name);
                }
                ::sentry::protocol::DebugImage::Symbolic(symbolic) => {
                    symbolic.name = basename_only(&symbolic.name);
                    if let Some(debug_file) = symbolic.debug_file.as_mut() {
                        *debug_file = basename_only(debug_file);
                    }
                }
                ::sentry::protocol::DebugImage::Wasm(wasm) => {
                    wasm.name = basename_only(&wasm.name);
                    wasm.code_file = basename_only(&wasm.code_file);
                    if let Some(debug_file) = wasm.debug_file.as_mut() {
                        *debug_file = basename_only(debug_file);
                    }
                }
                ::sentry::protocol::DebugImage::Proguard(_) => {}
            }
        }
    }

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

fn basename_only(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(path)
        .to_string()
}

pub(super) fn sanitize_sentry_event(event: Event<'static>) -> Option<Event<'static>> {
    if anlg_user_error::should_drop_sentry_event(&event) {
        return None;
    }
    let home_dir = dirs::home_dir().map(|path| path.to_string_lossy().into_owned());
    Some(sanitize_sentry_event_with_home(event, home_dir.as_deref()))
}

pub(super) fn capture_operational_error(
    operation: &'static str,
    stage: Option<&'static str>,
    error: &str,
    provider: Option<&str>,
    status_code: Option<u16>,
) {
    if !super::crash_reporting_enabled() {
        return;
    }
    if anlg_user_error::is_user_error_text(error) || anlg_user_error::is_ignored_error_text(error) {
        return;
    }
    ::sentry::with_scope(
        |scope| {
            scope.set_tag("anarlog.operation", operation);
            if let Some(stage) = stage.and_then(safe_identifier) {
                scope.set_tag("anarlog.error.stage", stage);
            }
            if let Some(provider) = provider.and_then(safe_identifier) {
                scope.set_tag("anarlog.stt.provider.name", provider);
            }
            if let Some(code) = status_code {
                scope.set_tag("http.response.status_code", code.to_string());
            }
        },
        || {
            ::sentry::capture_message(operation, ::sentry::Level::Error);
        },
    );
}

pub struct RedactingWriter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
    home_dir: Option<String>,
}

impl<W: Write> RedactingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self::with_home_dir(
            inner,
            dirs::home_dir().map(|path| path.to_string_lossy().into_owned()),
        )
    }

    fn with_home_dir(inner: W, home_dir: Option<String>) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(1024),
            home_dir,
        }
    }

    fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let line = String::from_utf8_lossy(&self.buffer);
        if !anlg_user_error::is_user_error_text(&line) {
            let redacted = redact_sensitive_text(&line, self.home_dir.as_deref());
            self.inner.write_all(redacted.as_bytes())?;
        }
        self.buffer.clear();
        Ok(())
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &byte in buf {
            self.buffer.push(byte);
            if byte == b'\n' {
                self.flush_buffer()?;
            }
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
        let _ = self.flush_buffer();
    }
}

pub struct RedactingStderr;

impl<'a> MakeWriter<'a> for RedactingStderr {
    type Writer = RedactingWriter<io::StderrLock<'a>>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(io::stderr().lock())
    }
}

pub(super) fn redacting_stderr() -> RedactingStderr {
    RedactingStderr
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::sentry::protocol::{Breadcrumb, Exception, Frame, LogEntry, OsContext, Request, User};
    use serde_json::json;

    #[test]
    fn sanitize_strips_pii_and_keeps_safe_gpui_tags() {
        let mut event = Event {
            message: Some("failed at /Users/alice/private.wav for alice@example.com".into()),
            logentry: Some(LogEntry {
                message: "private".into(),
                params: vec![json!("secret")],
            }),
            request: Some(Request {
                data: Some("body".into()),
                ..Default::default()
            }),
            user: Some(User {
                id: Some("u".into()),
                email: Some("alice@example.com".into()),
                ..Default::default()
            }),
            breadcrumbs: vec![Breadcrumb {
                message: Some("note".into()),
                data: [("body".into(), json!("secret"))].into_iter().collect(),
                ..Default::default()
            }]
            .into(),
            exception: vec![Exception {
                ty: "IoError".into(),
                value: Some("/Users/alice/file".into()),
                stacktrace: Some(Stacktrace {
                    frames: vec![Frame {
                        abs_path: Some("/Users/alice/file.rs".into()),
                        vars: [("note".into(), json!("secret"))].into_iter().collect(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }]
            .into(),
            tags: [
                ("service.name".into(), "desktop".into()),
                ("anarlog.surface".into(), "desktop-gpui".into()),
                ("release_channel".into(), "stable".into()),
                ("private-tag".into(), "secret".into()),
            ]
            .into_iter()
            .collect(),
            contexts: [(
                "os".into(),
                OsContext {
                    name: Some("macOS".into()),
                    other: [("note".into(), json!("secret"))].into_iter().collect(),
                    ..Default::default()
                }
                .into(),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };
        event.extra.insert("body".into(), json!("secret"));

        let sanitized = sanitize_sentry_event_with_home(event, Some("/Users/alice"));
        assert!(sanitized.message.is_none());
        assert!(sanitized.logentry.is_none());
        assert!(sanitized.request.is_none());
        assert!(sanitized.user.is_none());
        assert!(sanitized.extra.is_empty());
        assert_eq!(
            sanitized.tags.get("anarlog.surface").map(String::as_str),
            Some("desktop-gpui")
        );
        assert_eq!(
            sanitized.tags.get("release_channel").map(String::as_str),
            Some("stable")
        );
        assert!(!sanitized.tags.contains_key("private-tag"));
    }

    #[test]
    fn sanitize_drops_user_account_errors() {
        let event = Event {
            message: Some("insufficient_quota: upgrade plan".into()),
            ..Default::default()
        };
        assert!(sanitize_sentry_event(event).is_none());
    }

    #[test]
    fn parse_dsn_rejects_empty_and_malformed_values_without_panicking() {
        assert!(parse_dsn(None).is_none());
        assert!(parse_dsn(Some("")).is_none());
        assert!(parse_dsn(Some("   ")).is_none());
        assert!(parse_dsn(Some("not-a-dsn")).is_none());
        assert!(parse_dsn(Some("https://public@example.ingest.sentry.io/123456")).is_some());
        assert!(parse_dsn(Some("  https://public@example.ingest.sentry.io/123456  ")).is_some());
    }

    #[test]
    fn sanitize_strips_debug_image_paths_while_keeping_symbolication_ids() {
        use std::str::FromStr;

        let debug_id =
            ::sentry::types::DebugId::from_str("41e825a2-82c5-3b88-902f-2a5b62b1c3d4").unwrap();
        let code_id = ::sentry::types::CodeId::new("abcd1234".into());
        let apple_uuid =
            ::sentry::types::Uuid::from_str("41e825a2-82c5-3b88-902f-2a5b62b1c3d4").unwrap();
        let mut event = Event::default();
        event.debug_meta.to_mut().images = vec![
            ::sentry::protocol::DebugImage::Symbolic(::sentry::protocol::SymbolicDebugImage {
                name: "/Users/alice/Applications/Anarlog.app/Contents/MacOS/Anarlog".into(),
                arch: Some("arm64".into()),
                image_addr: ::sentry::protocol::Addr(0x1000_0000),
                image_size: 4096,
                image_vmaddr: ::sentry::protocol::Addr(0x1000_0000),
                id: debug_id,
                code_id: Some(code_id.clone()),
                debug_file: Some("/Users/alice/builds/Anarlog.dSYM".into()),
            }),
            ::sentry::protocol::DebugImage::Apple(::sentry::protocol::AppleDebugImage {
                name: "/Users/alice/lib/libfoo.dylib".into(),
                arch: Some("arm64".into()),
                cpu_type: None,
                cpu_subtype: None,
                image_addr: ::sentry::protocol::Addr(0x2000_0000),
                image_size: 2048,
                image_vmaddr: ::sentry::protocol::Addr(0x2000_0000),
                uuid: apple_uuid,
            }),
        ];

        let sanitized = sanitize_sentry_event_with_home(event, Some("/Users/alice"));
        assert_eq!(sanitized.debug_meta.images.len(), 2);
        match &sanitized.debug_meta.images[0] {
            ::sentry::protocol::DebugImage::Symbolic(symbolic) => {
                assert_eq!(symbolic.name, "Anarlog");
                assert_eq!(symbolic.debug_file.as_deref(), Some("Anarlog.dSYM"));
                assert_eq!(symbolic.id, debug_id);
                assert_eq!(symbolic.code_id.as_ref(), Some(&code_id));
                assert_eq!(symbolic.image_addr, ::sentry::protocol::Addr(0x1000_0000));
                assert_eq!(symbolic.image_size, 4096);
            }
            other => panic!("unexpected image variant: {other:?}"),
        }
        match &sanitized.debug_meta.images[1] {
            ::sentry::protocol::DebugImage::Apple(apple) => {
                assert_eq!(apple.name, "libfoo.dylib");
                assert_eq!(apple.uuid, apple_uuid);
                assert_eq!(apple.image_addr, ::sentry::protocol::Addr(0x2000_0000));
                assert_eq!(apple.image_size, 2048);
            }
            other => panic!("unexpected image variant: {other:?}"),
        }
    }

    #[test]
    fn sanitize_keeps_only_safe_anarlog_session_fields() {
        let mut session = ::sentry::protocol::Map::new();
        session.insert("anarlog.session.onboarding".into(), Value::Bool(true));
        session.insert("anarlog.session.transcription_mode".into(), json!("live"));
        session.insert("anarlog.session.id".into(), json!("secret-session-uuid"));
        session.insert("anarlog.session.secret".into(), json!("should-drop"));
        let mut event = Event::default();
        event
            .contexts
            .insert("anarlog.session".into(), Context::Other(session));

        let sanitized = sanitize_sentry_event_with_home(event, None);
        let Context::Other(kept) = sanitized.contexts.get("anarlog.session").unwrap() else {
            panic!("expected anarlog.session context");
        };
        assert_eq!(
            kept.get("anarlog.session.onboarding"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            kept.get("anarlog.session.transcription_mode"),
            Some(&json!("live"))
        );
        assert!(!kept.contains_key("anarlog.session.id"));
        assert!(!kept.contains_key("anarlog.session.secret"));
    }

    #[test]
    fn redacting_writer_strips_pii_and_suppresses_user_errors() {
        let mut out = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut out, Some("/Users/alice".into()));
            writeln!(
                writer,
                "WARN sync failed for alice@example.com at /Users/alice/notes/secret"
            )
            .unwrap();
            writeln!(writer, "ERROR insufficient_quota: please upgrade").unwrap();
            writer.flush().unwrap();
        }
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("[EMAIL_REDACTED]"));
        assert!(text.contains("[HOME]/notes/secret"));
        assert!(!text.contains("alice@example.com"));
        assert!(!text.contains("insufficient_quota"));
    }

    #[test]
    fn operational_error_respects_consent_and_filters_user_errors() {
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = captured.clone();
        let _guard = ::sentry::init((
            "https://public@example.ingest.sentry.io/123456",
            ::sentry::ClientOptions {
                traces_sample_rate: 0.0,
                auto_session_tracking: false,
                before_send: Some(Arc::new(move |event| {
                    if super::super::crash_reporting_enabled()
                        && let Some(sanitized) = sanitize_sentry_event(event)
                    {
                        sink.lock().unwrap().push(sanitized);
                    }
                    None
                })),
                ..Default::default()
            },
        ));

        super::super::set_crash_reporting_enabled(false);
        capture_operational_error("cloud_sync_manual", Some("push"), "timeout", None, None);

        super::super::set_crash_reporting_enabled(true);
        capture_operational_error(
            "cloud_sync_manual",
            Some("push"),
            "insufficient_quota: upgrade plan",
            None,
            None,
        );
        capture_operational_error(
            "cloud_sync_manual",
            Some("push"),
            "connection reset by alice@example.com",
            Some("anarlog"),
            Some(502),
        );
        super::super::set_crash_reporting_enabled(false);

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].message.as_deref(), Some("cloud_sync_manual"));
        assert_eq!(
            captured[0]
                .tags
                .get("anarlog.operation")
                .map(String::as_str),
            Some("cloud_sync_manual")
        );
        assert_eq!(
            captured[0]
                .tags
                .get("anarlog.error.stage")
                .map(String::as_str),
            Some("push")
        );
        assert_eq!(
            captured[0]
                .tags
                .get("http.response.status_code")
                .map(String::as_str),
            Some("502")
        );
    }
}
