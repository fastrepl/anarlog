use std::collections::HashSet;

#[cfg(target_os = "macos")]
use cidre::ns;

use super::{
    AxNode, MeetingApp, MeetingPlatform, MeetingSurface, is_platform_meeting_control, node_labels,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MeetingAppBundleKind {
    Native,
    Browser,
}

pub(super) struct MeetingAppBundle {
    pub(super) id: &'static str,
    pub(super) kind: MeetingAppBundleKind,
}

impl MeetingAppBundle {
    const fn native(id: &'static str) -> Self {
        Self {
            id,
            kind: MeetingAppBundleKind::Native,
        }
    }

    const fn browser(id: &'static str) -> Self {
        Self {
            id,
            kind: MeetingAppBundleKind::Browser,
        }
    }
}

pub(super) const MEETING_APP_BUNDLES: &[MeetingAppBundle] = &[
    MeetingAppBundle::native("us.zoom.xos"),
    MeetingAppBundle::native("com.microsoft.teams2"),
    MeetingAppBundle::native("com.microsoft.teams"),
    MeetingAppBundle::native("com.tinyspeck.slackmacgap"),
    MeetingAppBundle::native("com.slack.Slack"),
    MeetingAppBundle::native("com.hnc.Discord"),
    MeetingAppBundle::native("com.discordapp.Discord"),
    MeetingAppBundle::native("Cisco-Systems.Spark"),
    MeetingAppBundle::native("com.cisco.webex"),
    MeetingAppBundle::native("com.cisco.webexmeetingsapp"),
    MeetingAppBundle::browser("com.google.Chrome"),
    MeetingAppBundle::browser("com.google.Chrome.canary"),
    MeetingAppBundle::browser("com.microsoft.edgemac"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Beta"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Canary"),
    MeetingAppBundle::browser("com.microsoft.edgemac.Dev"),
    MeetingAppBundle::browser("org.mozilla.firefox"),
    MeetingAppBundle::browser("org.mozilla.firefoxdeveloperedition"),
    MeetingAppBundle::browser("org.mozilla.nightly"),
    MeetingAppBundle::browser("com.apple.Safari"),
    MeetingAppBundle::browser("com.apple.SafariTechnologyPreview"),
    MeetingAppBundle::browser("com.brave.Browser"),
    MeetingAppBundle::browser("com.brave.Browser.beta"),
    MeetingAppBundle::browser("com.brave.Browser.nightly"),
    MeetingAppBundle::browser("org.chromium.Chromium"),
    MeetingAppBundle::browser("com.vivaldi.Vivaldi"),
    MeetingAppBundle::browser("com.operasoftware.Opera"),
    MeetingAppBundle::browser("com.operasoftware.OperaDeveloper"),
    MeetingAppBundle::browser("com.operasoftware.OperaGX"),
    MeetingAppBundle::browser("com.operasoftware.OperaNext"),
    MeetingAppBundle::browser("company.thebrowser.Browser"),
    MeetingAppBundle::browser("ai.perplexity.comet"),
    MeetingAppBundle::browser("at.studio.AsideBrowser"),
    MeetingAppBundle::browser("company.thebrowser.dia"),
    MeetingAppBundle::browser("com.sigmaos.sigmaos.macos"),
    MeetingAppBundle::browser("net.imput.helium"),
    MeetingAppBundle::browser("com.nousresearch.hermes"),
];

pub(super) fn meeting_app_alias_key(id: &str) -> String {
    let basename = id
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(id)
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".desktop");
    basename.to_ascii_lowercase()
}

pub(super) fn canonicalize_meeting_app_id(id: &str) -> String {
    let trimmed = id.trim();
    if MEETING_APP_BUNDLES
        .iter()
        .any(|bundle| bundle.id == trimmed)
    {
        return trimmed.to_string();
    }

    match meeting_app_alias_key(trimmed).as_str() {
        "zoom" | "zoomlinux" => "us.zoom.xos".to_string(),
        "teams" | "teams-for-linux" | "ms-teams" | "msteams" => "com.microsoft.teams2".to_string(),
        "slack" | "slack-desktop" => "com.slack.Slack".to_string(),
        "discord" => "com.discordapp.Discord".to_string(),
        "webex" | "ciscocollabhost" => "com.cisco.webex".to_string(),
        "google-chrome" | "google-chrome-stable" | "google-chrome-beta" | "chrome" => {
            "com.google.Chrome".to_string()
        }
        "chromium" | "chromium-browser" => "org.chromium.Chromium".to_string(),
        "firefox" => "org.mozilla.firefox".to_string(),
        "microsoft-edge" | "microsoft-edge-stable" | "msedge" => {
            "com.microsoft.edgemac".to_string()
        }
        "brave" | "brave-browser" => "com.brave.Browser".to_string(),
        "vivaldi" | "vivaldi-stable" => "com.vivaldi.Vivaldi".to_string(),
        "opera" => "com.operasoftware.Opera".to_string(),
        _ => trimmed.to_string(),
    }
}

