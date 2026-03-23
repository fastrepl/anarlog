const COMMANDS: &[&str] = &[
    "list_installed_applications",
    "list_mic_using_applications",
    "set_respect_do_not_disturb",
    "set_ignored_bundle_ids",
    "list_default_ignored_bundle_ids",
    "get_preferred_languages",
    "get_current_locale_identifier",
    "set_mic_active_threshold",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
