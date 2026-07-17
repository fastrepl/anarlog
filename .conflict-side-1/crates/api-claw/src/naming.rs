use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const VM_NAME_PREFIX: &str = "claw";

/// Opaque user identifier. We accept any string so callers can plug in their
/// own id source (supabase uuid, stripe customer id, etc.). The VM name is
/// derived from a hash so the raw id never leaves our systems via the exe.dev
/// control plane.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for UserId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for UserId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Deterministic, DNS-safe VM name for a user: `claw-<first-12-hex-of-sha256>`.
///
/// Chosen so:
/// - it fits the `[a-z0-9-]` subdomain label rules for `<name>.exe.xyz`
/// - short enough to comfortably fit in DNS labels and logs
/// - collision probability is astronomically low for our user counts
/// - one-way: the user id can't be recovered from the name
pub fn vm_name(user: &UserId) -> String {
    let mut hasher = Sha256::new();
    hasher.update(user.as_str().as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{VM_NAME_PREFIX}-{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_across_calls() {
        let u = UserId::new("user-123");
        assert_eq!(vm_name(&u), vm_name(&u));
    }

    #[test]
    fn different_users_different_names() {
        let a = vm_name(&UserId::new("a"));
        let b = vm_name(&UserId::new("b"));
        assert_ne!(a, b);
    }

    #[test]
    fn dns_safe() {
        let name = vm_name(&UserId::new("User With Spaces & Weird!@#"));
        assert!(name.starts_with("claw-"));
        assert!(
            name.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "non-DNS chars in: {name}"
        );
        assert!(!name.starts_with('-') && !name.ends_with('-'));
        assert!(name.len() <= 63, "too long for DNS label: {name}");
    }

    #[test]
    fn prefix_matches_const() {
        let name = vm_name(&UserId::new("x"));
        assert!(name.starts_with(&format!("{VM_NAME_PREFIX}-")));
    }
}
