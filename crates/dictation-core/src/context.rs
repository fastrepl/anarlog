//! Whether a dictation may read the window it is typed into, and the bounded, operation-scoped
//! snapshot of what it read. Nothing here reads anything: the native side asks `decide_context`
//! before it reads, and builds a snapshot only from an allowed decision.

use serde::{Deserialize, Serialize};

use crate::terms::{OrdinaryWords, distinctive_terms, rank_sources};
use crate::text::{char_len, head_chars, tail_chars};

/// The most of the text before the caret a cleanup request carries.
pub const MAX_BEFORE_LENGTH: usize = 1_000;
/// The newest part of the window's text cleanup reads, nearest the field.
pub const MAX_SCREEN_LENGTH: usize = 1_500;
const MAX_HEADING_LENGTH: usize = 200;
pub const MAX_CONTEXT_TERMS: usize = 40;
const APP_TERMS: usize = 5;
/// Nothing gathered outlives the dictation it was for.
pub const CONTEXT_TTL_MS: u64 = 10 * 60 * 1000;

/// What the native side knows about the window and field a dictation is typed into. Every
/// privacy fact is `None` when it couldn't be told, and an unknown fact denies the read.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowFacts {
    pub bundle_id: Option<String>,
    pub app_name: Option<String>,
    pub title: Option<String>,
    /// The focused field is a password or other secure field.
    pub secure_field: Option<bool>,
    /// The window is a private or incognito one. Apps without private windows report `false`.
    pub private_window: Option<bool>,
    /// For a browser, the host of the page as read from the browser itself (never its title).
    pub browser_host: Option<String>,
}

/// What the person keeps out of dictation context.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextExclusions {
    /// App names or bundle ids, matched case-insensitively.
    pub apps: Vec<String>,
    /// Website domains; a subdomain of an excluded domain is excluded too.
    pub websites: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum DenyReason {
    /// Context is off for this dictation.
    Disabled,
    /// The exclusions couldn't be read.
    ExclusionsUnreadable,
    /// The app couldn't be identified.
    UnknownApp,
    PasswordManager,
    /// A browser that is always private (Tor, Mullvad).
    PrivateBrowser,
    SecureField,
    /// Whether the field is secure couldn't be told.
    UnknownSecureField,
    PrivateWindow,
    /// Whether the window is private couldn't be told.
    UnknownPrivateWindow,
    ExcludedApp,
    ExcludedSite,
    /// A browser whose page host couldn't be read from the browser itself.
    UnverifiedBrowserHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContextAccess {
    Allowed,
    Denied { reason: DenyReason },
}

/// Whether a dictation may read this window's title and text for context. `enabled` is the
/// person's explicit choice for this dictation; `exclusions` is `None` while unreadable.
pub fn decide_context(
    enabled: bool,
    facts: &WindowFacts,
    exclusions: Option<&ContextExclusions>,
) -> ContextAccess {
    let denied = |reason| ContextAccess::Denied { reason };
    if !enabled {
        return denied(DenyReason::Disabled);
    }
    let Some(exclusions) = exclusions else {
        return denied(DenyReason::ExclusionsUnreadable);
    };
    if let Some(reason) = local_read_denial(facts, Some(exclusions)) {
        return denied(reason);
    }
    let bundle_id = facts
        .bundle_id
        .as_deref()
        .unwrap_or_default()
        .to_lowercase();
    let title = facts.title.as_deref().unwrap_or_default();
    if is_browser(&bundle_id) {
        let Some(host) = facts.browser_host.as_deref().and_then(normalize_host) else {
            return denied(DenyReason::UnverifiedBrowserHost);
        };
        if exclusions
            .websites
            .iter()
            .filter_map(|domain| normalize_host(domain))
            .any(|domain| host == domain || host.ends_with(&format!(".{domain}")))
        {
            return denied(DenyReason::ExcludedSite);
        }
        return ContextAccess::Allowed;
    }
    if exclusions
        .websites
        .iter()
        .any(|domain| title_names_site(title, domain))
    {
        return denied(DenyReason::ExcludedSite);
    }
    ContextAccess::Allowed
}

/// Whether the text around the caret may be read to fit a dictation into it on this device: no
/// secure or private field, password manager, or excluded app. Unlike `decide_context`, it needs
/// no context opt-in and no verified browser host, because what it reads never leaves the
/// device and is never kept.
pub fn may_read_for_fitting(facts: &WindowFacts, exclusions: Option<&ContextExclusions>) -> bool {
    local_read_denial(facts, exclusions).is_none()
}

fn local_read_denial(
    facts: &WindowFacts,
    exclusions: Option<&ContextExclusions>,
) -> Option<DenyReason> {
    let bundle_id = facts
        .bundle_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_lowercase);
    let Some(bundle_id) = bundle_id else {
        return Some(DenyReason::UnknownApp);
    };
    if PASSWORD_MANAGERS.contains(&bundle_id.as_str()) {
        return Some(DenyReason::PasswordManager);
    }
    if PRIVATE_BROWSERS.contains(&bundle_id.as_str()) {
        return Some(DenyReason::PrivateBrowser);
    }
    match facts.secure_field {
        Some(true) => return Some(DenyReason::SecureField),
        None => return Some(DenyReason::UnknownSecureField),
        Some(false) => {}
    }
    match facts.private_window {
        Some(true) => return Some(DenyReason::PrivateWindow),
        None => return Some(DenyReason::UnknownPrivateWindow),
        Some(false) => {}
    }
    if facts.title.as_deref().is_some_and(title_looks_private) {
        return Some(DenyReason::PrivateWindow);
    }
    let app_name = facts
        .app_name
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if exclusions.is_some_and(|exclusions| {
        exclusions.apps.iter().any(|app| {
            let app = app.trim().to_lowercase();
            !app.is_empty() && (app == app_name || app == bundle_id)
        })
    }) {
        return Some(DenyReason::ExcludedApp);
    }
    None
}

