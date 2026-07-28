use std::time::Duration;

pub fn init() -> Option<sentry::ClientInitGuard> {
    if !crate::analytics::telemetry_enabled() {
        return None;
    }

    let dsn = std::env::var("SENTRY_DSN")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| option_env!("SENTRY_DSN").map(ToOwned::to_owned))?;
    let guard = sentry::init(sentry::ClientOptions {
        dsn: dsn.parse().ok(),
        release: Some(format!("anarlog-cli@{}", env!("CARGO_PKG_VERSION")).into()),
        environment: Some(if cfg!(debug_assertions) {
            "development".into()
        } else {
            "production".into()
        }),
        send_default_pii: false,
        attach_stacktrace: true,
        ..Default::default()
    });
    sentry::configure_scope(|scope| {
        scope.set_tag("service.name", "cli");
        scope.set_tag("service.namespace", "hyprnote");
        scope.set_tag("hyprnote.surface", "cli");
    });
    Some(guard)
}

pub fn capture_command_error(command: &str, error_code: &str) {
    sentry::with_scope(
        |scope| {
            scope.set_tag("hyprnote.operation", "cli_command");
            scope.set_tag("hyprnote.command", command);
            scope.set_tag("error.code", error_code);
        },
        || {
            sentry::capture_message("CLI command failed", sentry::Level::Error);
        },
    );
}

pub fn flush() {
    if let Some(client) = sentry::Hub::current().client() {
        client.close(Some(Duration::from_millis(750)));
    }
}
