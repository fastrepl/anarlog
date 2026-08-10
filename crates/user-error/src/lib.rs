//! Classification of failures caused by the end user's own account state
//! (exhausted credits, expired plans, bad API keys) rather than by a defect in
//! Anarlog. These are not actionable for engineering, so they are dropped
//! before reaching Sentry.

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

pub fn is_user_error_text(text: &str) -> bool {
    let text = text.to_lowercase();
    USER_ERROR_MARKERS
        .iter()
        .any(|marker| text.contains(marker))
}

fn value_is_user_error(value: &Value) -> bool {
    match value {
        Value::String(text) => is_user_error_text(text),
        Value::Array(values) => values.iter().any(value_is_user_error),
        Value::Object(values) => values.values().any(value_is_user_error),
        _ => false,
    }
}

pub fn is_user_error_event(event: &Event<'_>) -> bool {
    let message = event.message.as_deref().is_some_and(is_user_error_text);
    let logentry = event.logentry.as_ref().is_some_and(|entry| {
        is_user_error_text(&entry.message) || entry.params.iter().any(value_is_user_error)
    });
    let exception = event.exception.iter().any(|exception| {
        exception.value.as_deref().is_some_and(is_user_error_text)
            || is_user_error_text(&exception.ty)
    });
    let extra = event.extra.values().any(value_is_user_error);
    let tags = event.tags.values().any(|tag| is_user_error_text(tag));
    let contexts = event.contexts.values().any(|context| match context {
        Context::Other(values) => values.values().any(value_is_user_error),
        _ => false,
    });

    message || logentry || exception || extra || tags || contexts
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
}
