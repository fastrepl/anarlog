use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;

const PERSONAL_EMAIL_DOMAINS: &[&str] = &[
    "gmail.com",
    "googlemail.com",
    "yahoo.com",
    "outlook.com",
    "hotmail.com",
    "live.com",
    "msn.com",
    "icloud.com",
    "me.com",
    "mac.com",
    "aol.com",
    "proton.me",
    "protonmail.com",
    "pm.me",
    "hey.com",
    "fastmail.com",
];

const PERSONAL_PROVIDER_LABELS: &[&str] = &[
    "gmail",
    "googlemail",
    "yahoo",
    "ymail",
    "outlook",
    "hotmail",
    "live",
    "msn",
    "icloud",
    "aol",
    "proton",
    "protonmail",
    "gmx",
    "yandex",
    "naver",
    "daum",
    "hanmail",
    "nate",
    "qq",
    "163",
    "126",
    "mail",
    "email",
    "web",
];

const SECOND_LEVEL_SUFFIX_LABELS: &[&str] = &[
    "co", "com", "org", "net", "ac", "edu", "gov", "mil", "ne", "or", "go", "re",
];

static EMAIL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap());
static URL_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://").unwrap());
static LETTER_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}").unwrap());
static NON_ALNUM_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]+").unwrap());
static SEPARATOR_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[._\-]+").unwrap());
static DIGITS_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static WHITESPACE_PATTERN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static NAIVE_DATE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());
static NAIVE_DATETIME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(:\d{2}(\.\d+)?)?$").unwrap());

