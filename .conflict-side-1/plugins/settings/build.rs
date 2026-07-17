const COMMANDS: &[&str] = &[
    "global_base",
    "vault_base",
    "move_vault",
    "copy_vault",
    "set_vault_base",
    "is_empty_or_missing_dir",
    "obsidian_vaults",
    "path",
    "load",
    "save",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
