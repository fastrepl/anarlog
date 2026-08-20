pub fn hhmmss_to_ms(value: &str) -> u64 {
    let parts = value
        .split(':')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>();
    let Some(parts) = parts else {
        return 0;
    };
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return 0,
    };
    hours
        .saturating_mul(3_600_000)
        .saturating_add(minutes.saturating_mul(60_000))
        .saturating_add(seconds.saturating_mul(1_000))
}

pub fn duration_or_timestamp_to_ms(value: &str, origin_ms: u64) -> u64 {
    let trimmed = value.trim();
    if let Some(seconds) = trimmed.strip_suffix('s')
        && let Ok(parsed) = seconds.parse::<f64>()
    {
        return (parsed * 1_000.0).round().clamp(0.0, u64::MAX as f64) as u64;
    }
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        let millis = parsed.timestamp_millis().max(0) as u64;
        return millis.saturating_sub(origin_ms);
    }
    0
}

pub fn rfc3339_to_ms(value: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|parsed| parsed.timestamp_millis().max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_timestamps() {
        assert_eq!(hhmmss_to_ms("00:05:32"), 332_000);
        assert_eq!(hhmmss_to_ms("05:32"), 332_000);
    }

    #[test]
    fn parses_google_durations() {
        assert_eq!(duration_or_timestamp_to_ms("1.500s", 0), 1_500);
        let origin = rfc3339_to_ms("2026-08-01T10:00:00Z").unwrap();
        assert_eq!(
            duration_or_timestamp_to_ms("2026-08-01T10:00:01.500Z", origin),
            1_500
        );
    }
}
