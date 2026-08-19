include!("src/manifest.rs");

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