/// What a dictation read from its window, for its own operation only. Bounded at collection
/// and again when used; never written to recovery files or telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshot {
    pub operation_id: String,
    pub captured_at_ms: u64,
    /// The end of the text before the caret.
    pub before: Option<String>,
    /// The window's newest text under a line naming the app and window.
    pub screen: Option<String>,
    /// Distinctive terms from the app, the field and the window, most relevant first.
    pub terms: Vec<String>,
}

impl ContextSnapshot {
    /// The snapshot of what was read, or `None` when `access` denied the read or nothing was
    /// read. `before` is the field's text before the caret; `window_text` the window's text.
    pub fn collect(
        operation_id: &str,
        captured_at_ms: u64,
        access: &ContextAccess,
        facts: &WindowFacts,
        before: Option<&str>,
        window_text: Option<&str>,
        ordinary: Option<&OrdinaryWords>,
    ) -> Option<Self> {
        if *access != ContextAccess::Allowed {
            return None;
        }
        let before = before
            .map(|text| tail_chars(text, MAX_BEFORE_LENGTH))
            .filter(|text| !text.trim().is_empty());
        let window_text = window_text.map(str::trim).filter(|text| !text.is_empty());
        let browser = is_browser(
            &facts
                .bundle_id
                .as_deref()
                .unwrap_or_default()
                .to_lowercase(),
        );
        let heading = [facts.app_name.as_deref(), facts.title.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" — ");
        // A browser names itself in every window; its title names the site instead.
        let named = [
            facts.title.as_deref(),
            if browser {
                None
            } else {
                facts.app_name.as_deref()
            },
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n");
        let app_terms = distinctive_terms(&named, ordinary, true, APP_TERMS);
        let field_terms = before
            .map(|text| distinctive_terms(text, ordinary, false, MAX_CONTEXT_TERMS))
            .unwrap_or_default();
        let window_terms = window_text
            .map(|text| distinctive_terms(text, ordinary, false, MAX_CONTEXT_TERMS))
            .unwrap_or_default();
        let terms = rank_sources(
            [&app_terms, &field_terms, &window_terms].map(|list| list.iter().map(String::as_str)),
            MAX_CONTEXT_TERMS,
        );
        let screen = window_text.map(|text| {
            let head = head_chars(&heading, MAX_HEADING_LENGTH);
            let body = tail_chars(text, MAX_SCREEN_LENGTH);
            if head.is_empty() {
                body.to_owned()
            } else {
                format!("{head}\n\n{body}")
            }
        });
        if before.is_none() && screen.is_none() && terms.is_empty() {
            return None;
        }
        Some(Self {
            operation_id: operation_id.to_owned(),
            captured_at_ms,
            before: before.map(str::to_owned),
            screen,
            terms,
        })
    }

    /// This snapshot, bounded again, when it belongs to `operation_id` and is still fresh.
    pub(crate) fn usable_for(self, operation_id: &str, now_ms: u64) -> Option<Self> {
        let fresh = self.captured_at_ms <= now_ms && now_ms - self.captured_at_ms <= CONTEXT_TTL_MS;
        if self.operation_id != operation_id || !fresh {
            return None;
        }
        let bound = |text: Option<String>, limit: usize| {
            text.filter(|text| !text.trim().is_empty())
                .map(|text| tail_chars(&text, limit).to_owned())
        };
        let mut terms = self.terms;
        terms.truncate(MAX_CONTEXT_TERMS);
        Some(Self {
            before: bound(self.before, MAX_BEFORE_LENGTH),
            screen: bound(self.screen, MAX_HEADING_LENGTH + 2 + MAX_SCREEN_LENGTH),
            terms,
            ..self
        })
    }
}

fn normalize_host(raw: &str) -> Option<String> {
    let host = raw.trim().to_lowercase();
    let host = host
        .split_once("://")
        .map_or(host.as_str(), |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, rest)| rest);
    let host = host.split(':').next().unwrap_or_default();
    let host = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .trim_end_matches('.');
    (!host.is_empty()).then(|| host.to_owned())
}

