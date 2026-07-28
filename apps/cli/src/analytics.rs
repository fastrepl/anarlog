use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn capture_command_completed(command: &str, outcome: &str, duration: Duration) {
    if !telemetry_enabled() {
        return;
    }

    let Some(posthog_key) = posthog_key() else {
        return;
    };
    let Some(distinct_id) = distinct_id() else {
        return;
    };

    let host = std::env::var("POSTHOG_HOST")
        .unwrap_or_else(|_| "https://us.i.posthog.com".to_string())
        .trim_end_matches('/')
        .to_string();
    let payload = serde_json::json!({
        "api_key": posthog_key,
        "event": "cli_command_completed",
        "properties": {
            "distinct_id": distinct_id,
            "$session_id": new_id(),
            "surface": "cli",
            "analytics_schema_version": 1,
            "app_version": env!("CARGO_PKG_VERSION"),
            "command": command,
            "outcome": outcome,
            "duration_ms": duration.as_millis(),
        },
    });

    let Ok(mut child) = Command::new("curl")
        .args([
            "--silent",
            "--fail",
            "--max-time",
            "0.75",
            "--request",
            "POST",
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            "@-",
            &format!("{host}/capture/"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = serde_json::to_writer(&mut stdin, &payload);
        let _ = stdin.flush();
    }
    let _ = child.wait();
}

pub fn telemetry_enabled() -> bool {
    std::env::var("ANARLOG_ANALYTICS")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn posthog_key() -> Option<String> {
    std::env::var("POSTHOG_API_KEY")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            option_env!("POSTHOG_API_KEY")
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        })
}

fn distinct_id() -> Option<String> {
    let path = analytics_id_path()?;

    if let Ok(value) = fs::read_to_string(&path) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }

    let value = new_id();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, &value);
    Some(value)
}

fn analytics_id_path() -> Option<PathBuf> {
    dirs::config_dir().map(|path| path.join("anarlog").join("analytics-id"))
}

fn new_id() -> String {
    let mut hasher = DefaultHasher::new();
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("cli-{:016x}", hasher.finish())
}
