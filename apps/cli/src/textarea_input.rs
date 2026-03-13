use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{Input, Key};

pub fn textarea_input_from_key_event(key_event: KeyEvent, include_enter: bool) -> Option<Input> {
    let key = match key_event.code {
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter if include_enter => Key::Enter,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Tab => Key::Tab,
        KeyCode::Delete => Key::Delete,
        KeyCode::Char(c) => Key::Char(c),
        _ => return None,
    };

    Some(Input {
        key,
        ctrl: key_event.modifiers.contains(KeyModifiers::CONTROL),
        alt: key_event.modifiers.contains(KeyModifiers::ALT),
        shift: key_event.modifiers.contains(KeyModifiers::SHIFT),
    })
}
