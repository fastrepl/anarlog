use std::sync::atomic::{AtomicBool, Ordering};

static WANTED_VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn preferred_position_key(autosave_name: &str) -> String {
    format!("NSStatusItem Preferred Position {autosave_name}")
}

pub fn visibility_key(autosave_name: &str) -> String {
    format!("NSStatusItem Visible {autosave_name}")
}

pub fn set_wanted_visible(visible: bool) {
    WANTED_VISIBLE.store(visible, Ordering::SeqCst);
}

pub fn wanted_visible() -> bool {
    WANTED_VISIBLE.load(Ordering::SeqCst)
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
    use objc2_foundation::{NSString, NSUserDefaults};

    let defaults = NSUserDefaults::standardUserDefaults();
    // Visibility is owned by show_tray_icon. setAutosaveName also restores
    // `NSStatusItem Visible`, so a prior set_visible(false) would hide the
    // item we just created to show.
    defaults.removeObjectForKey(&NSString::from_str(&visibility_key(autosave_name)));

    let Some(tray) = app.tray_by_id(autosave_name) else {
        return;
    };

    if let Err(error) = tray.with_inner_tray_icon(move |inner: &tray_icon::TrayIcon| {
        let Some(item) = inner.ns_status_item() else {
            return;
        };
        item.setAutosaveName(Some(&NSString::from_str(autosave_name)));
        item.setVisible(true);
    }) {
        tracing::warn!(%error, "failed to set tray status item autosave name");
    }

    let app = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if !wanted_visible() {
            return;
        }
        if let Some(tray) = app.tray_by_id(autosave_name) {
            let _ = tray.set_visible(true);
        }
    }) {
        tracing::warn!(%error, "failed to force tray icon visible after autosave restore");
    }
}

#[cfg(test)]
mod tests {
    use super::{preferred_position_key, visibility_key};

    #[test]
    fn preferred_position_key_matches_appkit_convention() {
        assert_eq!(
            preferred_position_key("anlg-tray"),
            "NSStatusItem Preferred Position anlg-tray"
        );
    }

    #[test]
    fn visibility_key_matches_appkit_convention() {
        assert_eq!(
            visibility_key("anlg-tray"),
            "NSStatusItem Visible anlg-tray"
        );
    }

    #[test]
    fn deferred_show_respects_a_later_hide() {
        super::set_wanted_visible(true);
        assert!(super::wanted_visible());
        super::set_wanted_visible(false);
        assert!(!super::wanted_visible());
        super::set_wanted_visible(true);
    }
}
