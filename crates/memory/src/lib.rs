#![forbid(unsafe_code)]

mod error;
mod types;

pub use error::{Error, Result};
pub use types::MemoryId;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_id_rejects_empty_values() {
        assert_eq!(MemoryId::new(""), Err(Error::EmptyMemoryId));
        assert_eq!(MemoryId::new("   "), Err(Error::EmptyMemoryId));
    }

    #[test]
    fn memory_id_trims_and_preserves_value() {
        let id = MemoryId::new("  session-123  ").unwrap();

        assert_eq!(id.as_str(), "session-123");
        assert_eq!(id.to_string(), "session-123");
    }
}
