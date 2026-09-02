pub fn preferred_position_key(autosave_name: &str) -> String {
    format!("NSStatusItem Preferred Position {autosave_name}")
}

#[cfg(target_os = "macos")]
pub fn seed_default_position(autosave_name: &str) {
    use objc2_foundation::{NSString, NSUserDefaults};

    let defaults = NSUserDefaults::standardUserDefaults();
    let key = NSString::from_str(&preferred_position_key(autosave_name));
    if defaults.objectForKey(&key).is_some() {
        return;
    }

    // AppKit treats this as distance from the right edge of the status area.
    // 0 claims the rightmost third-party slot, just left of system extras.
    // Only seed when unset so a user ⌘-drag keeps winning on later launches.
    defaults.setDouble_forKey(0.0, &key);
}

#[cfg(target_os = "macos")]
pub fn apply_autosave_name(app: &tauri::AppHandle<tauri::Wry>, autosave_name: &'static str) {
    use objc2_foundation::NSString;

    let Some(tray) = app.tray_by_id(autosave_name) else {
        return;
    };

    if let Err(error) = tray.with_inner_tray_icon(move |inner: &tray_icon::TrayIcon| {
        let Some(item) = inner.ns_status_item() else {
            return;
        };
        item.setAutosaveName(Some(&NSString::from_str(autosave_name)));
    }) {
        tracing::warn!(%error, "failed to set tray status item autosave name");
    }
}

#[cfg(test)]
mod tests {
    use super::preferred_position_key;

    #[test]
    fn preferred_position_key_matches_appkit_convention() {
        assert_eq!(
            preferred_position_key("anlg-tray"),
            "NSStatusItem Preferred Position anlg-tray"
        );
    }
}
