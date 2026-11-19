const MEET_HOST: &str = "meet.google.com";

/// The canonical address `https://meet.google.com/abc-defg-hij` of a Meet call, from a form Meet's
/// page shows (`?authuser=1`, a `#` part, the `/meet/` path, a trailing slash, host case, `:443`).
/// `None` for anything else: http, a lookalike host, credentials, another port or another page.
pub fn meet_call_of(value: &str) -> Option<String> {
    let (scheme, rest) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, rest) = rest.split_at(authority_end);
    if authority.contains('@') {
        return None;
    }
    let host = match authority.rsplit_once(':') {
        Some((host, "443")) => host,
        Some(_) => return None,
        None => authority,
    };
    if !host.eq_ignore_ascii_case(MEET_HOST) {
        return None;
    }
    let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = rest[..path_end].strip_prefix('/')?;
    let path = path.strip_prefix("meet/").unwrap_or(path);
    let code = path.strip_suffix('/').unwrap_or(path);
    is_meeting_code(code).then(|| format!("https://{MEET_HOST}/{code}"))
}

/// Whether both addresses name the same Meet call. Raw strings are never compared.
pub fn same_meet_call(a: &str, b: &str) -> bool {
    matches!((meet_call_of(a), meet_call_of(b)), (Some(a), Some(b)) if a == b)
}

/// Whether `value` is already a canonical call address, the only form the extension sends.
pub fn is_canonical_meet_call(value: &str) -> bool {
    meet_call_of(value).as_deref() == Some(value)
}

/// The `abc-defg-hij` part of a canonical call address.
pub fn meeting_code_of(call: &str) -> Option<&str> {
    call.strip_prefix("https://meet.google.com/")
        .filter(|code| is_meeting_code(code))
}

fn is_meeting_code(code: &str) -> bool {
    let groups: Vec<&str> = code.split('-').collect();
    groups.len() == 3
        && groups.iter().zip([3, 4, 3]).all(|(group, len)| {
            group.len() == len && group.bytes().all(|byte| byte.is_ascii_lowercase())
        })
}

/// The Chromium browsers whose Meet tabs the extension can link, by macOS bundle id. A helper
/// process is signed with its browser's id plus a suffix (`com.google.Chrome.helper`).
const BROWSERS: &[&str] = &[
    "com.google.Chrome",
    "com.google.Chrome.beta",
    "com.google.Chrome.dev",
    "com.google.Chrome.canary",
    "org.chromium.Chromium",
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.brave.Browser.nightly",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.edgemac.Canary",
    "company.thebrowser.Browser",
];

/// The browser a bundle id belongs to, or `None` for any other app. The longest match wins, so
/// Chrome Canary's helper is Canary, not Chrome.
pub fn browser_family_of(bundle_id: &str) -> Option<&'static str> {
    BROWSERS
        .iter()
        .copied()
        .filter(|browser| {
            bundle_id == *browser
                || bundle_id
                    .strip_prefix(browser)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .max_by_key(|browser| browser.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEET: &str = "https://meet.google.com/abc-defg-hij";

    #[test]
    fn canonicalizes_only_the_forms_meet_shows() {
        for shown in [
            MEET,
            "https://meet.google.com/abc-defg-hij/",
            "https://meet.google.com/abc-defg-hij?authuser=0",
            "https://meet.google.com/abc-defg-hij?authuser=1&hs=122#chat",
            "https://meet.google.com/meet/abc-defg-hij",
            "https://MEET.google.com:443/abc-defg-hij",
        ] {
            assert_eq!(meet_call_of(shown).as_deref(), Some(MEET), "{shown}");
        }
        for other in [
            "http://meet.google.com/abc-defg-hij",
            "https://meet.google.com.evil.example/abc-defg-hij",
            "https://evil.example/meet.google.com/abc-defg-hij",
            "https://user:pw@meet.google.com/abc-defg-hij",
            "https://meet.google.com@evil.example/abc-defg-hij",
            "https://meet.google.com:8443/abc-defg-hij",
            "https://meet.google.com/abc-defg-hij/extra",
            "https://meet.google.com/ABC-DEFG-HIJ",
            "https://meet.google.com/abc-defg-hijk",
            "https://meet.google.com/landing",
            "https://meet.google.com/",
            "https://meet.google.com",
            "not a url",
        ] {
            assert_eq!(meet_call_of(other), None, "{other}");
        }
        assert!(same_meet_call(&format!("{MEET}?authuser=1"), MEET));
        assert!(!same_meet_call(
            MEET,
            "https://meet.google.com/xxx-yyyy-zzz"
        ));
        assert!(!same_meet_call(
            "https://meet.google.com/",
            "https://meet.google.com/"
        ));
        assert!(is_canonical_meet_call(MEET));
        assert!(!is_canonical_meet_call(&format!("{MEET}?authuser=1")));
        assert_eq!(meeting_code_of(MEET), Some("abc-defg-hij"));
    }

    #[test]
    fn names_a_browser_by_its_own_id_or_helper_never_by_a_prefix() {
        assert_eq!(
            browser_family_of("com.google.Chrome.helper"),
            Some("com.google.Chrome")
        );
        assert_eq!(
            browser_family_of("com.google.Chrome.canary.helper"),
            Some("com.google.Chrome.canary")
        );
        assert_eq!(browser_family_of("com.google.ChromeEvil"), None);
        assert_eq!(browser_family_of("us.zoom.xos"), None);
        assert_eq!(browser_family_of("com.microsoft.teams2"), None);
    }
}