pub(super) fn meeting_app_family(id: &str) -> Option<&'static str> {
    match canonicalize_meeting_app_id(id).as_str() {
        "us.zoom.xos" => Some("zoom"),
        "com.microsoft.teams2" | "com.microsoft.teams" => Some("teams"),
        "com.tinyspeck.slackmacgap" | "com.slack.Slack" => Some("slack"),
        "com.hnc.Discord" | "com.discordapp.Discord" => Some("discord"),
        "Cisco-Systems.Spark" | "com.cisco.webex" | "com.cisco.webexmeetingsapp" => Some("webex"),
        other if is_browser_bundle(other) => MEETING_APP_BUNDLES
            .iter()
            .find(|bundle| bundle.id == other)
            .map(|bundle| bundle.id),
        _ => None,
    }
}

pub(super) fn unique_recognized_meeting_bundle(
    mic_active_bundle_ids: &[String],
) -> Result<&str, String> {
    let mut recognized = Vec::new();
    let mut families = HashSet::new();
    for bundle_id in mic_active_bundle_ids {
        let Some(family) = meeting_app_family(bundle_id) else {
            continue;
        };
        if families.insert(family) {
            recognized.push(bundle_id.as_str());
        }
    }

    if recognized.len() != 1 {
        return Err(format!(
            "refusing to send because the mic-active apps contain {} recognized meeting app bundles; expected exactly one",
            recognized.len()
        ));
    }

    Ok(recognized[0])
}

#[cfg(target_os = "macos")]
pub(super) fn running_apps_for_bundle(bundle_id: &str) -> Vec<(MeetingApp, i32)> {
    let mut apps = Vec::new();
    let bundle = ns::String::with_str(bundle_id);
    let running = ns::RunningApp::with_bundle_id(&bundle);

    for app in running.iter() {
        let pid = app.pid();
        let name = app
            .localized_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| bundle_id.to_string());
        let id = app
            .bundle_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| bundle_id.to_string());

        apps.push((MeetingApp { id, name }, pid));
    }

    apps
}

#[cfg(target_os = "macos")]
pub(super) fn running_meeting_apps() -> Vec<(MeetingApp, i32)> {
    let mut seen = HashSet::new();

    MEETING_APP_BUNDLES
        .iter()
        .flat_map(|bundle| running_apps_for_bundle(bundle.id))
        .filter(|(_, pid)| seen.insert(*pid))
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn running_apps_for_bundle(bundle_id: &str) -> Vec<(MeetingApp, i32)> {
    let Some(family) = meeting_app_family(bundle_id) else {
        return Vec::new();
    };

    let mut system = sysinfo::System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut seen = HashSet::new();
    system
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let process_name = process.name().to_string_lossy().into_owned();
            let exe_name = process
                .exe()
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let command = process
                .cmd()
                .first()
                .map(|part| part.to_string_lossy().into_owned())
                .unwrap_or_default();
            let matches = [process_name.as_str(), exe_name.as_str(), command.as_str()]
                .into_iter()
                .any(|candidate| meeting_app_family(candidate) == Some(family));
            if !matches {
                return None;
            }

            let pid = i32::try_from(pid.as_u32()).ok()?;
            if !seen.insert(pid) {
                return None;
            }

            Some((
                MeetingApp {
                    id: bundle_id.to_string(),
                    name: process_name,
                },
                pid,
            ))
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
pub(super) fn running_meeting_apps() -> Vec<(MeetingApp, i32)> {
    let mut seen = HashSet::new();

    MEETING_APP_BUNDLES
        .iter()
        .flat_map(|bundle| running_apps_for_bundle(bundle.id))
        .filter(|(_, pid)| seen.insert(*pid))
        .collect()
}

pub(super) fn select_active_bundle_ids<'a>(
    supported_bundle_ids: impl IntoIterator<Item = &'a str>,
    active_bundle_ids: &[String],
) -> Vec<&'a str> {
    let active_families = active_bundle_ids
        .iter()
        .filter_map(|bundle_id| meeting_app_family(bundle_id))
        .collect::<HashSet<_>>();
    let mut seen_families = HashSet::new();

    supported_bundle_ids
        .into_iter()
        .filter(|bundle_id| {
            meeting_app_family(bundle_id).is_some_and(|family| {
                active_families.contains(family) && seen_families.insert(family)
            })
        })
        .collect()
}

