const COMMANDS: &[&str] = &[
    "search",
    "reindex",
    "add_document",
    "update_document",
    "update_documents",
    "remove_document",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
