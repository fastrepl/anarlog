/// One entry of the settings sidebar, mirroring `apps/desktop/src/sidebar/settings-nav-groups.ts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: &'static str,
    pub group: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    /// Labels of the controls on the page, so search finds a page by what it contains.
    pub terms: &'static [&'static str],
    pub requires_pro: bool,
}

const fn page_entry(
    id: &'static str,
    group: &'static str,
    label: &'static str,
    icon: &'static str,
    terms: &'static [&'static str],
    requires_pro: bool,
) -> Page {
    Page {
        id,
        group,
        label,
        icon,
        terms,
        requires_pro,
    }
}

pub const PAGES: &[Page] = &[
    page_entry(
        "app",
        "App",
        "General",
        "product/Settings02Icon.svg",
        &[
            "Start Anarlog at login",
            "Automatically install updates",
            "Show app in Dock",
            "Show tray icon",
            "Language & Region",
            "Main language",
            "Use main language for summaries",
            "Additional spoken languages",
            "Timezone",
            "Week starts on",
            "Storage",
            "Export location",
        ],
        false,
    ),
    page_entry(
        "account",
        "App",
        "Account",
        "product/UserIcon.svg",
        &[
            "Your info",
            "Connected accounts",
            "Manage sign-in methods",
            "Sign out",
        ],
        false,
    ),
    page_entry(
        "billing",
        "App",
        "Billing",
        "product/CreditCardIcon.svg",
        &["Your plan", "Manage billing", "Add payment method"],
        false,
    ),
    page_entry(
        "referrals",
        "App",
        "Invite friends",
        "workspace/UsersIcon.svg",
        &["Free months earned"],
        false,
    ),
    page_entry(
        "insights",
        "App",
        "Insights",
        "product/ChartLineData01Icon.svg",
        &["Hours transcribed", "Daily conversations"],
        false,
    ),
    // Shipping unlocks Teams once the account already belongs to a workspace.
    page_entry(
        "team",
        "App",
        "Teams",
        "product/UserGroupIcon.svg",
        &["Members", "Join automatically with a work email"],
        true,
    ),
    page_entry(
        "sync",
        "App",
        "Sync",
        "product/RefreshIcon.svg",
        &[
            "Cloud sync",
            "Devices",
            "Sync now",
            "Use recovery key instead",
        ],
        true,
    ),
    page_entry(
        "appearance",
        "App",
        "Appearance",
        "product/Sun01Icon.svg",
        &[
            "Theme",
            "Light",
            "Dark",
            "System",
            "Use 24-hour time",
            "App icon",
            "Notes list",
            "Folder",
            "Tags",
        ],
        false,
    ),
    page_entry(
        "notifications",
        "App",
        "Notifications",
        "product/BellIcon.svg",
        &[
            "Disable all notifications",
            "Summary complete",
            "Cloud sync complete",
            "Event notifications",
            "Recording status prompts",
            "Completion sound",
            "Bounce app icon",
            "Microphone detection",
            "Detection delay",
            "Exclude apps from detection",
            "Respect Do-Not-Disturb mode",
        ],
        false,
    ),
    page_entry(
        "transcription",
        "AI",
        "Transcription",
        "product/AudioWaveformIcon.svg",
        &["Configure Providers", "API Key"],
        false,
    ),
    page_entry(
        "dictation",
        "AI",
        "Dictation",
        "product/Mic01Icon.svg",
        &[
            "Enable dictation",
            "Microphone",
            "Live transcript preview",
            "Hands-free dictation",
            "Click here, then use your dictation shortcut.",
            "Last dictation",
        ],
        true,
    ),
    page_entry(
        "intelligence",
        "AI",
        "Intelligence",
        "product/SparkleIcon.svg",
        &["Configure Providers", "API Key"],
        false,
    ),
    page_entry(
        "dictionary",
        "AI",
        "Dictionary",
        "product/BookOpen01Icon.svg",
        &["Add names, jargon, or product terms to prefer"],
        true,
    ),
    page_entry(
        "meetings",
        "Workspace",
        "Meetings",
        "product/Video01Icon.svg",
        &[
            "Default sharing",
            "Primary recording device",
            "Start when meeting begins",
            "Join scheduled meetings",
            "Stop when meeting ends",
            "Post recording disclosure in meeting chat",
            "Capture meeting chat in Memos",
            "Capture shared screens in Memos",
            "Show floating bar",
            "Microphone",
            "Audio file retention",
            "Choose how long recordings stay on this device.",
            "Remember speakers",
        ],
        false,
    ),
    page_entry(
        "folders",
        "Workspace",
        "Folders",
        "workspace/Folder01Icon.svg",
        &[],
        false,
    ),
    page_entry(
        "calendar",
        "Workspace",
        "Calendar",
        "workspace/Calendar03Icon.svg",
        &[],
        false,
    ),
    page_entry(
        "contacts",
        "Workspace",
        "Contacts",
        "workspace/UsersIcon.svg",
        &[],
        false,
    ),
    page_entry(
        "templates",
        "Workspace",
        "Templates",
        "workspace/FileTextIcon.svg",
        &[],
        false,
    ),
    page_entry(
        "automations",
        "Workspace",
        "Automations",
        "workspace/ZapIcon.svg",
        &[],
        true,
    ),
    page_entry(
        "imports",
        "Data",
        "Imports",
        "product/Download01Icon.svg",
        &[],
        false,
    ),
    page_entry(
        "crm",
        "Data",
        "CRM",
        "product/Building03Icon.svg",
        &[],
        false,
    ),
    page_entry(
        "privacy",
        "Advanced",
        "Privacy",
        "product/ShieldCheckIcon.svg",
        &[
            "Lock app",
            "Share usage data",
            "Send sanitized crash and error reports to help improve Anarlog.",
        ],
        false,
    ),
    page_entry(
        "permissions",
        "Advanced",
        "Permissions",
        "product/LockPasswordIcon.svg",
        &["Microphone", "System audio", "Accessibility", "Calendar"],
        false,
    ),
    page_entry(
        "developers",
        "Advanced",
        "Developers",
        "product/CodeIcon.svg",
        &[
            "CLI & MCP",
            "Anarlog CLI",
            "MCP server",
            "Agent skills",
            "Cloud API & Connectors",
            "Cloud API keys",
            "Webhooks",
        ],
        false,
    ),
];

