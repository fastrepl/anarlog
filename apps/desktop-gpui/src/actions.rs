//! Keyboard-driven commands of the main window, bound to the same keys the
//! Tauri app uses (`react-hotkeys-hook` bindings and the Windows/Linux title
//! bar menus). `mod` is Cmd on macOS and Ctrl elsewhere.

use gpui::{App, KeyBinding, actions};

actions!(
    anarlog,
    [
        NewNote,
        StartRecording,
        OpenSettings,
        OpenTranscriptionSettings,
        OpenIntelligenceSettings,
        ToggleSidebar,
        ToggleFullscreen,
        CloseWindow,
        FocusSearch,
        OpenNoteDialog,
        Escape,
        ToggleChat,
        PreviousView,
        NextView,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
    ]
);

pub const KEY_CONTEXT: &str = "Workspace";

fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    }
}

pub fn bind_keys(cx: &mut App) {
    let m = modifier();
    let ctx = Some(KEY_CONTEXT);
    let mut bindings = vec![
        KeyBinding::new(&format!("{m}-n"), NewNote, ctx),
        KeyBinding::new(&format!("{m}-shift-n"), StartRecording, ctx),
        KeyBinding::new(&format!("{m}-,"), OpenSettings, ctx),
        KeyBinding::new(&format!("{m}-\\"), ToggleSidebar, ctx),
        KeyBinding::new(&format!("{m}-f"), FocusSearch, ctx),
        KeyBinding::new(&format!("{m}-k"), OpenNoteDialog, ctx),
        KeyBinding::new(&format!("{m}-j"), ToggleChat, ctx),
        KeyBinding::new(&format!("{m}-alt-left"), PreviousView, ctx),
        KeyBinding::new(&format!("{m}-alt-right"), NextView, ctx),
        KeyBinding::new(&format!("{m}-z"), Undo, ctx),
        KeyBinding::new(&format!("{m}-y"), Redo, ctx),
        KeyBinding::new(&format!("{m}-x"), Cut, ctx),
        KeyBinding::new(&format!("{m}-c"), Copy, ctx),
        KeyBinding::new(&format!("{m}-v"), Paste, ctx),
        KeyBinding::new(&format!("{m}-a"), SelectAll, ctx),
    ];
    bindings.push(KeyBinding::new("escape", Escape, ctx));
    if cfg!(any(target_os = "windows", target_os = "linux")) {
        bindings.push(KeyBinding::new("f11", ToggleFullscreen, ctx));
        bindings.push(KeyBinding::new("alt-f4", CloseWindow, ctx));
    }
    cx.bind_keys(bindings);
}

/// Shortcut labels as the title bar menus print them.
pub fn shortcut_label(keys: &str) -> String {
    if cfg!(target_os = "macos") {
        keys.replace("Ctrl", "⌘")
    } else {
        keys.to_string()
    }
}
