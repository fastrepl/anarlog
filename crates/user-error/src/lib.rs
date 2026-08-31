//! Classification of failures that should still be logged locally but must not
//! be reported to Sentry.
//!
//! This includes the end user's own account state (exhausted credits, expired
//! plans, bad API keys) and issue types that were already archived as solved.

use sentry::protocol::{Context, Event, Value};

const USER_ERROR_MARKERS: &[&str] = &[
    "billing_hard_limit_reached",
    "credit balance is too low",
    "exceeded your current quota",
    "incorrect api key",
    "insufficient balance",
    "insufficient credits",
    "insufficient funds",
    "insufficient_quota",
    "invalid api key",
    "invalid x-api-key",
    "invalid_api_key",
    "not enough credits",
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

#[cfg(test)]
mod tests {
    use super::*;
    use sentry::protocol::{Breadcrumb, Exception, LogEntry};

    const ANTHROPIC_CREDIT_ERROR: &str = "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.";

    #[test]
    fn detects_billing_and_credential_failures() {
        assert!(is_user_error_text(ANTHROPIC_CREDIT_ERROR));
        assert!(is_user_error_text(
            "{\"code\":\"insufficient_quota\",\"type\":\"invalid_request_error\"}"
        ));
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
}
