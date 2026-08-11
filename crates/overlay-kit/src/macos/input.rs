use objc2_core_graphics::{CGEvent, CGEventFlags, CGPreflightPostEventAccess};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyModifier {
    Command,
    Option,
    Control,
    Shift,
}

/// Whether this process currently has permission to post synthetic events.
pub fn can_post_key_events() -> bool {
    CGPreflightPostEventAccess()
}

/// Post one modified key press directly to a macOS process.
///
/// The caller owns the application-specific virtual key code and modifier
/// policy. This helper only validates that posting is possible and emits the
/// matching key-down and key-up events.
pub fn post_key_chord_to_pid(pid: i32, key_code: u16, modifiers: &[KeyModifier]) -> bool {
    if pid <= 0 || !can_post_key_events() {
        return false;
    }

    let Some(key_down) = CGEvent::new_keyboard_event(None, key_code, true) else {
        return false;
    };
    let Some(key_up) = CGEvent::new_keyboard_event(None, key_code, false) else {
        return false;
    };
    let flags = event_flags(modifiers);
    CGEvent::set_flags(Some(&key_down), flags);
    CGEvent::set_flags(Some(&key_up), flags);
    CGEvent::post_to_pid(pid, Some(&key_down));
    CGEvent::post_to_pid(pid, Some(&key_up));
    true
}

fn event_flags(modifiers: &[KeyModifier]) -> CGEventFlags {
    modifiers
        .iter()
        .fold(CGEventFlags::empty(), |flags, modifier| {
            flags
                | match modifier {
                    KeyModifier::Command => CGEventFlags::MaskCommand,
                    KeyModifier::Option => CGEventFlags::MaskAlternate,
                    KeyModifier::Control => CGEventFlags::MaskControl,
                    KeyModifier::Shift => CGEventFlags::MaskShift,
                }
        })
}

#[cfg(test)]
mod tests {
    use objc2_core_graphics::CGEventFlags;

    use super::{KeyModifier, event_flags};

    #[test]
    fn maps_only_requested_key_modifiers() {
        let flags = event_flags(&[KeyModifier::Command, KeyModifier::Option]);

        assert!(flags.contains(CGEventFlags::MaskCommand));
        assert!(flags.contains(CGEventFlags::MaskAlternate));
        assert!(!flags.contains(CGEventFlags::MaskControl));
        assert!(!flags.contains(CGEventFlags::MaskShift));
    }
}
