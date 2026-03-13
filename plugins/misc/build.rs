const COMMANDS: &[&str] = &[
    "get_git_hash",
    "get_fingerprint",
    "get_device_info",
    "opinionated_md_to_html",
    "delete_session_folder",
    "parse_meeting_link",
    "audio_open",
    "audio_exist",
    "audio_delete",
    "audio_path",
    "audio_import",
    "reveal_session_in_finder",
];

fn main() {
    let gitcl = vergen_gix::GixBuilder::default()
        .sha(false)
        .build()
        .unwrap();
    vergen_gix::Emitter::default()
        .add_instructions(&gitcl)
        .unwrap()
        .emit()
        .unwrap();

    tauri_plugin::Builder::new(COMMANDS).build();
}