/// An app that shows a website in its own window (Slack, Notion) names it in its title, so an
/// excluded website is matched by its domain, or by its name ("chase.com" in "Chase - Accounts")
/// when that is two letters or more.
fn title_names_site(title: &str, domain: &str) -> bool {
    let Some(host) = normalize_host(domain) else {
        return false;
    };
    let lower = title.to_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower.contains(&host) {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    // A subdomain such as mail.google.com is matched only in full: its name is Google's.
    if labels.len() != 2 || char_len(labels[0]) < 2 {
        return false;
    }
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word == labels[0])
}

fn title_looks_private(title: &str) -> bool {
    let lower = title.to_lowercase();
    PRIVATE_TITLES.iter().any(|marker| lower.contains(marker))
}

/// A browser by its macOS bundle id (`com.google.chrome.beta` by its `com.google.chrome`), or by
/// the Windows executable or Linux application id other platforms report in its place.
pub fn is_browser(bundle_id: &str) -> bool {
    let lower = bundle_id.to_lowercase();
    if OTHER_PLATFORM_BROWSERS.contains(&lower.trim_end_matches(".desktop")) {
        return true;
    }
    let parts: Vec<&str> = lower.split('.').collect();
    (2..=parts.len())
        .rev()
        .any(|end| BROWSERS.contains(&parts[..end].join(".").as_str()))
}

// A private window says so in its title: "… - Google Chrome (Incognito)", "Private Browsing".
const PRIVATE_TITLES: &[&str] = &[
    "incognito",
    "inprivate",
    "private browsing",
    "private window",
    "시크릿",
    "シークレット",
    "无痕",
    "無痕",
];

const PASSWORD_MANAGERS: &[&str] = &[
    "com.1password.1password",
    "com.agilebits.onepassword7",
    "com.agilebits.onepassword-osx",
    "com.apple.passwords",
    "com.apple.keychainaccess",
    "com.bitwarden.desktop",
    "com.dashlane.dashlanephonefinal",
    "com.dashlane.dashlane",
    "com.lastpass.lastpass",
    "com.lastpass.lastpassmacdesktop",
    "org.keepassxc.keepassxc",
    "com.keepassium.keepassium",
    "in.sinew.enpass-desktop",
    "com.nordsec.nordpass",
    "me.proton.pass.electron",
    "com.roboform.roboform",
    "com.keeper.keeperpasswordmanager",
    "com.strongbox.mac",
];

const OTHER_PLATFORM_BROWSERS: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "vivaldi.exe",
    "arc.exe",
    "chromium.exe",
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "firefox",
    "brave-browser",
    "microsoft-edge",
    "opera",
    "vivaldi-stable",
];

