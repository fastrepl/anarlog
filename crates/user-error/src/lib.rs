//! Classification of failures that must not be emitted to error telemetry.
//!
//! This includes the end user's own account state (exhausted credits, expired
//! plans, bad API keys) and issue types that were already archived as solved.

use sentry::protocol::{Context, Event, Value};

// Keep in sync with packages/user-error/src/index.js.
const USER_ERROR_MARKERS: &[&str] = &[
    "billing_hard_limit_reached",
    "api key is invalid",
    "apikey is invalid",
    "credit balance is too low",
    "exceeded your current quota",
    "incorrect api key",
    "insufficient balance",
    "insufficient credits",
    "insufficient funds",
    "insufficient_quota",
    "invalid api key",
    "invalid apikey",
    "invalid x-api-key",
    "invalid_api_key",
    "not enough credits",
    "no quota",
    "payment required",
    "plans & billing",
    "plans and billing",
    "quota exceeded",
    "upgrade or purchase credits",
];

// Archived Sentry issue types. Local error logs stay; Sentry should not reopen
// or bill for issues that were already solved. Keep in sync with
// `IGNORED_ERROR_MARKERS` in apps/desktop/src/error-reporting.ts.
const IGNORED_ERROR_MARKERS: &[&str] = &[
    "[runbatch]",
    "post-stop transcript repair failed",
    "[audio-retention]",
    "batch transcription failed",
    "connect_async_failed",
    "acquired connection, but time to acquire exceeded",
    "slow statement",
    "samples_dropped",
    "mic_samples_dropped",
    "zoom_mic_usage_check_failed",
    "e2ee recovery key setup is required",
    "couldn't find callback id",
    "[sessionpersister]",
    "update_check_failed",
    "failed to check for updates",
    "failed_to_check_for_updates",
    "listen_ws_connect_failed",
    "listener_retry_failed",
    "failed to fetch remote connection ids",
];

const WEBVIEW_CONSOLE_LOGGERS: &[&str] = &[
    "anarlog.webview.console",
    "hyprnote.webview.console",
    "tauri_plugin_tracing::ext",
];

pub fn is_user_error_text(text: &str) -> bool {
    contains_marker(text, USER_ERROR_MARKERS)
}

pub fn is_ignored_error_text(text: &str) -> bool {
    contains_marker(text, IGNORED_ERROR_MARKERS) || is_webview_console_dump(text)
}

fn contains_marker(text: &str, markers: &[&str]) -> bool {
    let text = text.to_lowercase();
    markers.iter().any(|marker| text.contains(marker))
}

fn is_webview_console_dump(text: &str) -> bool {
    text.starts_with("[String(") || text.starts_with("[Object {")
}

fn is_webview_console_logger(logger: Option<&str>) -> bool {
    logger.is_some_and(|logger| {
        WEBVIEW_CONSOLE_LOGGERS.contains(&logger) || logger.starts_with("tauri_plugin_tracing")
    })
}

fn value_matches(value: &Value, predicate: fn(&str) -> bool) -> bool {
    match value {
        Value::String(text) => predicate(text),
        Value::Array(values) => values.iter().any(|value| value_matches(value, predicate)),
        Value::Object(values) => values.values().any(|value| value_matches(value, predicate)),
        _ => false,
    }
}

fn event_matches_text(event: &Event<'_>, predicate: fn(&str) -> bool) -> bool {
    let message = event.message.as_deref().is_some_and(predicate);
    let logentry = event.logentry.as_ref().is_some_and(|entry| {
        predicate(&entry.message)
            || entry
                .params
                .iter()
                .any(|value| value_matches(value, predicate))
    });
    let exception = event.exception.iter().any(|exception| {
        exception.value.as_deref().is_some_and(predicate) || predicate(&exception.ty)
    });
    let extra = event
        .extra
        .values()
        .any(|value| value_matches(value, predicate));
    let tags = event.tags.values().any(|tag| predicate(tag));
    let contexts = event.contexts.values().any(|context| match context {
        Context::Other(values) => values.values().any(|value| value_matches(value, predicate)),
        _ => false,
    });

    message || logentry || exception || extra || tags || contexts
}

pub fn is_user_error_event(event: &Event<'_>) -> bool {
    event_matches_text(event, is_user_error_text)
}

pub fn is_ignored_error_event(event: &Event<'_>) -> bool {
    is_webview_console_logger(event.logger.as_deref())
        || event_matches_text(event, is_ignored_error_text)
}

