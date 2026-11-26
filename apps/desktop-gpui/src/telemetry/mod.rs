mod analytics;
mod sentry;

use std::sync::atomic::{AtomicBool, Ordering};

use anlg_desktop_identity::Channel;
use serde_json::Value;

use crate::product::settings::Snapshot;

static CRASH_REPORTING_ENABLED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_telemetry_enabled(enabled: bool) {
    TELEMETRY_ENABLED.store(enabled, Ordering::SeqCst);
    if enabled {
        analytics::sync_identity();
    }
}

pub fn set_crash_reporting_enabled(enabled: bool) {
    CRASH_REPORTING_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn telemetry_enabled() -> bool {
    TELEMETRY_ENABLED.load(Ordering::SeqCst)
}

pub fn crash_reporting_enabled() -> bool {
    CRASH_REPORTING_ENABLED.load(Ordering::SeqCst)
}

pub fn consent_from_settings(snapshot: &Snapshot) -> (bool, bool) {
    let telemetry = snapshot
        .get("telemetry_consent")
        .and_then(|setting| setting.value.as_bool())
        .unwrap_or(true);
    let crash = snapshot
        .get("crash_reporting_consent")
        .filter(|setting| setting.source != "Default")
        .and_then(|setting| setting.value.as_bool())
        .unwrap_or(telemetry);
    (telemetry, crash)
}

pub fn init_sentry(channel: Option<Channel>) -> Option<::sentry::ClientInitGuard> {
    sentry::init(channel)
}

pub fn init_analytics(channel: Option<Channel>) {
    analytics::init(channel);
}

pub fn sentry_tracing_layer<S>() -> impl tracing_subscriber::Layer<S>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    sentry::tracing_layer()
}

pub fn track(event: impl Into<String>, props: &[(&str, Value)]) {
    analytics::track(event, props);
}

pub fn identify(user_id: &str, plan: &str, trial_end: Option<i64>) {
    analytics::identify(user_id, plan, trial_end);
}

pub fn clear_identity() {
    analytics::clear_identity();
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(id: &str, value_json: &str) -> Value {
        json!({
            "id": id,
            "value_json": value_json,
            "oversized": 0,
            "source_rank": 0,
        })
    }

    #[test]
    fn consent_from_settings_honors_legacy_and_direct_overrides() {
        let defaults = crate::product::settings::decode(&[]).unwrap();
        assert_eq!(consent_from_settings(&defaults), (true, true));

        let legacy_opt_out = crate::product::settings::decode(&[row(
            "legacy_settings_document",
            r#"{"general":{"telemetry_consent":false}}"#,
        )])
        .unwrap();
        assert_eq!(consent_from_settings(&legacy_opt_out), (false, false));

        let legacy_split = crate::product::settings::decode(&[
            row(
                "legacy_settings_document",
                r#"{"general":{"telemetry_consent":false}}"#,
            ),
            row("crash_reporting_consent", "true"),
        ])
        .unwrap();
        assert_eq!(consent_from_settings(&legacy_split), (false, true));

        let direct_override = crate::product::settings::decode(&[
            row(
                "legacy_settings_document",
                r#"{"general":{"telemetry_consent":false}}"#,
            ),
            row("telemetry_consent", "true"),
            row("crash_reporting_consent", "false"),
        ])
        .unwrap();
        assert_eq!(consent_from_settings(&direct_override), (true, false));
    }
}
