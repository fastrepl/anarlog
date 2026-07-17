const COMMANDS: &[&str] = &[
    "list_input_devices",
    "list_output_devices",
    "set_default_input_device",
    "set_default_output_device",
    "get_input_priorities",
    "get_output_priorities",
    "save_input_priorities",
    "save_output_priorities",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
