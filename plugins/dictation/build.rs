const COMMANDS: &[&str] = &[
    "capture_target",
    "begin_dictation",
    "attach_dictation_stream",
    "finish_dictation",
    "complete_dictation",
    "release_dictation",
    "cancel_dictation",
    "retire_dictations",
    "list_dictation_recovery",
    "prepare_dictation_recovery_audio",
    "save_dictation_recovery_text",
    "claim_dictation_recovery",
    "discard_dictation_recovery",
    "fit_dictation_text",
    "prepare_dictation_refinement",
    "resolve_dictation_refinement",
    "suggest_dictation_terms",
    "rank_dictation_terms",
    "record_dictation_terms",
    "read_dictation_context",
    "insert_dictation",
    "read_dictation_kept",
    "release_dictation_target",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
