use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn env_usize(keys: &[&str], default: usize) -> usize {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

pub(crate) fn env_string(keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .unwrap_or_else(|| default.to_string())
}

pub(crate) fn sanitize_case_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(crate) fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
