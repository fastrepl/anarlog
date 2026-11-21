//! The marker the native app leaves in a channel's data folder ([`crate::Locations::base`]) the
//! first time it opens that channel's library, so a Tauri build that runs on the data
//! afterwards, and support, can tell the native app took it over:
//!
//! ```json
//! {"version":"1.6.0","at":"2026-11-21T09:30:00Z","bundle":"App"}
//! ```
//!
//! `version` is the native app's release, `at` when that launch started (RFC 3339, UTC), and
//! `bundle` its package type as `--print-package` reports it: `App` on macOS, `null` for a
//! Linux or Windows binary the bundler didn't patch. Written once and never rewritten; readers
//! ignore fields they don't know.

use std::path::Path;

pub const TAKEOVER_FILENAME: &str = "gpui-takeover.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Takeover {
    pub version: String,
    pub at: String,
    pub bundle: Option<String>,
}

/// Writes the marker unless the folder has one: it records the first takeover. Returns whether
/// it wrote.
pub fn record(base: &Path, takeover: &Takeover) -> std::io::Result<bool> {
    let path = base.join(TAKEOVER_FILENAME);
    if path.symlink_metadata().is_ok() {
        return Ok(false);
    }
    let content = serde_json::json!({
        "version": takeover.version,
        "at": takeover.at,
        "bundle": takeover.bundle,
    });
    anlg_storage::fs::atomic_write(&path, &content.to_string())?;
    Ok(true)
}

/// The marker in `base`, if the native app has taken it over.
pub fn read(base: &Path) -> std::io::Result<Option<Takeover>> {
    let content = match std::fs::read_to_string(base.join(TAKEOVER_FILENAME)) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let value: serde_json::Value = serde_json::from_str(&content)?;
    let text = |key: &str| value.get(key).and_then(serde_json::Value::as_str);
    let (Some(version), Some(at)) = (text("version"), text("at")) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "the takeover marker lacks its version or time",
        ));
    };
    Ok(Some(Takeover {
        version: version.into(),
        at: at.into(),
        bundle: text("bundle").map(Into::into),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A later launch, even of a newer version, must keep the first takeover's record.
    #[test]
    fn records_only_the_first_takeover() {
        let base = tempfile::tempdir().unwrap();
        assert_eq!(read(base.path()).unwrap(), None);
        let first = Takeover {
            version: "1.6.0".into(),
            at: "2026-11-21T09:30:00Z".into(),
            bundle: Some("App".into()),
        };

        assert!(record(base.path(), &first).unwrap());
        let written = std::fs::read(base.path().join(TAKEOVER_FILENAME)).unwrap();
        let later = Takeover {
            version: "1.7.0".into(),
            at: "2026-12-01T00:00:00Z".into(),
            bundle: None,
        };
        assert!(!record(base.path(), &later).unwrap());

        assert_eq!(
            std::fs::read(base.path().join(TAKEOVER_FILENAME)).unwrap(),
            written
        );
        assert_eq!(read(base.path()).unwrap(), Some(first));
    }
}
