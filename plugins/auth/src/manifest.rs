// Single source of truth for this plugin's Tauri commands. build.rs includes
// this file to drive permission generation, and the lib tests assert the
// specta bindings and generated permission files stay in sync with it.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const COMMANDS: &[&str] = &[
    "decode_claims",
    "get_item",
    "set_item",
    "remove_item",
    "clear",
    "get_account_info",
];
