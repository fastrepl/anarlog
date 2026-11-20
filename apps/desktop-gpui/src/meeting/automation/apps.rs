//! Shipping's `stt/meeting-apps.ts`: how a microphone app is named and drawn in the
//! "meeting detected" notification, including the meeting platform a browser call is on.

use std::{collections::HashSet, path::Path, sync::OnceLock};

use anlg_notification_interface::NotificationIcon;
use regex::Regex;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MicApp {
    pub id: String,
    pub name: String,
}

impl MicApp {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// The calendar event closest to now, for the notification title and the browser's platform.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NearbyEvent {
    pub id: String,
    pub title: String,
    pub meeting_link: String,
    pub location: String,
    pub description: String,
    pub participant_names: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub name: &'static str,
    icon: &'static str,
}

const fn platform(name: &'static str, icon: &'static str) -> Platform {
    Platform { name, icon }
}

const ZOOM: Platform = platform("Zoom", "zoom.svg");
const GOOGLE_MEET: Platform = platform("Google Meet", "google-meet.svg");
const WEBEX: Platform = platform("Webex", "webex.svg");
const TEAMS: Platform = platform("Microsoft Teams", "microsoft-teams.svg");
const CAL_COM: Platform = platform("Cal.com", "cal-com.png");
const CAL_VIDEO: Platform = platform("Cal Video", "cal-video.png");
const DAILY: Platform = platform("Daily", "daily.png");
const WHEREBY: Platform = platform("Whereby", "whereby.png");
const JITSI: Platform = platform("Jitsi", "jitsi.png");
const GOTOMEETING: Platform = platform("GoTo Meeting", "gotomeeting.png");
const SLACK: Platform = platform("Slack", "slack.svg");
const DISCORD: Platform = platform("Discord", "discord.png");
const WHATSAPP: Platform = platform("WhatsApp", "whatsapp.png");
const KAKAOTALK: Platform = platform("KakaoTalk", "kakaotalk.png");
const TELEGRAM: Platform = platform("Telegram", "telegram.png");
const SIGNAL: Platform = platform("Signal", "signal.png");
const LINE: Platform = platform("LINE", "line.png");
const MESSENGER: Platform = platform("Messenger", "messenger.png");
const X_CALLS: Platform = platform("X Calls", "x-calls.svg");

const BROWSER_APP_IDS: &[&str] = &[
    "at.studio.AsideBrowser",
    "app.zen-browser.zen",
    "ai.perplexity.comet",
    "com.apple.Safari",
    "com.apple.SafariTechnologyPreview",
    "com.browseros.BrowserOS",
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.brave.Browser.nightly",
    "com.brave.Browser.origin",
    "com.duckduckgo.macos.browser",
    "chrome",
    "comet",
    "chromium",
    "chromium-browser",
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "firefox",
    "google-chrome",
    "google-chrome-beta",
    "google-chrome-stable",
    "microsoft-edge",
    "msedge",
    "helium",
    "com.kagi.kagimacOS",
    "com.kagi.kagimacOS.RC",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.edgemac.Canary",
    "com.microsoft.edgemac.Dev",
    "com.operasoftware.Opera",
    "com.operasoftware.OperaDeveloper",
    "com.operasoftware.OperaGX",
    "com.operasoftware.OperaNext",
    "opera",
    "com.nousresearch.hermes",
    "com.sigmaos.sigmaos.macos",
    "com.vivaldi.Vivaldi",
    "vivaldi",
    "company.thebrowser.Browser",
    "company.thebrowser.dia",
    "net.imput.helium",
    "net.mullvad.mullvadbrowser",
    "net.waterfox.waterfox",
    "org.chromium.Chromium",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "org.mozilla.librewolf",
    "org.mozilla.nightly",
    "org.torproject.torbrowser",
    "zen",
];

enum OverrideIcon {
    Resource(&'static str),
    BundleId(&'static str),
}

struct Override {
    ids: &'static [&'static str],
    names: &'static [&'static str],
    name: &'static str,
    platform: Option<Platform>,
    icon: OverrideIcon,
}