/// Pages that open a workspace view instead of a settings page.
pub fn opens_workspace(page: Page) -> bool {
    matches!(
        page.id,
        "folders" | "calendar" | "contacts" | "templates" | "automations"
    )
}

pub fn page(id: &str) -> Page {
    if id == "todo" {
        return page_entry("todo", "Workspace", "Todo", "", &[], false);
    }
    let id = match id {
        "data" => "imports",
        "personalization" => "dictionary",
        "audio" => "meetings",
        "stats" => "insights",
        id => id,
    };
    PAGES
        .iter()
        .find(|page| page.id == id)
        .copied()
        .unwrap_or(PAGES[0])
}

/// Every whitespace-separated query term must appear in the group, label, or search terms.
pub fn matches(page: Page, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let text = [page.group, page.label]
        .iter()
        .chain(page.terms)
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    query.split_whitespace().all(|term| text.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(query: &str) -> Vec<&'static str> {
        PAGES
            .iter()
            .filter(|page| matches(**page, query))
            .map(|page| page.id)
            .collect()
    }

    #[test]
    fn aliases_resolve_like_shipping_navigation() {
        for (alias, canonical) in [
            ("data", "imports"),
            ("personalization", "dictionary"),
            ("audio", "meetings"),
            ("stats", "insights"),
        ] {
            assert_eq!(page(alias), page(canonical));
        }
        assert_eq!(page("unknown").id, "app");
    }

    #[test]
    fn search_finds_pages_by_the_controls_they_contain() {
        assert_eq!(found("dock"), ["app"]);
        assert_eq!(found(" 24-HOUR "), ["appearance"]);
        assert_eq!(
            found("api key"),
            ["transcription", "intelligence", "developers"]
        );
        assert_eq!(found("advanced lock"), ["privacy"]);
        assert_eq!(
            found("microphone"),
            ["notifications", "dictation", "meetings", "permissions"]
        );
        assert!(found("no such setting").is_empty());
        assert_eq!(found("").len(), PAGES.len());
    }
}