const PRIVATE_BROWSERS: &[&str] = &["org.torproject.torbrowser", "net.mullvad.mullvadbrowser"];

const BROWSERS: &[&str] = &[
    "com.apple.safari",
    "com.apple.safaritechnologypreview",
    "com.google.chrome",
    "com.google.chrome.canary",
    "org.chromium.chromium",
    "com.microsoft.edgemac",
    "com.brave.browser",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "org.mozilla.nightly",
    "com.operasoftware.opera",
    "com.operasoftware.operaneon",
    "com.vivaldi.vivaldi",
    "at.studio.asidebrowser",
    "company.thebrowser.browser",
    "company.thebrowser.dia",
    "ai.perplexity.comet",
    "com.openai.atlas",
    "app.zen-browser.zen",
    "com.kagi.kagimacos",
    "com.duckduckgo.macos.browser",
    "io.gitlab.librewolf-community",
    "net.waterfox.waterfox",
    "org.ablaze.floorp",
    "org.palemoon.palemoon",
    "com.hiddenreflex.epic",
    "net.imput.helium",
    "org.chromium.thorium",
    "com.sigmaos.sigmaos.macos",
    "com.pushplaylabs.sidekick",
    "com.bookry.wavebox",
    "com.polypane.app",
    "com.ghostbrowser.ghost",
    "ru.yandex.desktop.yandex-browser",
    "com.naver.whale",
    "com.coccoc.coccoc",
    "com.maxthon.mac.maxthon",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn app(bundle_id: &str, title: &str) -> WindowFacts {
        WindowFacts {
            bundle_id: Some(bundle_id.into()),
            app_name: Some("Notes".into()),
            title: Some(title.into()),
            secure_field: Some(false),
            private_window: Some(false),
            browser_host: None,
        }
    }

    fn excluding(apps: &[&str], websites: &[&str]) -> ContextExclusions {
        ContextExclusions {
            apps: apps.iter().map(|a| (*a).into()).collect(),
            websites: websites.iter().map(|w| (*w).into()).collect(),
        }
    }

    fn denied(reason: DenyReason) -> ContextAccess {
        ContextAccess::Denied { reason }
    }

    #[test]
    fn reads_context_only_when_enabled_and_every_privacy_fact_is_known_and_clear() {
        let none = excluding(&[], &[]);
        let notes = app("com.apple.notes", "Launch plan");
        assert_eq!(
            decide_context(true, &notes, Some(&none)),
            ContextAccess::Allowed
        );
        assert_eq!(
            decide_context(false, &notes, Some(&none)),
            denied(DenyReason::Disabled)
        );
        assert_eq!(
            decide_context(true, &notes, None),
            denied(DenyReason::ExclusionsUnreadable)
        );
        let cases = [
            (
                WindowFacts {
                    bundle_id: None,
                    ..notes.clone()
                },
                DenyReason::UnknownApp,
            ),
            (
                app("com.1password.1password", "Vault"),
                DenyReason::PasswordManager,
            ),
            (
                app("org.torproject.torbrowser", "News"),
                DenyReason::PrivateBrowser,
            ),
            (
                WindowFacts {
                    secure_field: Some(true),
                    ..notes.clone()
                },
                DenyReason::SecureField,
            ),
            (
                WindowFacts {
                    secure_field: None,
                    ..notes.clone()
                },
                DenyReason::UnknownSecureField,
            ),
            (
                WindowFacts {
                    private_window: Some(true),
                    ..notes.clone()
                },
                DenyReason::PrivateWindow,
            ),
            (
                WindowFacts {
                    private_window: None,
                    ..notes.clone()
                },
                DenyReason::UnknownPrivateWindow,
            ),
            (
                app("com.google.Chrome", "Docs - Google Chrome (Incognito)"),
                DenyReason::PrivateWindow,
            ),
            (
                app("com.google.Chrome", "새 시크릿 탭"),
                DenyReason::PrivateWindow,
            ),
        ];
        for (facts, reason) in cases {
            assert_eq!(
                decide_context(true, &facts, Some(&none)),
                denied(reason),
                "{facts:?}"
            );
            assert!(!may_read_for_fitting(&facts, Some(&none)), "{facts:?}");
        }
    }

    #[test]
    fn reads_a_browser_only_on_a_host_read_from_the_browser_that_is_not_excluded() {
        let banks = excluding(&[], &["chase.com", "mail.google.com"]);
        let chrome = app("com.google.Chrome.beta", "Inbox - Gmail");
        // A title alone never establishes the page's host, so the window gives no context.
        assert_eq!(
            decide_context(true, &chrome, Some(&banks)),
            denied(DenyReason::UnverifiedBrowserHost)
        );
        // But the text around the caret may still be read to fit the dictation on this device.
        assert!(may_read_for_fitting(&chrome, Some(&banks)));
        let on = |host: &str| WindowFacts {
            browser_host: Some(host.into()),
            ..chrome.clone()
        };
        assert_eq!(
            decide_context(true, &on("https://mail.google.com/mail/u/0"), Some(&banks)),
            denied(DenyReason::ExcludedSite)
        );
        assert_eq!(
            decide_context(true, &on("secure.chase.com"), Some(&banks)),
            denied(DenyReason::ExcludedSite)
        );
        assert_eq!(
            decide_context(true, &on("docs.google.com"), Some(&banks)),
            ContextAccess::Allowed
        );
        let firefox = WindowFacts {
            bundle_id: Some("firefox.exe".into()),
            ..chrome.clone()
        };
        assert_eq!(
            decide_context(true, &firefox, Some(&banks)),
            denied(DenyReason::UnverifiedBrowserHost)
        );
    }

    #[test]
    fn denies_excluded_apps_and_apps_whose_title_names_an_excluded_site() {
        let list = excluding(&["Slack", "com.tinyspeck.slackmacgap"], &["chase.com"]);
        let slack = WindowFacts {
            app_name: Some("Slack".into()),
            ..app("com.tinyspeck.slackmacgap", "general")
        };
        assert_eq!(
            decide_context(true, &slack, Some(&list)),
            denied(DenyReason::ExcludedApp)
        );
        assert!(!may_read_for_fitting(&slack, Some(&list)));
        let mail = app("com.apple.mail", "Chase - Accounts");
        assert_eq!(
            decide_context(true, &mail, Some(&list)),
            denied(DenyReason::ExcludedSite)
        );
        assert_eq!(
            decide_context(true, &app("com.apple.mail", "Purchase order"), Some(&list)),
            ContextAccess::Allowed
        );
    }

    #[test]
    fn collects_a_bounded_snapshot_only_from_an_allowed_decision() {
        let facts = WindowFacts {
            app_name: Some("Linear".into()),
            ..app("com.linear", "ENG-42 PostHog funnel")
        };
        let window = format!(
            "{}\nWoohyeok: can GPT5 read the PostHog export?",
            "older line\n".repeat(400)
        );
        let before = format!("{}Ask Woohyeok", "x".repeat(2_000));
        let collect = |access: &ContextAccess| {
            ContextSnapshot::collect(
                "op-1",
                5,
                access,
                &facts,
                Some(&before),
                Some(&window),
                None,
            )
        };
        assert_eq!(collect(&denied(DenyReason::Disabled)), None);
        let snapshot = collect(&ContextAccess::Allowed).unwrap();
        assert_eq!(snapshot.operation_id, "op-1");
        assert_eq!(
            char_len(snapshot.before.as_deref().unwrap()),
            MAX_BEFORE_LENGTH
        );
        let screen = snapshot.screen.unwrap();
        assert!(screen.starts_with("Linear — ENG-42 PostHog funnel\n\n"));
        assert!(screen.ends_with("PostHog export?"));
        assert!(char_len(&screen) <= MAX_HEADING_LENGTH + 2 + MAX_SCREEN_LENGTH);
        // Acronyms and words a word list would be needed to judge are left to cleanup.
        assert_eq!(snapshot.terms, ["PostHog", "GPT5"]);
        assert_eq!(
            ContextSnapshot::collect(
                "op-1",
                5,
                &ContextAccess::Allowed,
                &facts,
                Some("  "),
                None,
                None
            )
            .map(|s| s.terms),
            Some(vec!["PostHog".to_owned()])
        );
    }
}
