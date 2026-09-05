use std::sync::atomic::{AtomicBool, Ordering};

static WANTED_VISIBLE: AtomicBool = AtomicBool::new(true);

// Bump when the seeding rule changes so installs placed by an older rule get
// re-seeded once. 1.4.20 seeded 0, which parked the icon next to Control Center.
const SEED_VERSION: isize = 2;

pub fn preferred_position_key(autosave_name: &str) -> String {
    format!("NSStatusItem Preferred Position {autosave_name}")
}

pub fn visibility_key(autosave_name: &str) -> String {
    format!("NSStatusItem Visible {autosave_name}")
}

pub fn seed_version_key(autosave_name: &str) -> String {
    format!("{autosave_name} position seed version")
}

// Only seed when unset so a user ⌘-drag keeps winning on later launches.
pub fn should_seed(has_position: bool, seeded_version: isize) -> bool {
    !has_position || seeded_version < SEED_VERSION
}

pub fn set_wanted_visible(visible: bool) {
    WANTED_VISIBLE.store(visible, Ordering::SeqCst);
}

pub fn wanted_visible() -> bool {
    WANTED_VISIBLE.load(Ordering::SeqCst)
}

// AppKit orders status items by preferred position, measured from the right
// edge of the status area, so anything above Wi-Fi's own value lands to its
// left. Without a Wi-Fi reference (key missing, or the sandbox denying reads
// of Control Center's domain) we leave the position unset and let AppKit use
// its default slot at the left end of the status items.
pub fn seed_position(wifi_position: Option<f64>) -> Option<f64> {
    wifi_position.map(|position| position + 1.0)
}

// Control Center autosaves the system extras (Wi-Fi, Bluetooth, ...) in its
// own defaults domain with the same key format as third-party status items.
#[cfg(target_os = "macos")]
fn wifi_position() -> Option<f64> {
    use objc2::AllocAnyThread;
    use objc2_foundation::{NSString, NSUserDefaults};

    let defaults = NSUserDefaults::initWithSuiteName(
        NSUserDefaults::alloc(),
        Some(&NSString::from_str("com.apple.controlcenter")),
    )?;
    let key = NSString::from_str(&preferred_position_key("WiFi"));
    defaults.objectForKey(&key)?;
    Some(defaults.doubleForKey(&key))
}

#[cfg(target_os = "macos")]
pub fn seed_default_position(autosave_name: &str) {
    use objc2_foundation::{NSString, NSUserDefaults};

    let defaults = NSUserDefaults::standardUserDefaults();
    let key = NSString::from_str(&preferred_position_key(autosave_name));
    let version_key = NSString::from_str(&seed_version_key(autosave_name));
    if !should_seed(
        defaults.objectForKey(&key).is_some(),
        defaults.integerForKey(&version_key),
    ) {
        return;
    }

    defaults.setInteger_forKey(SEED_VERSION, &version_key);
    match seed_position(wifi_position()) {
        Some(position) => defaults.setDouble_forKey(position, &key),
        None => defaults.removeObjectForKey(&key),
    }
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

    let app_for_callback = app.clone();
    if let Err(error) = app.run_on_main_thread(move || {
        if !wanted_visible() {
            return;
        }
        if let Some(tray) = app_for_callback.tray_by_id(autosave_name) {
            let _ = tray.set_visible(true);
        }
    }) {
        tracing::warn!(%error, "failed to force tray icon visible after autosave restore");
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SEED_VERSION, preferred_position_key, seed_position, seed_version_key, should_seed,
        visibility_key,
    };

    #[test]
    fn preferred_position_key_matches_appkit_convention() {
        assert_eq!(
            preferred_position_key("anlg-tray"),
            "NSStatusItem Preferred Position anlg-tray"
        );
    }

    #[test]
    fn seed_version_key_stays_out_of_the_appkit_namespace() {
        assert_eq!(
            seed_version_key("anlg-tray"),
            "anlg-tray position seed version"
        );
    }

    #[test]
    fn seed_position_lands_just_left_of_wifi() {
        assert_eq!(seed_position(Some(211.0)), Some(212.0));
    }

    #[test]
    fn seed_position_defers_to_appkit_without_wifi() {
        assert_eq!(seed_position(None), None);
    }

    #[test]
    fn seeds_fresh_installs_and_positions_from_older_rules() {
        assert!(should_seed(false, 0));
        assert!(should_seed(true, 0));
        assert!(should_seed(true, SEED_VERSION - 1));
    }

    #[test]
    fn keeps_a_position_already_seeded_by_the_current_rule() {
        assert!(!should_seed(true, SEED_VERSION));
        assert!(!should_seed(true, SEED_VERSION + 1));
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
