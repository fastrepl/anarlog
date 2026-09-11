pub mod consent;
pub mod filter;
pub mod redaction;

use std::sync::atomic::{AtomicBool, Ordering};

use sentry::integrations::tracing::SentryLayer;
use tracing::Subscriber;
use tracing_subscriber::registry::LookupSpan;

static ENABLED: AtomicBool = AtomicBool::new(false);

pub struct Options<'a> {
    pub dsn: Option<&'a str>,
    pub release: Option<String>,
    pub release_channel: &'a str,
    pub service_name: &'a str,
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

pub fn set_enabled(value: bool) {
    ENABLED.store(value, Ordering::SeqCst);
}

pub fn init(options: Options<'_>, initially_enabled: bool) -> Option<sentry::ClientInitGuard> {
    set_enabled(initially_enabled);
    let dsn = options
        .dsn
        .filter(|_| std::env::var_os("ANARLOG_DISABLE_SENTRY").is_none())?;
    let service_name = options.service_name;
    let release_channel = options.release_channel;
    let client = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: options.release.map(Into::into),
            traces_sample_rate: 1.0,
            auto_session_tracking: false,
            before_send: Some(std::sync::Arc::new(|event| {
                enabled()
                    .then(|| redaction::sanitize_sentry_event(event))
                    .flatten()
            })),
            before_breadcrumb: Some(std::sync::Arc::new(|breadcrumb| {
                enabled().then_some(breadcrumb)
            })),
            ..Default::default()
        },
    ));

    sentry::configure_scope(|scope| {
        scope.set_tag("service.namespace", "anarlog");
        scope.set_tag("service.name", service_name);
        scope.set_tag("release_channel", release_channel);
    });

    Some(client)
}

pub fn tracing_layer<S>() -> SentryLayer<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    sentry::integrations::tracing::layer().event_filter(|metadata| {
        filter::sentry_event_filter_for(metadata.level(), metadata.target())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn init_without_dsn_returns_none() {
        let _lock = STATE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        assert!(
            init(
                Options {
                    dsn: None,
                    release: None,
                    release_channel: "dev",
                    service_name: "test",
                },
                true,
            )
            .is_none()
        );
    }

    #[test]
    fn init_disabled_by_environment_returns_none() {
        let _lock = STATE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        if std::env::var_os("ANARLOG_DISABLE_SENTRY").is_none() {
            unsafe { std::env::set_var("ANARLOG_DISABLE_SENTRY", "1") };
            let result = init(
                Options {
                    dsn: Some("https://public@example.com/1"),
                    release: None,
                    release_channel: "dev",
                    service_name: "test",
                },
                true,
            );
            unsafe { std::env::remove_var("ANARLOG_DISABLE_SENTRY") };
            assert!(result.is_none());
        }
    }

    #[test]
    fn enabled_round_trip() {
        let _lock = STATE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        set_enabled(false);
        assert!(!enabled());
        set_enabled(true);
        assert!(enabled());
        set_enabled(false);
    }
}
