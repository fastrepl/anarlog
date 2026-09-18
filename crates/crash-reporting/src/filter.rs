use sentry::integrations::tracing::EventFilter;

pub const WEBVIEW_CONSOLE_TARGET: &str = "anarlog.webview.console";

pub fn is_webview_console_target(target: &str) -> bool {
    target == WEBVIEW_CONSOLE_TARGET
        || target == "hyprnote.webview.console"
        || target.starts_with("tauri_plugin_tracing")
}

pub fn sentry_event_filter_for(level: &tracing::Level, target: &str) -> EventFilter {
    if is_webview_console_target(target) {
        return EventFilter::Ignore;
    }

    match *level {
        tracing::Level::ERROR => EventFilter::Event,
        tracing::Level::WARN | tracing::Level::INFO => EventFilter::Breadcrumb,
        tracing::Level::DEBUG | tracing::Level::TRACE => EventFilter::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentry_filter_keeps_only_native_errors_as_events() {
        assert_eq!(
            sentry_event_filter_for(&tracing::Level::ERROR, "native").bits(),
            EventFilter::Event.bits()
        );
        assert_eq!(
            sentry_event_filter_for(&tracing::Level::WARN, "native").bits(),
            EventFilter::Breadcrumb.bits()
        );
        assert_eq!(
            sentry_event_filter_for(&tracing::Level::ERROR, WEBVIEW_CONSOLE_TARGET).bits(),
            EventFilter::Ignore.bits()
        );
        assert_eq!(
            sentry_event_filter_for(&tracing::Level::ERROR, "tauri_plugin_tracing::ext").bits(),
            EventFilter::Ignore.bits()
        );
        assert_eq!(
            sentry_event_filter_for(&tracing::Level::ERROR, "hyprnote.webview.console").bits(),
            EventFilter::Ignore.bits()
        );
    }
}
