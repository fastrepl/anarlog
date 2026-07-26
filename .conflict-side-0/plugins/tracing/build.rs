const COMMANDS: &[&str] = &["logs_dir", "do_log", "log_content"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
