use serde_json::from_str;

pub const CONSENT_QUERY: &str = "SELECT id, value_json FROM app_settings \
    WHERE id IN ('crash_reporting_consent', 'telemetry_consent')";

pub fn from_rows(rows: &[(String, String)]) -> bool {
    let read = |id: &str| {
        rows.iter()
            .find(|(key, _)| key == id)
            .and_then(|(_, value)| from_str::<bool>(value).ok())
    };

    read("crash_reporting_consent")
        .or_else(|| read("telemetry_consent"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::from_rows;

    #[test]
    fn prefers_crash_reporting_consent_over_legacy_telemetry() {
        let rows = vec![
            ("telemetry_consent".to_string(), "false".to_string()),
            ("crash_reporting_consent".to_string(), "true".to_string()),
        ];

        assert!(from_rows(&rows));
        assert!(!from_rows(&rows[..1]));
        assert!(!from_rows(&[]));
    }
}
