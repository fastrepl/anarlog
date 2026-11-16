//! `settings/dictation-shortcut.tsx`: recording a dictation shortcut stores modifiers plus the
//! DOM `KeyboardEvent.code` ("Control+Alt+KeyD"), the format the shortcut plugin parses.

use std::str::FromStr;

use global_hotkey::hotkey::{Code, HotKey};
use gpui::Keystroke;

/// What a key press does while recording.
#[derive(Debug, PartialEq, Eq)]
pub enum Recorded {
    Cancel,
    Ignore,
    Shortcut(String),
}

pub fn record(keystroke: &Keystroke) -> Recorded {
    let key = keystroke.key.as_str();
    if matches!(key, "escape" | "tab") {
        return Recorded::Cancel;
    }
    let Some(code) = code(key) else {
        return Recorded::Ignore;
    };
    let modifiers = &keystroke.modifiers;
    let parts = [
        (modifiers.control, "Control"),
        (modifiers.alt, "Alt"),
        (modifiers.shift, "Shift"),
        (modifiers.platform, "Super"),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .map(|(_, name)| name.to_owned())
    .chain([code])
    .collect::<Vec<_>>();
    Recorded::Shortcut(parts.join("+"))
}

/// The DOM `code` for a GPUI key name, or `None` for modifiers and unmapped keys.
fn code(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return Some(format!("Key{}", c.to_ascii_uppercase()));
        }
        if c.is_ascii_digit() {
            return Some(format!("Digit{c}"));
        }
        let named = match c {
            '-' => "Minus",
            '=' => "Equal",
            '[' => "BracketLeft",
            ']' => "BracketRight",
            '\\' => "Backslash",
            ';' => "Semicolon",
            '\'' => "Quote",
            ',' => "Comma",
            '.' => "Period",
            '/' => "Slash",
            '`' => "Backquote",
            _ => return None,
        };
        return Some(named.into());
    }
    if let Some(number) = key.strip_prefix('f')
        && number.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n))
    {
        return Some(format!("F{number}"));
    }
    let named = match key {
        "space" => "Space",
        "enter" => "Enter",
        "backspace" => "Backspace",
        "delete" => "Delete",
        "home" => "Home",
        "end" => "End",
        "pageup" => "PageUp",
        "pagedown" => "PageDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "insert" => "Insert",
        _ => return None,
    };
    Some(named.into())
}

/// The shortcut plugin's `validate_shortcut`.
pub fn validate(shortcut: &str) -> Result<(), String> {
    if cfg!(target_os = "macos") && matches!(shortcut, "Fn" | "RightCommand") {
        return Ok(());
    }
    let hotkey = HotKey::from_str(shortcut)
        .map_err(|error| format!("Invalid dictation shortcut: {error}"))?;
    if hotkey.mods.is_empty() {
        return Err("Choose a shortcut with Control, Alt, Shift, or Command.".into());
    }
    if hotkey.key == Code::Escape {
        return Err("Escape is reserved for cancelling dictation. Choose another key.".into());
    }
    Ok(())
}

/// The key caps shown for a stored shortcut.
pub fn labels(shortcut: &str) -> Vec<String> {
    let mac = cfg!(target_os = "macos");
    shortcut
        .split('+')
        .map(|key| {
            match key {
                "Control" => {
                    if mac {
                        "⌃"
                    } else {
                        "Ctrl"
                    }
                }
                "Alt" => {
                    if mac {
                        "⌥"
                    } else {
                        "Alt"
                    }
                }
                "Shift" => {
                    if mac {
                        "⇧"
                    } else {
                        "Shift"
                    }
                }
                "Super" | "Meta" => {
                    if mac {
                        "⌘"
                    } else {
                        "Super"
                    }
                }
                "Fn" => "Fn / Globe",
                "RightCommand" => "Right Command",
                "ArrowLeft" => "←",
                "ArrowRight" => "→",
                "ArrowUp" => "↑",
                "ArrowDown" => "↓",
                key => {
                    return key
                        .strip_prefix("Key")
                        .or_else(|| key.strip_prefix("Digit"))
                        .unwrap_or(key)
                        .to_owned();
                }
            }
            .to_owned()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &'static str) -> Recorded {
        record(&Keystroke::parse(text).unwrap())
    }

    #[test]
    fn recorded_keys_use_dom_codes_and_validate_like_shipping() {
        assert_eq!(
            press("ctrl-alt-d"),
            Recorded::Shortcut("Control+Alt+KeyD".into())
        );
        assert_eq!(
            press("cmd-shift-space"),
            Recorded::Shortcut("Shift+Super+Space".into())
        );
        assert_eq!(press("escape"), Recorded::Cancel);
        assert_eq!(press("ctrl-escape"), Recorded::Cancel);
        let Recorded::Shortcut(plain) = press("d") else {
            panic!("a letter records");
        };
        assert!(validate(&plain).is_err());
        assert!(validate("Control+Alt+KeyD").is_ok());
        assert!(validate("Shift+Super+Space").is_ok());
        assert!(validate("not-a-shortcut").is_err());
        assert_eq!(labels("Control+Alt+KeyD")[2], "D");
    }
}
