use super::settings::Definition;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Page {
    pub id: &'static str,
    pub group: &'static str,
    pub label: &'static str,
}

pub const PAGES: &[Page] = &[
    Page {
        id: "app",
        group: "App",
        label: "General",
    },
    Page {
        id: "account",
        group: "App",
        label: "Account",
    },
    Page {
        id: "billing",
        group: "App",
        label: "Billing",
    },
    Page {
        id: "insights",
        group: "App",
        label: "Insights",
    },
    Page {
        id: "team",
        group: "App",
        label: "Teams",
    },
    Page {
        id: "sync",
        group: "App",
        label: "Sync",
    },
    Page {
        id: "appearance",
        group: "App",
        label: "Appearance",
    },
    Page {
        id: "notifications",
        group: "App",
        label: "Notifications",
    },
    Page {
        id: "transcription",
        group: "AI",
        label: "Transcription",
    },
    Page {
        id: "dictation",
        group: "AI",
        label: "Dictation",
    },
    Page {
        id: "intelligence",
        group: "AI",
        label: "Intelligence",
    },
    Page {
        id: "dictionary",
        group: "AI",
        label: "Dictionary",
    },
    Page {
        id: "meetings",
        group: "Workspace",
        label: "Meetings",
    },
    Page {
        id: "folders",
        group: "Workspace",
        label: "Folders",
    },
    Page {
        id: "calendar",
        group: "Workspace",
        label: "Calendar",
    },
    Page {
        id: "contacts",
        group: "Workspace",
        label: "Contacts",
    },
    Page {
        id: "templates",
        group: "Workspace",
        label: "Templates",
    },
    Page {
        id: "automations",
        group: "Workspace",
        label: "Automations",
    },
    Page {
        id: "imports",
        group: "Data",
        label: "Imports",
    },
    Page {
        id: "privacy",
        group: "Advanced",
        label: "Privacy",
    },
    Page {
        id: "permissions",
        group: "Advanced",
        label: "Permissions",
    },
    Page {
        id: "developers",
        group: "Advanced",
        label: "Developers",
    },
];

pub fn page(id: &str) -> Page {
    if id == "todo" {
        return Page {
            id: "todo",
            group: "Workspace",
            label: "Todo",
        };
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

pub fn matches(page: Page, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    page.label.to_lowercase().contains(&query) || page.group.to_lowercase().contains(&query)
}

pub fn section(def: &Definition) -> &'static str {
    match def.key.as_str() {
        "theme"
        | "app_icon"
        | "sidebar_show_folder"
        | "sidebar_show_tags"
        | "floating_bar_opacity"
        | "live_caption_opacity"
        | "live_caption_width"
        | "live_caption_line_count"
        | "live_caption_position"
        | "live_caption_minimized" => "appearance",
        "telemetry_consent" | "crash_reporting_consent" | "lock_app" | "consent_auto_send_chat" => {
            "privacy"
        }
        "cloud_sync_enabled" => "sync",
        "personalization_dictionary_terms" => "dictionary",
        "current_stt_provider"
        | "current_stt_model"
        | "local_stt_model_path"
        | "spoken_languages" => "transcription",
        "ai_language"
        | "custom_summary_instructions"
        | "custom_summary_instructions_token_aware" => "intelligence",
        "auto_stop_meetings"
        | "auto_start_scheduled_meetings"
        | "auto_join_scheduled_meetings"
        | "save_recordings"
        | "audio_retention"
        | "remember_speakers"
        | "microphone_device"
        | "capture_meeting_chat"
        | "default_meeting_share_access" => "meetings",
        _ => match def.path[0].as_str() {
            "notification" => "notifications",
            "dictation" => "dictation",
            "ai" => "intelligence",
            "automations" => "automations",
            "todo" => "todo",
            _ => "app",
        },
    }
}

pub fn label(key: &str) -> String {
    let mut label = key.replace('_', " ");
    if let Some(first) = label.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_and_group_search_match_shipping_navigation() {
        for (alias, canonical) in [
            ("data", "imports"),
            ("personalization", "dictionary"),
            ("audio", "meetings"),
            ("stats", "insights"),
        ] {
            assert_eq!(page(alias), page(canonical));
        }
        assert_eq!(page("unknown").id, "app");
        assert!(matches(page("intelligence"), " AI "));
        assert!(matches(page("privacy"), "advanced"));
        assert!(!matches(page("privacy"), "account"));
    }
}
