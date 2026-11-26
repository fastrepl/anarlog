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
        .and_then(|setting| setting.value.as_bool())
        .unwrap_or(telemetry);
    (telemetry, crash)
}

pub fn consent_from_rows(rows: &[(String, String)]) -> (bool, bool) {
    let read = |id: &str| {
        rows.iter()
            .find(|(key, _)| key == id)
            .and_then(|(_, value)| serde_json::from_str::<bool>(value).ok())
    };
    let telemetry = read("telemetry_consent").unwrap_or(true);
    let crash = read("crash_reporting_consent").unwrap_or(telemetry);
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
    use super::*;

    #[test]
    fn consent_from_rows_defaults_to_true_and_inherits_telemetry_for_missing_crash_flag() {
        assert_eq!(consent_from_rows(&[]), (true, true));
        assert_eq!(
            consent_from_rows(&[("telemetry_consent".into(), "false".into())]),
            (false, false)
        );
        assert_eq!(
            consent_from_rows(&[
                ("telemetry_consent".into(), "false".into()),
                ("crash_reporting_consent".into(), "true".into()),
            ]),
            (false, true)
        );
        assert_eq!(
            consent_from_rows(&[
                ("telemetry_consent".into(), "true".into()),
                ("crash_reporting_consent".into(), "false".into()),
            ]),
            (true, false)
        );
    }
}
