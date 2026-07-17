use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Command,
    Option,
    Shift,
    Control,
    Fn,
}

impl Modifier {
    const fn bit(self) -> u8 {
        match self {
            Self::Command => 1 << 0,
            Self::Option => 1 << 1,
            Self::Shift => 1 << 2,
            Self::Control => 1 << 3,
            Self::Fn => 1 << 4,
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
)]
#[serde(transparent)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_raw(bits: u8) -> Self {
        Self(bits)
    }

    pub const fn raw(self) -> u8 {
        self.0
    }

    pub fn from_slice(mods: &[Modifier]) -> Self {
        let mut bits = 0u8;
        let mut i = 0;
        while i < mods.len() {
            bits |= mods[i].bit();
            i += 1;
        }
        Self(bits)
    }

    pub fn contains(self, m: Modifier) -> bool {
        self.0 & m.bit() != 0
    }

    pub fn insert(&mut self, m: Modifier) {
        self.0 |= m.bit();
    }

    pub fn remove(&mut self, m: Modifier) {
        self.0 &= !m.bit();
    }

    pub fn removing(self, m: Modifier) -> Self {
        Self(self.0 & !m.bit())
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn is_subset_of(self, other: Self) -> bool {
        self.0 & !other.0 == 0
    }

    pub fn matches_exactly(self, other: Self) -> bool {
        self.0 == other.0
    }
}

impl From<&[Modifier]> for Modifiers {
    fn from(mods: &[Modifier]) -> Self {
        Self::from_slice(mods)
    }
}

impl<const N: usize> From<[Modifier; N]> for Modifiers {
    fn from(mods: [Modifier; N]) -> Self {
        Self::from_slice(&mods)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub struct HotKey {
    pub key: Option<u16>,
    pub modifiers: Modifiers,
}

impl HotKey {
    pub const fn new(key: Option<u16>, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub const fn modifier_only(modifiers: Modifiers) -> Self {
        Self {
            key: None,
            modifiers,
        }
    }

    pub fn is_modifier_only(&self) -> bool {
        self.key.is_none()
    }
}
