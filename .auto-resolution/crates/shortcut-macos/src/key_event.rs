use crate::hotkey::Modifiers;

pub const KVK_ESCAPE: u16 = 0x35;
pub const KVK_FUNCTION: u16 = 0x3F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Option<u16>,
    pub modifiers: Modifiers,
}

impl KeyEvent {
    pub const fn new(key: Option<u16>, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub const fn empty() -> Self {
        Self {
            key: None,
            modifiers: Modifiers::empty(),
        }
    }

    pub fn is_escape(&self) -> bool {
        self.key == Some(KVK_ESCAPE)
    }
}
