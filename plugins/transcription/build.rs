const COMMANDS: &[&str] = &[
    "list_microphone_devices",
    "get_current_microphone_device",
    "get_mic_muted",
    "set_mic_muted",
    "start_capture",
    "stop_capture",
    "get_capture_state",
    "is_supported_languages_live",
    "suggest_providers_for_languages_live",
    "list_documented_language_codes_live",
    "render_transcript_segments",
    "start_transcription",
    "stop_transcription",
    "run_denoise",
    "parse_subtitle",
    "export_to_vtt",
    "is_supported_languages_batch",
    "suggest_providers_for_languages_batch",
    "list_documented_language_codes_batch",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
