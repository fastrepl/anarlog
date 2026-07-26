const COMMANDS: &[&str] = &[
    "window_show",
    "window_hide",
    "window_destroy",
    "window_navigate",
    "window_emit_navigate",
    "window_is_exists",
    "window_is_occluded",
    "window_set_frame_animated",
    "window_save_frame",
    "window_restore_frame_animated",
    "window_expand_width",
    "window_restore_width",
    "set_show_app_in_dock",
    "floating_bar_show",
    "floating_bar_hide",
    "floating_bar_update",
    "live_caption_show",
    "live_caption_hide",
    "live_caption_update",
    "devtools_panel_show",
    "devtools_panel_hide",
];

fn main() {
    #[cfg(target_os = "macos")]
    {
        swift_rs::SwiftLinker::new("14.2")
            .with_package("windows-swift", "./swift-lib/")
            .link();
    }

    #[cfg(not(target_os = "macos"))]
    {
        println!("cargo:warning=Swift linking is only available on macOS");
    }

    tauri_plugin::Builder::new(COMMANDS).build();
}
