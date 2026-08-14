const COMMANDS: &[&str] = &[
    "show",
    "hide",
    "set_phase",
    "update_amplitude",
    "start_recording",
    "stop_recording",
    "cancel_recording",
    "discard_recording",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
