use std::io::{self, Write};
use std::sync::LazyLock;

use regex::Regex;
use sentry::protocol::{Context, Event, Stacktrace, User};

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").expect("Invalid regex")
});

static IP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("Invalid regex"));

fn redact_sensitive_text(value: &str, home_dir: Option<&str>) -> String {
    let mut redacted = value.to_string();
    if let Some(home) = home_dir {
        redacted = redacted.replace(home, "[HOME]");
    }
    redacted = EMAIL_REGEX
        .replace_all(&redacted, "[EMAIL_REDACTED]")
        .into_owned();
    IP_REGEX
        .replace_all(&redacted, "[IP_REDACTED]")
        .into_owned()
}

fn sanitize_stacktrace(stacktrace: &mut Stacktrace, home_dir: Option<&str>) {
    for frame in &mut stacktrace.frames {
        frame.filename = frame
            .filename
            .take()
            .map(|path| redact_sensitive_text(&path, home_dir));
        frame.abs_path = frame
            .abs_path
            .take()
            .map(|path| redact_sensitive_text(&path, home_dir));
        frame.pre_context.clear();
        frame.context_line = None;
        frame.post_context.clear();
        frame.vars.clear();
    }
}

fn sanitize_context(context: &mut Context) -> bool {
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
        _ => return false,
    }
    true
}

fn sanitize_sentry_event_with_home(
    mut event: Event<'static>,
    home_dir: Option<&str>,
) -> Event<'static> {
    event.fingerprint = Event::default().fingerprint;
    event.culprit = None;
    event.transaction = None;
    event.message = None;
    event.logentry = None;
    event.server_name = None;
    event.request = None;
    event.extra.clear();

    if let Some(user) = event.user.take() {
        event.user = user.id.map(|id| User {
            id: Some(id),
            ..Default::default()
        });
    }

    for breadcrumb in &mut event.breadcrumbs {
        breadcrumb.message = None;
        breadcrumb.data.clear();
    }
    for exception in &mut event.exception {
        exception.value = Some(format!("{} captured", exception.ty));
        if let Some(mechanism) = &mut exception.mechanism {
            mechanism.description = None;
            mechanism.help_link = None;
            mechanism.data.clear();
        }
        if let Some(stacktrace) = &mut exception.stacktrace {
            sanitize_stacktrace(stacktrace, home_dir);
        }
        if let Some(stacktrace) = &mut exception.raw_stacktrace {
            sanitize_stacktrace(stacktrace, home_dir);
        }
    }
    if let Some(stacktrace) = &mut event.stacktrace {
        sanitize_stacktrace(stacktrace, home_dir);
    }
    for thread in &mut event.threads {
        if let Some(stacktrace) = &mut thread.stacktrace {
            sanitize_stacktrace(stacktrace, home_dir);
        }
        if let Some(stacktrace) = &mut thread.raw_stacktrace {
            sanitize_stacktrace(stacktrace, home_dir);
        }
    }
    if let Some(template) = &mut event.template {
        template.filename = template
            .filename
            .take()
            .map(|path| redact_sensitive_text(&path, home_dir));
        template.abs_path = template
            .abs_path
            .take()
            .map(|path| redact_sensitive_text(&path, home_dir));
        template.pre_context.clear();
        template.context_line = None;
        template.post_context.clear();
    }
    event
        .contexts
        .retain(|_, context| sanitize_context(context));
    event.tags.retain(|key, _| {
        matches!(
            key.as_str(),
            "service.name" | "service.namespace" | "enduser.pseudo.id"
        )
    });

    event
}

pub fn sanitize_sentry_event(event: Event<'static>) -> Option<Event<'static>> {
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
    use sentry::protocol::{Breadcrumb, Exception, Frame, LogEntry, OsContext, Request};
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
    redact_test!(redact_no_sensitive_data, home = None, "Application started successfully" => "Application started successfully");

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
        assert_eq!(
            event.user,
            Some(User {
                id: Some("pseudonymous-id".to_string()),
                ..Default::default()
            })
        );
        assert!(event.breadcrumbs[0].message.is_none());
        assert!(event.breadcrumbs[0].data.is_empty());
        assert_eq!(
            event.exception[0].value.as_deref(),
            Some("IoError captured")
        );
        let frame = &event.exception[0].stacktrace.as_ref().unwrap().frames[0];
        assert_eq!(frame.abs_path.as_deref(), Some("[HOME]/private.wav"));
        assert!(frame.vars.is_empty());
        assert_eq!(
            event.tags,
            [("service.name".to_string(), "desktop".to_string())]
                .into_iter()
                .collect()
        );
        assert_eq!(event.contexts.len(), 1);
        let Context::Os(os) = &event.contexts["os"] else {
            panic!("expected os context");
        };
        assert!(os.other.is_empty());
    }
}
