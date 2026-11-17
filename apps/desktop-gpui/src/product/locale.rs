//! Language, timezone and week-start helpers matching `settings/general/{language,timezone,
//! week-start}.tsx`. Display names are the English `Intl.DisplayNames` output.

/// `CORE_TRANSCRIPTION_LANGUAGE_CODES` with their English names.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("ar", "Arabic"),
    ("be", "Belarusian"),
    ("bg", "Bulgarian"),
    ("bn", "Bangla"),
    ("bs", "Bosnian"),
    ("ca", "Catalan"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("de", "German"),
    ("el", "Greek"),
    ("en", "English"),
    ("es", "Spanish"),
    ("et", "Estonian"),
    ("fa", "Persian"),
    ("fi", "Finnish"),
    ("fr", "French"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hr", "Croatian"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ja", "Japanese"),
    ("kn", "Kannada"),
    ("ko", "Korean"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("mk", "Macedonian"),
    ("mr", "Marathi"),
    ("ms", "Malay"),
    ("nl", "Dutch"),
    ("no", "Norwegian"),
    ("pl", "Polish"),
    ("pt", "Portuguese"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("sr", "Serbian"),
    ("sv", "Swedish"),
    ("ta", "Tamil"),
    ("te", "Telugu"),
    ("th", "Thai"),
    ("tl", "Filipino"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("ur", "Urdu"),
    ("vi", "Vietnamese"),
    ("zh", "Chinese"),
];

/// `COMMON_TIMEZONES`: (IANA name, label, UTC offset detail).
pub const TIMEZONES: &[(&str, &str, &str)] = &[
    ("Pacific/Honolulu", "Hawaii", "UTC-10"),
    ("America/Anchorage", "Alaska", "UTC-9"),
    ("America/Los_Angeles", "Pacific Time", "UTC-8"),
    ("America/Denver", "Mountain Time", "UTC-7"),
    ("America/Chicago", "Central Time", "UTC-6"),
    ("America/New_York", "Eastern Time", "UTC-5"),
    ("America/Sao_Paulo", "Sao Paulo", "UTC-3"),
    ("Atlantic/Reykjavik", "Reykjavik", "UTC+0"),
    ("Europe/London", "London", "UTC+0/+1"),
    ("Europe/Paris", "Paris", "UTC+1/+2"),
    ("Europe/Berlin", "Berlin", "UTC+1/+2"),
    ("Africa/Cairo", "Cairo", "UTC+2"),
    ("Europe/Moscow", "Moscow", "UTC+3"),
    ("Asia/Dubai", "Dubai", "UTC+4"),
    ("Asia/Kolkata", "India", "UTC+5:30"),
    ("Asia/Bangkok", "Bangkok", "UTC+7"),
    ("Asia/Singapore", "Singapore", "UTC+8"),
    ("Asia/Shanghai", "China", "UTC+8"),
    ("Asia/Tokyo", "Tokyo", "UTC+9"),
    ("Asia/Seoul", "Seoul", "UTC+9"),
    ("Australia/Sydney", "Sydney", "UTC+10/+11"),
    ("Pacific/Auckland", "Auckland", "UTC+12/+13"),
];

/// The ISO 639 base of a locale tag (`en-US` → `en`), using the transcription aliases.
pub fn base_code(code: &str) -> String {
    let language = code
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !(2..=3).contains(&language.len()) || !language.chars().all(|c| c.is_ascii_lowercase()) {
        return String::new();
    }
    match language.as_str() {
        "fil" => "tl".into(),
        "bho" => "bh".into(),
        "iw" => "he".into(),
        "in" => "id".into(),
        _ => language,
    }
}

pub fn language_name(code: &str) -> String {
    let base = base_code(code);
    LANGUAGES
        .iter()
        .find(|(language, _)| *language == base)
        .map(|(_, name)| (*name).to_owned())
        .unwrap_or_else(|| code.to_owned())
}

/// `getAdditionalSpokenLanguages`: base codes, without the main language or duplicates.
pub fn additional_spoken(main: &str, stored: &str) -> Vec<String> {
    let main = base_code(main);
    let mut languages = Vec::new();
    for code in serde_json::from_str::<Vec<String>>(stored).unwrap_or_default() {
        let code = base_code(&code);
        if !code.is_empty() && code != main && !languages.contains(&code) {
            languages.push(code);
        }
    }
    languages
}

/// Languages offered while typing in the spoken-language field: names containing `query`,
/// without the main language or ones already chosen.
pub fn spoken_matches(query: &str, main: &str, selected: &[String]) -> Vec<&'static str> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let main = base_code(main);
    LANGUAGES
        .iter()
        .filter(|(code, name)| {
            *code != main
                && !selected.iter().any(|chosen| chosen == code)
                && name.to_lowercase().contains(&query)
        })
        .map(|(code, _)| *code)
        .collect()
}

/// The IANA zone the OS uses, read from the `/etc/localtime` link on macOS and Linux.
pub fn system_timezone() -> String {
    if let Ok(zone) = std::env::var("TZ")
        && zone.contains('/')
    {
        return zone.trim_start_matches(':').to_owned();
    }
    std::fs::read_link("/etc/localtime")
        .ok()
        .and_then(|path| {
            let path = path.to_string_lossy().into_owned();
            path.split_once("zoneinfo/")
                .map(|(_, zone)| zone.to_owned())
        })
        .unwrap_or_default()
}

/// Regions whose calendar weeks start on Sunday (CLDR `firstDay`), from the POSIX locale.
pub fn system_week_start() -> &'static str {
    const SUNDAY: &[&str] = &[
        "AG", "AS", "BD", "BR", "BS", "BT", "BW", "BZ", "CA", "CN", "CO", "DM", "DO", "ET", "GT",
        "GU", "HK", "HN", "ID", "IL", "IN", "JM", "JP", "KE", "KH", "KR", "LA", "MH", "MM", "MO",
        "MT", "MX", "MZ", "NI", "NP", "PA", "PE", "PH", "PK", "PR", "PT", "PY", "SA", "SG", "SV",
        "TH", "TT", "TW", "UM", "US", "VE", "VI", "WS", "YE", "ZA", "ZW",
    ];
    let locale = ["LC_ALL", "LC_TIME", "LANG"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default();
    let region = locale
        .split('.')
        .next()
        .and_then(|tag| tag.split(['_', '-']).nth(1))
        .unwrap_or_default();
    if region.is_empty() || SUNDAY.contains(&region) {
        "sunday"
    } else {
        "monday"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additional_languages_drop_the_main_language_and_duplicates() {
        assert_eq!(
            additional_spoken("en-US", r#"["en","ko-KR","ko","fil","bad tag"]"#),
            ["ko", "tl"]
        );
        assert!(additional_spoken("en", "not json").is_empty());
        assert_eq!(spoken_matches(" KOR ", "en-US", &[]), ["ko"]);
        assert!(spoken_matches("kor", "en", &["ko".into()]).is_empty());
        assert!(spoken_matches("eng", "en-US", &[]).is_empty());
        assert!(spoken_matches("  ", "en", &[]).is_empty());
        assert_eq!(language_name("ko-KR"), "Korean");
        assert_eq!(language_name("xx"), "xx");
    }
}