const OVERRIDES: &[Override] = &[
    Override {
        ids: &[
            "/usr/libexec/avconferenced",
            "com.apple.avconferenced",
            "com.apple.TelephonyUtilities",
            "com.apple.TelephonyUtilities.callservicesd",
        ],
        names: &["av capture", "avcapture", "avconferenced", "iphone call"],
        name: "iPhone Call",
        platform: None,
        icon: OverrideIcon::Resource("phone.png"),
    },
    Override {
        ids: &["com.apple.FaceTime"],
        names: &["facetime"],
        name: "FaceTime",
        platform: None,
        icon: OverrideIcon::BundleId("com.apple.FaceTime"),
    },
    Override {
        ids: &["us.zoom.xos", "zoom", "Zoom.exe"],
        names: &["zoom", "zoom helper", "zoom workplace"],
        name: "Zoom",
        platform: Some(ZOOM),
        icon: OverrideIcon::Resource("zoom.svg"),
    },
    Override {
        ids: &[
            "com.microsoft.teams",
            "com.microsoft.teams2",
            "ms-teams",
            "teams",
            "teams-for-linux",
        ],
        names: &[
            "microsoft teams",
            "microsoft teams helper",
            "teams",
            "teams helper",
        ],
        name: "Microsoft Teams",
        platform: Some(TEAMS),
        icon: OverrideIcon::Resource("microsoft-teams.svg"),
    },
    Override {
        ids: &[
            "Cisco-Systems.Spark",
            "com.cisco.webex",
            "com.cisco.webexmeetingsapp",
        ],
        names: &["cisco webex", "webex", "webex helper", "webex meetings"],
        name: "Webex",
        platform: Some(WEBEX),
        icon: OverrideIcon::Resource("webex.svg"),
    },
    Override {
        ids: &[
            "com.slack.Slack",
            "com.tinyspeck.slackmacgap",
            "slack",
            "slack-desktop",
            "Slack.exe",
        ],
        names: &["slack", "slack helper"],
        name: "Slack",
        platform: Some(SLACK),
        icon: OverrideIcon::Resource("slack.svg"),
    },
    Override {
        ids: &["com.kakao.KakaoTalkMac"],
        names: &["kakaotalk", "kakaotalk helper"],
        name: "KakaoTalk",
        platform: Some(KAKAOTALK),
        icon: OverrideIcon::Resource("kakaotalk.png"),
    },
    Override {
        ids: &["net.whatsapp.WhatsApp"],
        names: &["whatsapp", "whatsapp helper"],
        name: "WhatsApp",
        platform: Some(WHATSAPP),
        icon: OverrideIcon::Resource("whatsapp.png"),
    },
    Override {
        ids: &["com.hnc.Discord", "com.discordapp.Discord"],
        names: &["discord", "discord helper"],
        name: "Discord",
        platform: Some(DISCORD),
        icon: OverrideIcon::Resource("discord.png"),
    },
    Override {
        ids: &["org.whispersystems.signal-desktop"],
        names: &["signal", "signal helper"],
        name: "Signal",
        platform: Some(SIGNAL),
        icon: OverrideIcon::Resource("signal.png"),
    },
    Override {
        ids: &["ru.keepcoder.Telegram", "ru.keepcoder.TelegramLite"],
        names: &["telegram", "telegram helper", "telegram lite"],
        name: "Telegram",
        platform: Some(TELEGRAM),
        icon: OverrideIcon::Resource("telegram.png"),
    },
    Override {
        ids: &["jp.naver.line.mac"],
        names: &["line", "line helper"],
        name: "LINE",
        platform: Some(LINE),
        icon: OverrideIcon::Resource("line.png"),
    },
    Override {
        ids: &["com.facebook.archon"],
        names: &["messenger", "messenger helper"],
        name: "Messenger",
        platform: Some(MESSENGER),
        icon: OverrideIcon::Resource("messenger.png"),
    },
];

fn find_override(app: &MicApp) -> Option<&'static Override> {
    let name = app.name.trim().to_lowercase();
    OVERRIDES
        .iter()
        .find(|entry| entry.names.contains(&name.as_str()))
        .or_else(|| {
            OVERRIDES
                .iter()
                .find(|entry| entry.ids.contains(&app.id.as_str()))
        })
}

pub fn display_name(app: &MicApp) -> String {
    find_override(app).map_or_else(|| app.name.clone(), |entry| entry.name.into())
}

