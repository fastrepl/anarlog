const COMMANDS: &[&str] = &[
    "list_microphone_devices",
    "get_current_microphone_device",
    "get_mic_muted",
    "set_mic_muted",
    "start_capture",
    "stop_capture",
    "get_capture_state",
    "get_capture_snapshot",
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
    "extract_voiceprint_candidates",
    "promote_voiceprint_candidates",
    "cleanup_expired_voiceprint_candidates",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
