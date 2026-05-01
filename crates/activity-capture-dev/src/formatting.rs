use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local};
use url::Url;

pub fn compact(value: &str, limit: usize) -> String {
    let mut collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return "-".to_string();
    }
    if collapsed.chars().count() > limit {
        collapsed = collapsed.chars().take(limit).collect::<String>();
        collapsed.push_str("...");
    }
    collapsed
}

pub fn compact_url(value: &str, limit: usize) -> String {
    let compacted = match Url::parse(value) {
        Ok(url) => {
            let mut compacted = match url.host_str() {
                Some(host) => format!("{}://{host}", url.scheme()),
                None => format!("{}:", url.scheme()),
            };

            let mut suffix = url.path().to_string();
            if let Some(query) = url.query() {
                suffix.push('?');
                suffix.push_str(query);
            }
            if !suffix.is_empty() && suffix != "/" {
                compacted.push_str(&suffix);
            }

            compacted
        }
        Err(_) => value.to_string(),
    };

    compact(&compacted, limit)
}

pub fn format_timestamp(time: SystemTime) -> String {
    let local: DateTime<Local> = time.into();
    local.format("%H:%M:%S").to_string()
}

pub fn format_duration(duration: Duration) -> String {
    if duration.as_millis() < 1_000 {
        return format!("{}ms", duration.as_millis());
    }

    if duration.as_secs() < 60 {
        return format!("{:.1}s", duration.as_secs_f64());
    }

    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() % 60;
    format!("{minutes}m{seconds:02}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_short_duration_in_millis() {
        assert_eq!(format_duration(Duration::from_millis(275)), "275ms");
    }

    #[test]
    fn formats_second_duration_with_fraction() {
        assert_eq!(format_duration(Duration::from_millis(1_500)), "1.5s");
    }

    #[test]
    fn formats_minute_duration() {
        assert_eq!(format_duration(Duration::from_secs(125)), "2m05s");
    }
}