fn is_browser(app: &MicApp) -> bool {
    BROWSER_APP_IDS.contains(&app.id.as_str())
}

/// Shipping's notification icons: the shipping sources in debug builds, else the package's.
pub fn notification_icons() -> Option<std::path::PathBuf> {
    crate::platform::package::Resources::current().map(|resources| resources.notification_icons())
}

fn resource_icon(resources: Option<&Path>, file: &str) -> Option<NotificationIcon> {
    let path = resources?.join(file);
    path.is_file().then(|| NotificationIcon::Path {
        path: path.to_string_lossy().into_owned(),
    })
}

fn platform_icon(resources: Option<&Path>, platform: Platform) -> NotificationIcon {
    resource_icon(resources, platform.icon).unwrap_or(NotificationIcon::SystemSymbol {
        name: "video.fill".into(),
    })
}

fn app_icon(resources: Option<&Path>, app: &MicApp) -> Option<NotificationIcon> {
    match find_override(app).map(|entry| &entry.icon) {
        Some(OverrideIcon::Resource(file)) => {
            resource_icon(resources, file).or_else(|| NotificationIcon::from_app_id(&app.id))
        }
        Some(OverrideIcon::BundleId(bundle_id)) => Some(NotificationIcon::BundleId {
            bundle_id: (*bundle_id).into(),
        }),
        None => NotificationIcon::from_app_id(&app.id),
    }
}

fn url_platform(value: &str) -> Option<Platform> {
    let url = url::Url::parse(value).ok()?;
    let host = url.host_str()?.to_lowercase();
    let path = url.path().to_lowercase();
    let host = host.as_str();
    let under = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    Some(match host {
        "call.x.com" => X_CALLS,
        "x.com" | "www.x.com" if path.starts_with("/i/call/") || path.starts_with("/call/") => {
            X_CALLS
        }
        _ if under("zoom.us") => ZOOM,
        "meet.google.com" => GOOGLE_MEET,
        _ if under("webex.com") => WEBEX,
        "teams.microsoft.com" | "teams.live.com" => TEAMS,
        "app.cal.video" | "cal.video" => CAL_VIDEO,
        _ if host.ends_with(".cal.video") => CAL_VIDEO,
        "cal.com" | "app.cal.com" if path.starts_with("/video/") => CAL_VIDEO,
        "cal.com" | "app.cal.com" => CAL_COM,
        _ if under("daily.co") => DAILY,
        _ if under("whereby.com") || under("appear.in") => WHEREBY,
        "meet.jit.si" => JITSI,
        _ if host.ends_with(".jitsi.org") => JITSI,
        _ if under("gotomeeting.com") || under("goto.com") => GOTOMEETING,
        _ if under("slack.com") => SLACK,
        "discord.gg" => DISCORD,
        _ if under("discord.com") => DISCORD,
        "web.whatsapp.com" | "whatsapp.com" => WHATSAPP,
        "talk.kakao.com" => KAKAOTALK,
        _ if host.ends_with(".kakao.com") => KAKAOTALK,
        "web.telegram.org" | "t.me" | "telegram.me" => TELEGRAM,
        "signal.me" => SIGNAL,
        _ if under("line.me") => LINE,
        "messenger.com" | "www.messenger.com" => MESSENGER,
        _ => return None,
    })
}

