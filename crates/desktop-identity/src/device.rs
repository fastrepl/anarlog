//! How the desktop app identifies this device to CloudSync (`x-device-fingerprint`,
//! `x-anarlog-device-name`). The fingerprint is machine-derived, so every channel and both
//! desktop apps on one machine present the same device.

pub use anlg_host::{fingerprint, fingerprint_from};

const MAX_DEVICE_NAME_BYTES: usize = 128;

/// The OS hostname as the device name, like shipping's `getDeviceIdentity`.
pub fn device_name() -> Option<String> {
    sanitize_device_name(&gethostname::gethostname().to_string_lossy())
}

/// Printable ASCII, trimmed and capped. Matches `sanitizeDeviceName` in
/// `apps/desktop/src/auth/cloudsync-credentials.ts`.
pub fn sanitize_device_name(name: &str) -> Option<String> {
    let ascii: String = name
        .chars()
        .filter(|char| (' '..='~').contains(char))
        .collect();
    let ascii = ascii.trim_matches(' ');
    if ascii.is_empty() {
        return None;
    }
    Some(ascii[..ascii.len().min(MAX_DEVICE_NAME_BYTES)].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DefaultHasher` is unspecified across Rust releases; a change would make every user a
    /// new sync device. The native app's workspace pins the same values on its toolchain.
    #[test]
    fn fingerprint_golden() {
        for (machine_uid, expected) in [
            ("", "30406ea523c53def"),
            ("00000000-0000-0000-0000-000000000000", "c104447f85eea341"),
            ("6F8E2A0C-3B1D-4E5F-9A7B-1C2D3E4F5A6B", "5681594ddfa3a7"),
            ("{6F8E2A0C-3B1D-4E5F-9A7B-1C2D3E4F5A6B}", "8136fff4c7357ff5"),
            ("0123456789abcdef0123456789abcdef", "b7fc4b7cafcbcea0"),
        ] {
            assert_eq!(fingerprint_from(machine_uid), expected, "{machine_uid:?}");
        }
        assert_eq!(fingerprint(), fingerprint());
    }

    #[test]
    fn device_names_match_the_shipping_sanitizer() {
        for (name, expected) in [
            ("Johns-MacBook-Pro.local", Some("Johns-MacBook-Pro.local")),
            ("  desk\ttop \n", Some("desktop")),
            ("Café 😀 Mac", Some("Caf  Mac")),
            ("\u{3000}studio\u{3000}", Some("studio")),
            ("", None),
            ("   ", None),
            ("日本語", None),
        ] {
            assert_eq!(sanitize_device_name(name).as_deref(), expected, "{name:?}");
        }
        assert_eq!(
            sanitize_device_name(&"a".repeat(200)),
            Some("a".repeat(128))
        );
        assert_eq!(
            sanitize_device_name(&format!(" {}x", "b".repeat(127))),
            Some(format!("{}x", "b".repeat(127)))
        );
    }
}