pub(crate) fn is_email_placeholder_name(name: &str) -> bool {
    let trimmed = name.trim();
    trimmed.is_empty() || EMAIL_PATTERN.is_match(trimmed)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedContactIdentity {
    pub name: String,
    pub name_source: NameSource,
    pub company_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameSource {
    Provider,
    Email,
}

pub(crate) fn derive_contact_identity(name: Option<&str>, email: &str) -> DerivedContactIdentity {
    let trimmed_name = name.unwrap_or("").trim();
    let provided_name = is_likely_person_name(trimmed_name);
    let derived_name = if provided_name {
        trimmed_name.to_string()
    } else {
        let from_email = name_from_email_local_part(email);
        if from_email.is_empty() {
            email.to_string()
        } else {
            from_email
        }
    };
    DerivedContactIdentity {
        name: derived_name,
        name_source: if provided_name {
            NameSource::Provider
        } else {
            NameSource::Email
        },
        company_name: infer_company_name_from_email(email),
    }
}

pub(crate) fn is_likely_person_name(value: &str) -> bool {
    let utf16_len = value.encode_utf16().count();
    if value.is_empty() || !(2..=80).contains(&utf16_len) {
        return false;
    }
    if value.contains('@') || URL_PATTERN.is_match(value) {
        return false;
    }
    let normalized = normalize_name(value);
    if normalized.is_empty()
        || [
            "what",
            "who",
            "invitee timezone",
            "meeting link",
            "zoom",
            "google meet",
            "teams",
        ]
        .contains(&normalized.as_str())
    {
        return false;
    }
    LETTER_PATTERN.find_iter(value).count() >= 2
}

pub(crate) fn name_from_email_local_part(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("");
    let local = local.split('+').next().unwrap_or("");
    SEPARATOR_PATTERN
        .replace_all(local, " ")
        .split(' ')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty() && !DIGITS_PATTERN.is_match(part))
        .map(capitalize_js)
        .collect::<Vec<_>>()
        .join(" ")
}

// JS `part.charAt(0).toUpperCase() + part.slice(1).toLowerCase()` uppercases
// the first UTF-16 code unit and lowercases the remainder; an astral first
// char has no case mapping, so it stays as-is.
fn capitalize_js(part: &str) -> String {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let first: String = if first.len_utf16() == 1 {
        first.to_uppercase().collect()
    } else {
        first.to_string()
    };
    format!("{first}{}", chars.as_str().to_lowercase())
}

pub(crate) fn infer_company_name_from_email(email: &str) -> Option<String> {
    static PERSONAL_DOMAINS: LazyLock<HashSet<&'static str>> =
        LazyLock::new(|| PERSONAL_EMAIL_DOMAINS.iter().copied().collect());
    static PROVIDER_LABELS: LazyLock<HashSet<&'static str>> =
        LazyLock::new(|| PERSONAL_PROVIDER_LABELS.iter().copied().collect());
    let domain = email.split('@').nth(1)?.to_lowercase();
    if domain.is_empty() || PERSONAL_DOMAINS.contains(domain.as_str()) {
        return None;
    }
    let labels: Vec<&str> = domain
        .split('.')
        .filter(|label| !label.is_empty())
        .collect();
    if labels.len() < 2 {
        return None;
    }
    let last = labels[labels.len() - 1];
    let second_last = labels[labels.len() - 2];
    let is_public_suffix_label = SECOND_LEVEL_SUFFIX_LABELS.contains(&second_last)
        || (last.chars().count() == 2 && second_last.chars().count() <= 3);
    let company_label = if labels.len() >= 3 && is_public_suffix_label {
        labels[labels.len() - 3]
    } else {
        second_last
    };
    if company_label.is_empty()
        || company_label.encode_utf16().count() < 2
        || PROVIDER_LABELS.contains(company_label)
    {
        return None;
    }
    let first = company_label
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    let rest = &company_label[company_label.chars().next().map_or(0, |c| c.len_utf8())..];
    normalize_company_name(&format!("{first}{rest}"))
}

pub(crate) fn normalize_company_name(value: &str) -> Option<String> {
    let name = WHITESPACE_PATTERN
        .replace_all(value.trim(), " ")
        .to_string();
    let utf16_len = name.encode_utf16().count();
    if name.is_empty() || !(2..=80).contains(&utf16_len) {
        return None;
    }
    if name.contains('@') || URL_PATTERN.is_match(&name) {
        return None;
    }
    Some(name)
}

pub(crate) fn normalize_name(value: &str) -> String {
    let decomposed: String = value
        .nfkd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();
    let lower = decomposed.to_lowercase();
    NON_ALNUM_PATTERN
        .replace_all(&lower, " ")
        .trim()
        .to_string()
}

// Approximates JS `Date.parse`: RFC 3339 first, then `YYYY-MM-DD` as UTC
// midnight, then naive `YYYY-MM-DD[T| ]HH:mm[:ss[.f]]` in the local timezone.
// Anything else is NaN, which makes comparisons false.
pub(crate) fn js_date_parse_millis(value: &str) -> Option<i64> {
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(value) {
        return Some(parsed.timestamp_millis());
    }
    if NAIVE_DATE_PATTERN.is_match(value) {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
            return date
                .and_hms_opt(0, 0, 0)
                .map(|dt| dt.and_utc().timestamp_millis());
        }
        return None;
    }
    if NAIVE_DATETIME_PATTERN.is_match(value) {
        for format in [
            "%Y-%m-%dT%H:%M:%S%.f",
            "%Y-%m-%dT%H:%M",
            "%Y-%m-%d %H:%M:%S%.f",
            "%Y-%m-%d %H:%M",
        ] {
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(value, format) {
                return naive
                    .and_local_timezone(chrono::Local)
                    .single()
                    .map(|dt| dt.timestamp_millis());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_placeholder_names_match_the_ts_predicate() {
        for (name, expected) in [
            ("", true),
            ("   ", true),
            ("alice@acme.com", true),
            (" alice@acme.com ", true),
            ("Jane @ Acme", false),
            ("Jane Doe", false),
            ("@handle", false),
            ("alice@localhost", false),
            ("김철수", false),
        ] {
            assert_eq!(is_email_placeholder_name(name), expected, "{name}");
        }
    }

    #[test]
    fn derives_names_and_companies_from_emails() {
        let identity = derive_contact_identity(None, "simon.goldstein@ionprotocol.io");
        assert_eq!(identity.name, "Simon Goldstein");
        assert_eq!(identity.name_source, NameSource::Email);
        assert_eq!(identity.company_name.as_deref(), Some("Ionprotocol"));

        let identity = derive_contact_identity(Some("Dr. Alice Smith"), "alice@acme.com");
        assert_eq!(identity.name, "Dr. Alice Smith");
        assert_eq!(identity.name_source, NameSource::Provider);
        assert_eq!(identity.company_name.as_deref(), Some("Acme"));

        let identity = derive_contact_identity(None, "jane.doe@gmail.com");
        assert_eq!(identity.name, "Jane Doe");
        assert_eq!(identity.company_name, None);
    }

    #[test]
    fn capitalizes_the_first_utf16_unit_like_js() {
        assert_eq!(
            name_from_email_local_part("𝒜LICE.smith@acme.com"),
            "𝒜lice Smith"
        );
        assert_eq!(name_from_email_local_part("élodie@x.com"), "Élodie");
    }

    #[test]
    fn parses_dates_like_date_parse() {
        assert_eq!(
            js_date_parse_millis("2026-09-15T10:00:00Z"),
            js_date_parse_millis("2026-09-15T10:00:00.000Z")
        );
        assert_eq!(
            js_date_parse_millis("2026-09-15"),
            Some(
                chrono::NaiveDate::from_ymd_opt(2026, 9, 15)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis()
            )
        );
        assert_eq!(
            js_date_parse_millis("2026-09-16 10:00:00"),
            js_date_parse_millis("2026-09-16T10:00:00")
        );
        assert_eq!(
            js_date_parse_millis("2026-09-16 10:00"),
            js_date_parse_millis("2026-09-16T10:00")
        );
        assert!(js_date_parse_millis("not a date").is_none());
        assert!(js_date_parse_millis("").is_none());
    }
}
