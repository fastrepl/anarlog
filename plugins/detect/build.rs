const COMMANDS: &[&str] = &[
    "list_installed_applications",
    "get_installed_application_icons",
    "terminate_competing_applications",
    "set_competing_application_termination_paused",
    "list_mic_using_applications",
    "set_respect_do_not_disturb",
    "set_ignored_bundle_ids",
    "list_default_ignored_bundle_ids",
    "inspect_meeting_accessibility",
    "send_meeting_chat_message",
    "capture_meeting_chat_messages",
    "get_preferred_languages",
    "get_current_locale_identifier",
    "set_mic_active_threshold",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
