use tauri_plugin_global_shortcut::{Modifiers, Shortcut};

pub(super) fn portal_trigger(key: Shortcut) -> Result<String, String> {
    let mut parts = Vec::new();
    for (flag, label) in [
        (Modifiers::CONTROL, "CTRL"),
        (Modifiers::ALT, "ALT"),
        (Modifiers::SHIFT, "SHIFT"),
        (Modifiers::SUPER, "LOGO"),
    ] {
        if key.mods.contains(flag) {
            parts.push(label.to_string());
        }
    }
    let code = key.key.to_string();
    parts.push(match code.as_str() {
        "Space" => "space".into(),
        "Backspace" => "BackSpace".into(),
        "Enter" => "Return".into(),
        "NumpadEnter" => "KP_Enter".into(),
        "NumpadAdd" => "KP_Add".into(),
        "NumpadSubtract" => "KP_Subtract".into(),
        "NumpadMultiply" => "KP_Multiply".into(),
        "NumpadDivide" => "KP_Divide".into(),
        "NumpadDecimal" => "KP_Decimal".into(),
        "NumpadComma" => "KP_Separator".into(),
        "NumpadEqual" => "KP_Equal".into(),
        code if code
            .strip_prefix("Numpad")
            .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit()) =>
        {
            format!("KP_{}", &code[6..])
        }
        "ArrowLeft" => "Left".into(),
        "ArrowRight" => "Right".into(),
        "ArrowUp" => "Up".into(),
        "ArrowDown" => "Down".into(),
        "Minus" => "minus".into(),
        "Equal" => "equal".into(),
        "BracketLeft" => "bracketleft".into(),
        "BracketRight" => "bracketright".into(),
        "Backslash" => "backslash".into(),
        "Semicolon" => "semicolon".into(),
        "Quote" => "apostrophe".into(),
        "Backquote" => "grave".into(),
        "Comma" => "comma".into(),
        "Period" => "period".into(),
        "Slash" => "slash".into(),
        "PageUp" => "Prior".into(),
        "PageDown" => "Next".into(),
        "CapsLock" => "Caps_Lock".into(),
        "ContextMenu" => "Menu".into(),
        "NumLock" => "Num_Lock".into(),
        "PrintScreen" => "Print".into(),
        "ScrollLock" => "Scroll_Lock".into(),
        "IntlBackslash" => "backslash".into(),
        "IntlYen" => "yen".into(),
        "Tab" | "Escape" | "Delete" | "Insert" | "Home" | "End" | "Pause" => code.clone(),
        value if value.strip_prefix("Key").is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_uppercase()) => value[3..].into(),
        value if value.strip_prefix("Digit").is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit()) => value[5..].into(),
        value if value.strip_prefix('F').and_then(|s| s.parse::<u8>().ok()).is_some_and(|n| (1..=35).contains(&n)) => code.clone(),
        _ => return Err(format!("The {code} key is not supported for Wayland dictation shortcuts. Choose another key.")),
    });
    Ok(parts.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Code;

    #[test]
    fn translates_portal_keys_without_forwarding_web_code_names() {
        for (key, expected) in [
            (Code::PageUp, "Prior"),
            (Code::PageDown, "Next"),
            (Code::CapsLock, "Caps_Lock"),
            (Code::ContextMenu, "Menu"),
            (Code::NumLock, "Num_Lock"),
            (Code::PrintScreen, "Print"),
            (Code::ScrollLock, "Scroll_Lock"),
            (Code::IntlBackslash, "backslash"),
            (Code::IntlYen, "yen"),
            (Code::NumpadComma, "KP_Separator"),
            (Code::NumpadEqual, "KP_Equal"),
            (Code::KeyD, "D"),
            (Code::Digit1, "1"),
            (Code::F12, "F12"),
            (Code::Numpad5, "KP_5"),
            (Code::ArrowLeft, "Left"),
        ] {
            assert_eq!(
                portal_trigger(Shortcut::new(Some(Modifiers::CONTROL), key)).unwrap(),
                format!("CTRL+{expected}")
            );
        }
        assert!(
            portal_trigger(Shortcut::new(Some(Modifiers::CONTROL), Code::Unidentified)).is_err()
        );
    }
}