pub fn should_drop_sentry_event(event: &Event<'_>) -> bool {
    is_user_error_event(event) || is_ignored_error_event(event)
}

pub fn drop_user_error_event(event: Event<'static>) -> Option<Event<'static>> {
    (!is_user_error_event(&event)).then_some(event)
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:/-".contains(&byte))
}

fn is_safe_frame_symbol(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_:.$<>-[]".contains(&byte))
}

fn sanitize_stacktrace(stacktrace: &mut sentry::protocol::Stacktrace) {
    stacktrace.registers.clear();
    for frame in &mut stacktrace.frames {
        frame.function = frame
            .function
            .take()
            .filter(|value| is_safe_frame_symbol(value));
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

pub fn sanitize_sentry_event(mut event: Event<'static>) -> Option<Event<'static>> {
    if should_drop_sentry_event(&event) {
        return None;
    }

    let operation = event
        .message
        .as_deref()
        .filter(|value| is_safe_identifier(value))
        .map(str::to_owned);
    event.user = None;
    event.request = None;
    event.contexts.clear();
    event.breadcrumbs = Default::default();
    event.extra.clear();
    event.message = None;
    event.logentry = None;
    event.transaction = None;
    event.culprit = None;
    event.fingerprint = Default::default();
    event.tags.retain(|key, value| {
        matches!(
            key.as_str(),
            "anarlog.error.stage"
                | "anarlog.honeycomb.span_id"
                | "anarlog.honeycomb.trace_id"
                | "anarlog.operation"
                | "anarlog.surface"
                | "error.code"
                | "error.type"
                | "http.response.status_code"
                | "service.name"
                | "service.namespace"
        ) && is_safe_identifier(value)
    });

    if let Some(stacktrace) = &mut event.stacktrace {
        sanitize_stacktrace(stacktrace);
    }
    for exception in &mut event.exception {
        if !is_safe_identifier(&exception.ty) {
            exception.ty = "Error".to_string();
        }
        exception.value = Some(format!("{} captured", exception.ty));
        exception.module = None;
        if let Some(stacktrace) = &mut exception.stacktrace {
            sanitize_stacktrace(stacktrace);
        }
        if let Some(stacktrace) = &mut exception.raw_stacktrace {
            sanitize_stacktrace(stacktrace);
        }
        if let Some(mechanism) = &mut exception.mechanism {
            mechanism.description = None;
            mechanism.help_link = None;
            mechanism.data.clear();
        }
    }
    for thread in &mut event.threads {
        thread.id = None;
        thread.name = None;
        if let Some(stacktrace) = &mut thread.stacktrace {
            sanitize_stacktrace(stacktrace);
        }
        if let Some(stacktrace) = &mut thread.raw_stacktrace {
            sanitize_stacktrace(stacktrace);
        }
    }

    if event.exception.is_empty() && event.stacktrace.is_none() {
        let grouping_key = operation.unwrap_or_else(|| "server_error".to_string());
        event.message = Some(grouping_key.clone());
        event
            .tags
            .insert("anarlog.operation".to_string(), grouping_key.clone());
        event.fingerprint = vec!["server_error".into(), grouping_key.into()].into();
    }

    Some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Breadcrumb, Exception, Frame, LogEntry, Stacktrace};

    const ANTHROPIC_CREDIT_ERROR: &str = "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.";

    #[test]
    fn detects_billing_and_credential_failures() {
        assert!(is_user_error_text(ANTHROPIC_CREDIT_ERROR));
        assert!(is_user_error_text(
            "{\"code\":\"insufficient_quota\",\"type\":\"invalid_request_error\"}"
        ));
        assert!(is_user_error_text("API key is invalid"));
        assert!(is_user_error_text("Invalid APIKEY"));
        assert!(!is_user_error_text("failed to open database"));
    }

    #[test]
    fn drops_events_carrying_user_error_details() {
        let from_message = Event {
            message: Some(ANTHROPIC_CREDIT_ERROR.to_string()),
            ..Default::default()
        };
        let from_logentry = Event {
            logentry: Some(LogEntry {
                message: "chat_completion_failed: {}".to_string(),
                params: vec![Value::String(ANTHROPIC_CREDIT_ERROR.to_string())],
            }),
            ..Default::default()
        };
        let from_exception = Event {
            exception: vec![Exception {
                ty: "ProviderError".to_string(),
                value: Some(ANTHROPIC_CREDIT_ERROR.to_string()),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };
        let from_extra = Event {
            extra: [(
                "error.message".to_string(),
                Value::String(ANTHROPIC_CREDIT_ERROR.into()),
            )]
            .into_iter()
            .collect(),
            ..Default::default()
        };

        for event in [from_message, from_logentry, from_exception, from_extra] {
            assert!(drop_user_error_event(event).is_none());
        }
    }

    #[test]
    fn keeps_events_whose_only_match_is_an_earlier_breadcrumb() {
        let event = Event {
            message: Some("database_migration_failed".to_string()),
            breadcrumbs: vec![Breadcrumb {
                message: Some(ANTHROPIC_CREDIT_ERROR.to_string()),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        assert!(drop_user_error_event(event).is_some());
    }

    #[test]
    fn keeps_unrelated_events() {
        let event = Event {
            message: Some("database_migration_failed".to_string()),
            ..Default::default()
        };

        assert!(drop_user_error_event(event).is_some());
    }

    #[test]
    fn sanitizer_replaces_stack_paths_with_a_fixed_source_name() {
        let event = Event {
            exception: vec![Exception {
                ty: "IoError".to_string(),
                stacktrace: Some(Stacktrace {
                    frames: vec![Frame {
                        filename: Some("private@example.com.rs".to_string()),
                        abs_path: Some("/Users/alice/private.rs".to_string()),
                        module: Some("private.module".to_string()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        let event = sanitize_sentry_event(event).unwrap();
        let frame = &event.exception[0].stacktrace.as_ref().unwrap().frames[0];
        assert_eq!(frame.filename.as_deref(), Some("source"));
        assert!(frame.abs_path.is_none());
        assert!(frame.module.is_none());
    }

    #[test]
    fn drops_archived_operational_noise_and_webview_console() {
        let from_message = Event {
            message: Some(
                r#"[String("[runBatch] error handling batch response"), Object {}]"#.to_string(),
            ),
            logger: Some("tauri_plugin_tracing::ext".to_string()),
            ..Default::default()
        };
        let from_repair = Event {
            message: Some("[listener] post-stop transcript repair failed".to_string()),
            ..Default::default()
        };
        let from_batch = Event {
            message: Some("batch transcription failed".to_string()),
            ..Default::default()
        };

        for event in [from_message, from_repair, from_batch] {
            assert!(should_drop_sentry_event(&event));
        }
    }

    #[test]
    fn ignored_noise_does_not_change_user_error_drop() {
        let event = Event {
            message: Some("batch transcription failed".to_string()),
            ..Default::default()
        };

        assert!(drop_user_error_event(event).is_some());
    }

    #[test]
    fn sanitizer_removes_identity_and_free_text_but_keeps_safe_diagnostics() {
        let event = Event {
            message: Some("patient@example.com".to_string()),
            user: Some(sentry::User {
                id: Some("user-1".to_string()),
                email: Some("patient@example.com".to_string()),
                ..Default::default()
            }),
            tags: [
                ("error.type".to_string(), "database_error".to_string()),
                ("enduser.id".to_string(), "user-1".to_string()),
            ]
            .into_iter()
            .collect(),
            exception: vec![Exception {
                ty: "DatabaseError".to_string(),
                value: Some("patient@example.com".to_string()),
                ..Default::default()
            }]
            .into(),
            ..Default::default()
        };

        let sanitized = sanitize_sentry_event(event).unwrap();
        assert!(sanitized.user.is_none());
        assert!(sanitized.message.is_none());
        assert_eq!(sanitized.tags.get("error.type").unwrap(), "database_error");
        assert!(!sanitized.tags.contains_key("enduser.id"));
        assert_eq!(
            sanitized.exception[0].value.as_deref(),
            Some("DatabaseError captured")
        );
    }

    #[test]
    fn sanitizer_keeps_safe_grouping_for_stackless_events() {
        let event = Event {
            message: Some("database_write_failed".to_string()),
            ..Default::default()
        };

        let sanitized = sanitize_sentry_event(event).unwrap();
        assert_eq!(sanitized.message.as_deref(), Some("database_write_failed"));
        assert_eq!(
            sanitized.tags.get("anarlog.operation").map(String::as_str),
            Some("database_write_failed")
        );
        assert_eq!(
            sanitized
                .fingerprint
                .iter()
                .map(|value| value.as_ref())
                .collect::<Vec<_>>(),
            vec!["server_error", "database_write_failed"]
        );
    }
}
