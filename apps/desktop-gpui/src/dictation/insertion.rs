//! Inserting the transcript into the field that was focused when dictation started. On macOS
//! this is shipping's own Swift path (`crates/dictation-ui-macos`): the Accessibility selected
//! text, falling back to a clipboard-preserving Command-V.

use desktop_runtime::{Result, ServiceError};

const MAX_TEXT_BYTES: usize = 100_000;

pub trait Insertion: Send + Sync {
    /// Remembers the focused editable field and returns a token for it.
    fn capture(&self) -> Result<String>;
    fn insert(&self, target: &str, text: &str) -> Result<()>;
}

/// Shipping's `insert_text` command checks.
pub fn validate(text: &str) -> Result<()> {
    if text.contains('\0') {
        return Err(ServiceError::Failed(
            "Dictation text contains an unsupported NUL character".into(),
        ));
    }
    if text.is_empty() || text.len() > MAX_TEXT_BYTES {
        return Err(ServiceError::Failed(
            "Dictation text is empty or too long".into(),
        ));
    }
    Ok(())
}

pub struct NativeInsertion;

#[cfg(target_os = "macos")]
impl Insertion for NativeInsertion {
    fn capture(&self) -> Result<String> {
        anlg_dictation_ui_macos::capture_target()
            .map_err(|error| ServiceError::Failed(error.into()))
    }

    fn insert(&self, target: &str, text: &str) -> Result<()> {
        validate(text)?;
        anlg_dictation_ui_macos::insert_text(target, text)
            .map_err(|error| ServiceError::Failed(error.into()))
    }
}

// Shipping's AT-SPI and UI Automation insertion live inside its Tauri plugin, not a shared crate.
#[cfg(not(target_os = "macos"))]
impl Insertion for NativeInsertion {
    fn capture(&self) -> Result<String> {
        Err(ServiceError::Unsupported(
            "Dictation text insertion is not available on this platform yet.".into(),
        ))
    }

    fn insert(&self, _: &str, text: &str) -> Result<()> {
        validate(text)?;
        self.capture().map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_oversized_and_nul_text() {
        assert!(validate("Hello there.").is_ok());
        assert!(validate("").is_err());
        assert!(validate("a\0b").is_err());
        assert!(validate(&"a".repeat(MAX_TEXT_BYTES + 1)).is_err());
        assert!(validate(&"a".repeat(MAX_TEXT_BYTES)).is_ok());
    }
}