pub(super) fn classify_bundle(bundle_id: &str) -> MeetingPlatform {
    match canonicalize_meeting_app_id(bundle_id).as_str() {
        "us.zoom.xos" => MeetingPlatform::Zoom,
        "com.microsoft.teams2" | "com.microsoft.teams" => MeetingPlatform::MicrosoftTeams,
        "com.tinyspeck.slackmacgap" | "com.slack.Slack" => MeetingPlatform::Slack,
        "com.hnc.Discord" | "com.discordapp.Discord" => MeetingPlatform::Discord,
        "Cisco-Systems.Spark" | "com.cisco.webex" | "com.cisco.webexmeetingsapp" => {
            MeetingPlatform::Webex
        }
        _ => MeetingPlatform::Unknown,
    }
}

pub(super) fn supports_meeting_chat_mutation(bundle_id: &str) -> bool {
    meeting_app_family(bundle_id).is_some()
}

pub(super) fn classify_browser_context(
    web_area_url: Option<&str>,
    window_title: Option<&str>,
    active_web_area: Option<&AxNode>,
    nodes: &[AxNode],
) -> MeetingPlatform {
    let Some(platform) = browser_platform_from_url(web_area_url) else {
        return MeetingPlatform::Unknown;
    };

    let mut title_platforms = window_title
        .into_iter()
        .chain(active_web_area.into_iter().flat_map(node_labels))
        .flat_map(browser_title_platform_signals)
        .collect::<Vec<_>>();
    title_platforms.dedup();
    if title_platforms.iter().any(|signal| signal != &platform) {
        return MeetingPlatform::Unknown;
    }
    let has_matching_title = title_platforms.contains(&platform);
    let has_matching_control = nodes
        .iter()
        .any(|node| is_platform_meeting_control(&platform, node));

    if has_matching_title || has_matching_control {
        platform
    } else {
        MeetingPlatform::Unknown
    }
}

pub(super) fn browser_platform_from_url(url: Option<&str>) -> Option<MeetingPlatform> {
    let url = url::Url::parse(url?).ok()?;
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();

    if host == "meet.google.com" {
        Some(MeetingPlatform::GoogleMeet)
    } else if matches!(host.as_str(), "teams.microsoft.com" | "teams.live.com") {
        Some(MeetingPlatform::MicrosoftTeams)
    } else if host == "zoom.us" || host.ends_with(".zoom.us") {
        Some(MeetingPlatform::Zoom)
    } else if host == "webex.com" || host.ends_with(".webex.com") {
        Some(MeetingPlatform::Webex)
    } else if matches!(host.as_str(), "slack.com" | "app.slack.com") {
        Some(MeetingPlatform::Slack)
    } else if matches!(
        host.as_str(),
        "discord.com" | "canary.discord.com" | "ptb.discord.com"
    ) {
        Some(MeetingPlatform::Discord)
    } else {
        None
    }
}

pub(super) fn browser_title_platform_signals(text: &str) -> Vec<MeetingPlatform> {
    let text = text.to_ascii_lowercase();
    let mut platforms = Vec::new();

    if text.contains("google meet") {
        platforms.push(MeetingPlatform::GoogleMeet);
    }
    if text.contains("microsoft teams") || text.contains("teams meeting") {
        platforms.push(MeetingPlatform::MicrosoftTeams);
    }
    if text.contains("zoom meeting") {
        platforms.push(MeetingPlatform::Zoom);
    }
    if text.contains("huddle") && text.contains("slack") {
        platforms.push(MeetingPlatform::Slack);
    }
    if text.contains("discord") && (text.contains("voice") || text.contains("call")) {
        platforms.push(MeetingPlatform::Discord);
    }
    if text.contains("webex meeting") || text.contains("cisco webex") {
        platforms.push(MeetingPlatform::Webex);
    }

    platforms
}

pub(super) fn classify_platform(
    bundle_id: &str,
    _window_title: Option<&str>,
    _nodes: &[AxNode],
    bundle_platform: MeetingPlatform,
) -> MeetingPlatform {
    if is_browser_bundle(bundle_id) {
        MeetingPlatform::Unknown
    } else {
        bundle_platform
    }
}

pub(super) fn classify_surface(bundle_id: &str, platform: &MeetingPlatform) -> MeetingSurface {
    if is_browser_bundle(bundle_id) {
        MeetingSurface::Web
    } else if *platform == MeetingPlatform::Unknown {
        MeetingSurface::Unknown
    } else {
        MeetingSurface::Native
    }
}

fn meeting_app_bundle(bundle_id: &str) -> Option<&MeetingAppBundle> {
    let canonical = canonicalize_meeting_app_id(bundle_id);
    MEETING_APP_BUNDLES
        .iter()
        .find(|bundle| bundle.id == canonical || bundle.id == bundle_id)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn is_meeting_app_bundle(bundle_id: &str) -> bool {
    meeting_app_bundle(bundle_id).is_some()
}

pub(super) fn is_browser_bundle(bundle_id: &str) -> bool {
    meeting_app_bundle(bundle_id).is_some_and(|bundle| bundle.kind == MeetingAppBundleKind::Browser)
}
