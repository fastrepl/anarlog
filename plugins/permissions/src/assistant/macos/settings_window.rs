use objc2::rc::Retained;
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSScreen};
use objc2_core_foundation::CGRect;
use objc2_foundation::{MainThreadMarker, NSRect, NSString};

use anlg_overlay_kit::macos::{
    application::frontmost_application_identity,
    geometry::{cg_frame_to_appkit, screen_geometries},
    support::rect,
    window::onscreen_windows,
};

use super::layout::{MIN_SETTINGS_WINDOW_HEIGHT, MIN_SETTINGS_WINDOW_WIDTH};

const SYSTEM_SETTINGS_BUNDLE_ID: &str = "com.apple.systempreferences";

#[derive(Clone, Copy, Debug)]
pub(crate) struct SettingsWindowSnapshot {
    pub(crate) frame: NSRect,
    pub(crate) visible_frame: NSRect,
}

impl SettingsWindowSnapshot {
    pub(crate) fn locate_visible_frontmost_window() -> Option<Self> {
        Self::visible_frontmost_window()
    }

    pub(crate) fn locate_with_launch_fallback() -> Option<Self> {
        Self::visible_frontmost_window().or_else(Self::fallback)
    }

    fn visible_frontmost_window() -> Option<Self> {
        if !is_system_settings_frontmost() {
            return None;
        }

        let app = best_settings_running_app()?;
        let pid = app.processIdentifier();
        let mtm = MainThreadMarker::new()?;
        let screens = screen_geometries(mtm);

        onscreen_windows()
            .into_iter()
            .filter_map(|window| {
                settings_window_snapshot_from_candidate(
                    SettingsWindowCandidate {
                        owner_pid: window.owner_pid,
                        layer: window.layer,
                        is_onscreen: window.is_onscreen,
                        frame: window.cg_bounds,
                    },
                    pid,
                    &screens,
                )
            })
            .max_by(|left, right| {
                rect_area(left.frame)
                    .partial_cmp(&rect_area(right.frame))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn fallback() -> Option<Self> {
        if !is_system_settings_frontmost() {
            return None;
        }
        let mtm = MainThreadMarker::new()?;
        let screen = NSScreen::mainScreen(mtm).or_else(|| NSScreen::screens(mtm).iter().next())?;
        let visible_frame = screen.visibleFrame();
        Some(Self {
            frame: inset(visible_frame, 80.0, 70.0),
            visible_frame,
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct SettingsWindowCandidate {
    owner_pid: i32,
    layer: i32,
    is_onscreen: Option<bool>,
    frame: CGRect,
}

fn settings_window_snapshot_from_candidate(
    candidate: SettingsWindowCandidate,
    pid: i32,
    screens: &[anlg_overlay_kit::macos::geometry::ScreenGeometry],
) -> Option<SettingsWindowSnapshot> {
    if candidate.owner_pid != pid {
        return None;
    }

    if candidate.layer != 0 {
        return None;
    }

    if candidate.is_onscreen == Some(false) {
        return None;
    }

    if candidate.frame.size.width <= MIN_SETTINGS_WINDOW_WIDTH
        || candidate.frame.size.height <= MIN_SETTINGS_WINDOW_HEIGHT
    {
        return None;
    }

    let (frame, visible_frame) = cg_frame_to_appkit(candidate.frame, screens)?;
    Some(SettingsWindowSnapshot {
        frame,
        visible_frame,
    })
}

fn best_settings_running_app() -> Option<Retained<NSRunningApplication>> {
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        SYSTEM_SETTINGS_BUNDLE_ID,
    ));
    apps.into_iter().max_by_key(|app| {
        if app.activationPolicy() == NSApplicationActivationPolicy::Prohibited {
            0
        } else {
            1
        }
    })
}

fn is_system_settings_frontmost() -> bool {
    frontmost_application_identity()
        .is_some_and(|identity| identity.matches_bundle_identifier(SYSTEM_SETTINGS_BUNDLE_ID))
}

fn inset(source: NSRect, dx: f64, dy: f64) -> NSRect {
    rect(
        source.origin.x + dx,
        source.origin.y + dy,
        (source.size.width - (dx * 2.0)).max(0.0),
        (source.size.height - (dy * 2.0)).max(0.0),
    )
}

fn rect_area(rect: NSRect) -> f64 {
    rect.size.width * rect.size.height
}

#[cfg(test)]
mod tests {
    use anlg_overlay_kit::macos::geometry::ScreenGeometry;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};

    use super::*;

    const PID: i32 = 42;

    fn screen() -> ScreenGeometry {
        ScreenGeometry {
            frame: rect(0.0, 0.0, 1440.0, 900.0),
            visible_frame: rect(0.0, 0.0, 1440.0, 860.0),
            cg_bounds: CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(1440.0, 900.0)),
        }
    }

    fn candidate(is_onscreen: Option<bool>) -> SettingsWindowCandidate {
        SettingsWindowCandidate {
            owner_pid: PID,
            layer: 0,
            is_onscreen,
            frame: CGRect::new(CGPoint::new(100.0, 100.0), CGSize::new(900.0, 700.0)),
        }
    }

    #[test]
    fn settings_window_candidate_rejects_hidden_window() {
        let screens = [screen()];
        assert!(
            settings_window_snapshot_from_candidate(candidate(Some(false)), PID, &screens)
                .is_none()
        );
    }

    #[test]
    fn settings_window_candidate_accepts_visible_window() {
        let screens = [screen()];
        assert!(
            settings_window_snapshot_from_candidate(candidate(Some(true)), PID, &screens).is_some()
        );
    }
}
