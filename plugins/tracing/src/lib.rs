mod commands;
mod errors;
mod ext;
pub mod redaction;
mod utils;

pub use errors::*;
pub use ext::*;
pub use utils::{cleanup_old_daily_logs, make_file_writer};

use sentry::integrations::tracing::EventFilter;
use tauri::Manager;
use tracing_subscriber::{
    EnvFilter, fmt, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt,
};

use utils::cleanup_legacy_logs;

const PLUGIN_NAME: &str = "tracing";
const WEBVIEW_CONSOLE_TARGET: &str = "anarlog.webview.console";

fn sentry_event_filter(metadata: &tracing::Metadata<'_>) -> EventFilter {
    sentry_event_filter_for(metadata.level(), metadata.target())
}

fn is_webview_console_target(target: &str) -> bool {
    target == WEBVIEW_CONSOLE_TARGET
        || target == "hyprnote.webview.console"
        || target.starts_with("tauri_plugin_tracing")
}

fn sentry_event_filter_for(level: &tracing::Level, target: &str) -> EventFilter {
    if is_webview_console_target(target) {
        return EventFilter::Ignore;
    }

    match *level {
        tracing::Level::ERROR => EventFilter::Event,
        tracing::Level::WARN | tracing::Level::INFO => EventFilter::Breadcrumb,
        tracing::Level::DEBUG | tracing::Level::TRACE => EventFilter::Ignore,
    }
}

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![])
        .commands(tauri_specta::collect_commands![
            commands::logs_dir::<tauri::Wry>,
            commands::do_log::<tauri::Wry>,
            commands::log_content::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

#[derive(Default)]
pub struct Builder {
    skip_subscriber_init: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn skip_subscriber_init(mut self, skip: bool) -> Self {
        self.skip_subscriber_init = skip;
        self
    }

    pub fn build(self) -> tauri::plugin::TauriPlugin<tauri::Wry> {
        let specta_builder = make_specta_builder();
        let skip_subscriber_init = self.skip_subscriber_init;

        tauri::plugin::Builder::new(PLUGIN_NAME)
            .invoke_handler(specta_builder.invoke_handler())
            .js_init_script(JS_INIT_SCRIPT)
            .setup(move |app, _api| {
                specta_builder.mount_events(app);

                cleanup_legacy_logs(app);

                if skip_subscriber_init {
                    return Ok(());
                }

                let env_filter = EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("info"))
                    .add_directive("ort=warn".parse().unwrap())
                    .add_directive("tantivy=error".parse().unwrap());

                let sentry_layer =
                    sentry::integrations::tracing::layer().event_filter(sentry_event_filter);

                let logs_dir = match app.tracing().logs_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!("Failed to create logs directory: {}", e);
                        return Ok(());
                    }
                };
                match make_file_writer(&logs_dir) {
                    Ok((file_writer, guard)) => {
                        tracing_subscriber::Registry::default()
                            .with(env_filter)
                            .with(sentry_layer)
                            .with(
                                fmt::layer().with_writer(|| {
                                    redaction::RedactingWriter::new(std::io::stderr())
                                }),
                            )
                            .with(fmt::layer().with_ansi(false).with_writer(file_writer))
                            .init();
                        app.manage(guard);
                    }
                    Err(error) => {
                        eprintln!("Failed to create log file: {error}");
                        tracing_subscriber::Registry::default()
                            .with(env_filter)
                            .with(sentry_layer)
                            .with(
                                fmt::layer().with_writer(|| {
                                    redaction::RedactingWriter::new(std::io::stderr())
                                }),
                            )
                            .init();
                    }
                }

                Ok(())
            })
            .build()
    }
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    Builder::new().build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    fn create_mock_app() -> tauri::App<tauri::test::MockRuntime> {
        let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
        ctx.config_mut().identifier = "com.hyprnote.dev".to_string();
        ctx.config_mut().version = Some("0.0.1".to_string());

        tauri::test::mock_builder().build(ctx).unwrap()
    }

    #[test]
    fn test_log_content_empty() {
        let app = create_mock_app();
        let result = app.tracing().log_content();
        assert!(result.is_ok());
    }

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
