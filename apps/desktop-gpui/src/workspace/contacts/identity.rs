//! Shipping's contact identity helpers (`contacts/identity.ts`).

const PERSONAL_EMAIL_DOMAINS: [&str; 16] = [
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

const PERSONAL_PROVIDER_LABELS: [&str; 24] = [
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

const SECOND_LEVEL_SUFFIX_LABELS: [&str; 12] = [
    "co", "com", "org", "net", "ac", "edu", "gov", "mil", "ne", "or", "go", "re",
];

/// Latin letters whose NFKD form is the base letter plus a combining mark that shipping's
/// `normalizeName` strips.
fn fold(c: char) -> char {
    match c {
        'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' => 'd',
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' => 'h',
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' => 'n',
        'ò'..='ö' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' => 't',
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        c => c,
    }
}

/// Shipping's `normalizeName`: accents dropped, lowercase, letters and digits split by single
/// spaces.
pub(crate) fn normalize_name(value: &str) -> String {
    let mut normalized = String::new();
    let mut gap = false;
    for c in value.chars().flat_map(char::to_lowercase).map(fold) {
        if ('\u{300}'..='\u{36f}').contains(&c) {
            continue;
        }
        if c.is_alphanumeric() {
            if gap && !normalized.is_empty() {
                normalized.push(' ');
            }
            gap = false;
            normalized.push(c);
        } else {
            gap = true;
        }
    }
    normalized
}

/// Shipping's `isLikelyPersonName`.
pub(crate) fn is_likely_person_name(value: &str) -> bool {
    let length = value.encode_utf16().count();
    if !(2..=80).contains(&length) || value.contains('@') || has_url_scheme(value) {
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
    value.chars().filter(|c| c.is_alphabetic()).count() >= 2
}

fn has_url_scheme(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Shipping's `nameFromEmailLocalPart`: `ada.lovelace+x@` reads as "Ada Lovelace".
pub(crate) fn name_from_email_local_part(email: &str) -> String {
    let local = email.split('@').next().unwrap_or_default();
    let local = local.split('+').next().unwrap_or_default();
    local
        .split(['.', '_', '-'])
        .flat_map(|part| part.split(' '))
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.chars().all(|c| c.is_ascii_digit()))
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shipping's `inferCompanyNameFromEmail`: the work domain's company label.
pub(crate) fn infer_company_name_from_email(email: Option<&str>) -> Option<String> {
    let domain = email?.split('@').nth(1)?.to_lowercase();
    if domain.is_empty() || PERSONAL_EMAIL_DOMAINS.contains(&domain.as_str()) {
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
    let public_suffix = SECOND_LEVEL_SUFFIX_LABELS.contains(&second_last)
        || (last.encode_utf16().count() == 2 && second_last.encode_utf16().count() <= 3);
    let company = if labels.len() >= 3 && public_suffix {
        labels[labels.len() - 3]
    } else {
        second_last
    };
    if company.encode_utf16().count() < 2 || PERSONAL_PROVIDER_LABELS.contains(&company) {
        return None;
    }
    let mut chars = company.chars();
    let capitalized: String = chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    normalize_company_name(Some(&capitalized))
}

/// Shipping's `normalizeCompanyName`.
pub(crate) fn normalize_company_name(value: Option<&str>) -> Option<String> {
    let name = value?.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = name.encode_utf16().count();
    if !(2..=80).contains(&length) || name.contains('@') || has_url_scheme(&name) {
        return None;
    }
    Some(name)
}