fn text_platform(value: &str) -> Option<Platform> {
    static URLS: OnceLock<Regex> = OnceLock::new();
    static PATTERNS: OnceLock<Vec<(Regex, Platform)>> = OnceLock::new();
    let urls = URLS.get_or_init(|| Regex::new(r#"https?://[^\s<>"')]+"#).expect("url pattern"));
    if let Some(platform) = urls
        .find_iter(value)
        .find_map(|url| url_platform(url.as_str()))
    {
        return Some(platform);
    }
    let patterns =
        PATTERNS.get_or_init(|| {
            [
            (r"\bgoogle meet\b", GOOGLE_MEET),
            (r"\bmicrosoft teams\b", TEAMS),
            (r"\bzoom meeting\b", ZOOM),
            (r"\bwebex\b", WEBEX),
            (
                r"\bcal video\b|(^|[^a-z0-9])cal\.video([^a-z0-9]|$)|(^|[^a-z0-9])cal\.com/video/",
                CAL_VIDEO,
            ),
            (r"(^|[^a-z0-9])cal\.com([^a-z0-9]|$)", CAL_COM),
            (r"(^|[^a-z0-9])daily\.co([^a-z0-9]|$)|\bdaily prebuilt\b", DAILY),
            (r"\bwhereby\b", WHEREBY),
            (r"\bjitsi\b", JITSI),
            (r"\bgoto meeting\b|\bgotomeeting\b", GOTOMEETING),
            (r"\bslack (huddle|call)\b", SLACK),
            (r"\bdiscord (call|meeting|voice)\b", DISCORD),
            (r"\bwhatsapp (call|meeting)\b", WHATSAPP),
            (r"\b(kakaotalk|kakao talk) (call|meeting)\b", KAKAOTALK),
            (r"\btelegram (call|meeting)\b", TELEGRAM),
            (r"\bsignal (call|meeting)\b", SIGNAL),
            (r"\bline meeting\b|^line$", LINE),
            (r"(^|[^a-z0-9])call\.x\.com/|\bx calls\b", X_CALLS),
            (r"\bmessenger (call|meeting|room)\b", MESSENGER),
        ]
        .into_iter()
        .map(|(pattern, platform)| (Regex::new(pattern).expect("platform pattern"), platform))
        .collect()
        });
    let normalized = value.to_lowercase();
    patterns
        .iter()
        .find(|(pattern, _)| pattern.is_match(&normalized))
        .map(|(_, platform)| *platform)
}

/// The meeting platform a browser call is on, read from the nearby event, unless a native
/// meeting app is also using the microphone.
pub fn browser_platform(apps: &[MicApp], event: Option<&NearbyEvent>) -> Option<Platform> {
    let event = event?;
    if !apps.iter().any(is_browser)
        || apps.iter().any(|app| {
            !is_browser(app) && find_override(app).is_some_and(|entry| entry.platform.is_some())
        })
    {
        return None;
    }
    [
        &event.meeting_link,
        &event.location,
        &event.description,
        &event.title,
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .find_map(|value| {
        if value.starts_with("http") {
            url_platform(value)
        } else {
            text_platform(value)
        }
    })
}

pub fn display_app(app: &MicApp, platform: Option<Platform>) -> MicApp {
    match platform {
        Some(platform) if is_browser(app) => MicApp::new(app.id.clone(), platform.name),
        _ => app.clone(),
    }
}

fn display_icon(
    resources: Option<&Path>,
    app: &MicApp,
    platform: Option<Platform>,
) -> Option<NotificationIcon> {
    match platform {
        Some(platform) if is_browser(app) => Some(platform_icon(resources, platform)),
        _ => app_icon(resources, app),
    }
}

/// The notification's icon: the first detected app that has one.
pub fn detected_icon(
    resources: Option<&Path>,
    apps: &[MicApp],
    platform: Option<Platform>,
) -> Option<NotificationIcon> {
    apps.iter()
        .find_map(|app| display_icon(resources, app, platform))
}

pub fn footer_icon(
    resources: Option<&Path>,
    apps: &[MicApp],
    platform: Option<Platform>,
) -> Option<NotificationIcon> {
    apps.first()
        .and_then(|app| display_icon(resources, &display_app(app, platform), platform))
}

/// Apps the footer can offer to ignore: identified, without duplicates.
pub fn ignorable(apps: &[MicApp]) -> Vec<MicApp> {
    let mut seen = HashSet::new();
    apps.iter()
        .filter(|app| !app.id.is_empty() && !app.id.starts_with("pid:") && seen.insert(&app.id))
        .cloned()
        .collect()
}

pub fn ignore_footer_text(apps: &[MicApp]) -> String {
    let first = apps
        .first()
        .map(|app| display_name(app).trim().to_owned())
        .unwrap_or_default();
    if apps.len() == 1 {
        return if first.is_empty() {
            "Ignore this app?".into()
        } else {
            format!("Ignore {first}?")
        };
    }
    if first.is_empty() {
        return "Ignore these apps?".into();
    }
    let second = apps
        .get(1)
        .map(|app| display_name(app).trim().to_owned())
        .unwrap_or_default();
    if apps.len() == 2 && !second.is_empty() {
        return format!("Ignore {first} and {second}?");
    }
    let others = apps.len() - 1;
    format!(
        "Ignore {first} and {others} other app{}?",
        if others == 1 { "" } else { "s" }
    )
}

/// Shipping's `getMicDetectedNotificationTitle`.
pub fn detected_title(event: Option<&NearbyEvent>) -> String {
    let Some(event) = event else {
        return "Are you in a meeting?".into();
    };
    match event.participant_names.as_slice() {
        [one] => format!("Are you talking to {one} right now?"),
        [one, two] => format!("Are you talking to {one} and {two} right now?"),
        _ => format!("Are you in {} right now?", event.title),
    }
}

/// One entry of a session's `source_apps_json` (shipping's `SessionSourceApp`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceApp {
    pub app: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
}

const SOURCE_ALIASES: &[(&str, &str)] = &[
    ("us.zoom.xos", "zoom"),
    ("com.microsoft.teams", "teams"),
    ("com.microsoft.teams2", "teams"),
    ("ms-teams", "teams"),
    ("msteams", "teams"),
    ("teams-for-linux", "teams"),
    ("com.slack.slack", "slack"),
    ("com.tinyspeck.slackmacgap", "slack"),
    ("slack-desktop", "slack"),
    ("com.hnc.discord", "discord"),
    ("com.discordapp.discord", "discord"),
    ("cisco-systems.spark", "webex"),
    ("com.cisco.webex", "webex"),
    ("com.cisco.webexmeetingsapp", "webex"),
    ("ciscocollabhost", "webex"),
    ("com.google.chrome", "chrome"),
    ("google-chrome", "chrome"),
    ("google-chrome-beta", "chrome"),
    ("google-chrome-stable", "chrome"),
    ("com.microsoft.edgemac", "edge"),
    ("microsoft-edge", "edge"),
    ("microsoft-edge-stable", "edge"),
    ("msedge", "edge"),
    ("org.mozilla.firefox", "firefox"),
    ("firefox-bin", "firefox"),
    ("org.chromium.chromium", "chromium"),
    ("chromium-browser", "chromium"),
    ("com.apple.safari", "safari"),
    ("com.brave.browser", "brave"),
    ("brave-browser", "brave"),
    ("com.operasoftware.opera", "opera"),
    ("com.vivaldi.vivaldi", "vivaldi"),
    ("at.studio.asidebrowser", "aside"),
    ("com.browseros.browseros", "browseros"),
    ("ai.perplexity.comet", "comet"),
    ("company.thebrowser.dia", "dia"),
    ("net.imput.helium", "helium"),
    ("com.nousresearch.hermes", "hermes"),
    ("app.zen-browser.zen", "zen"),
];

fn source_identity(app: &str) -> String {
    let last = app.rsplit(['/', '\\']).next().unwrap_or_default();
    let normalized = last.trim().to_lowercase();
    let normalized = normalized.strip_suffix(".exe").unwrap_or(&normalized);
    SOURCE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == normalized)
        .map_or_else(
            || normalized.to_owned(),
            |(_, identity)| (*identity).to_owned(),
        )
}

fn present(value: &Option<String>) -> Option<String> {
    value.clone().filter(|value| !value.trim().is_empty())
}

fn parse_sources(json: &str) -> Vec<SourceApp> {
    serde_json::from_str::<Vec<serde_json::Value>>(json)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<SourceApp>(entry).ok())
        .filter(|entry| !entry.app.trim().is_empty())
        .map(|entry| SourceApp {
            name: present(&entry.name),
            platform: present(&entry.platform),
            app: entry.app,
        })
        .collect()
}

/// Shipping's `mergeSessionSourceApps`: one entry per app identity, newer names and
/// platforms winning over stored ones.
pub fn merge_sources(current: Vec<SourceApp>, updates: &[SourceApp]) -> Vec<SourceApp> {
    let mut merged = current;
    for update in updates
        .iter()
        .filter(|update| !update.app.trim().is_empty())
    {
        let identity = source_identity(&update.app);
        match merged
            .iter_mut()
            .find(|entry| source_identity(&entry.app) == identity)
        {
            Some(existing) => {
                existing.name = present(&update.name).or(existing.name.take());
                existing.platform = present(&update.platform).or(existing.platform.take());
            }
            None => merged.push(SourceApp {
                app: update.app.clone(),
                name: present(&update.name),
                platform: present(&update.platform),
            }),
        }
    }
    merged
}

/// Shipping's `recordDetectedMeetingApps`: remembers which apps a captured call ran in.
pub fn record_source_apps(
    runtime: &desktop_runtime::RuntimeHandle,
    session: desktop_runtime::SessionId,
    apps: Vec<MicApp>,
) -> desktop_runtime::Result<desktop_runtime::Reply<()>> {
    let updates: Vec<SourceApp> = apps
        .iter()
        .map(|app| SourceApp {
            app: app.id.clone(),
            name: Some(app.name.clone()),
            platform: find_override(app)
                .and_then(|entry| entry.platform)
                .map(|platform| platform.name.to_owned()),
        })
        .collect();
    runtime.submit(move |services| async move {
        let failure = |error: String| desktop_runtime::ServiceError::Failed(error.into());
        let rows = services
            .executor
            .execute(
                "SELECT source_apps_json FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1"
                    .into(),
                vec![serde_json::json!(session.0)],
            )
            .await
            .map_err(|error| failure(error.to_string()))?;
        let Some(existing) = rows
            .first()
            .and_then(|row| row["source_apps_json"].as_str().map(str::to_owned))
        else {
            return Ok(());
        };
        let next = serde_json::to_string(&merge_sources(parse_sources(&existing), &updates))
            .map_err(|error| failure(error.to_string()))?;
        if next != existing {
            services
                .executor
                .execute(
                    "UPDATE sessions SET source_apps_json = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?2 AND deleted_at IS NULL AND source_apps_json = ?3"
                        .into(),
                    vec![serde_json::json!(next), serde_json::json!(session.0), serde_json::json!(existing)],
                )
                .await
                .map_err(|error| failure(error.to_string()))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(link: &str, description: &str) -> NearbyEvent {
        NearbyEvent {
            id: "event-1".into(),
            title: "Weekly sync".into(),
            meeting_link: link.into(),
            description: description.into(),
            ..NearbyEvent::default()
        }
    }

    #[test]
    fn browser_calls_take_the_nearby_events_platform_unless_a_meeting_app_is_also_live() {
        let chrome = MicApp::new("com.google.Chrome", "Google Chrome");
        let zoom = MicApp::new("us.zoom.xos", "zoom.us");
        let meet = event("https://meet.google.com/abc-defg-hij", "");
        assert_eq!(
            browser_platform(std::slice::from_ref(&chrome), Some(&meet)),
            Some(GOOGLE_MEET)
        );
        assert_eq!(display_app(&chrome, Some(GOOGLE_MEET)).name, "Google Meet");
        assert_eq!(browser_platform(&[chrome.clone(), zoom], Some(&meet)), None);
        assert_eq!(
            browser_platform(
                std::slice::from_ref(&chrome),
                Some(&event("", "Join the Slack huddle here"))
            ),
            Some(SLACK)
        );
        assert_eq!(
            browser_platform(
                std::slice::from_ref(&chrome),
                Some(&event("https://app.cal.com/video/xyz", ""))
            ),
            Some(CAL_VIDEO)
        );
        assert_eq!(browser_platform(&[chrome], None), None);
    }

    #[test]
    fn detected_apps_merge_into_a_sessions_sources_by_identity() {
        let stored = vec![SourceApp {
            app: "com.microsoft.teams2".into(),
            name: Some("Teams".into()),
            platform: None,
        }];
        let merged = merge_sources(
            stored,
            &[
                SourceApp {
                    app: "ms-teams".into(),
                    name: None,
                    platform: Some("Microsoft Teams".into()),
                },
                SourceApp {
                    app: "us.zoom.xos".into(),
                    name: Some("zoom.us".into()),
                    platform: Some("Zoom".into()),
                },
            ],
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].app, "com.microsoft.teams2");
        assert_eq!(merged[0].name.as_deref(), Some("Teams"));
        assert_eq!(merged[0].platform.as_deref(), Some("Microsoft Teams"));
        assert_eq!(
            serde_json::to_string(&merged[1]).unwrap(),
            r#"{"app":"us.zoom.xos","name":"zoom.us","platform":"Zoom"}"#
        );
    }
}
