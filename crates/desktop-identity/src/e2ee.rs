//! Secure-store keys of an account's E2EE secrets, all in the [`SECRET_SCOPE`] scope.

pub const SECRET_SCOPE: &str = "e2ee";

pub fn canonical_account_user_id(account_user_id: &str) -> Result<String, String> {
    uuid::Uuid::parse_str(account_user_id.trim())
        .map(|account_user_id| account_user_id.to_string())
        .map_err(|_| "E2EE account ID is invalid".to_string())
}

/// The recovery key this device uses for the account.
pub fn recovery_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = canonical_account_user_id(account_user_id)?;
    Ok(format!("account:{account_user_id}:recovery-v1"))
}

/// A recovery key created but not yet claimed on the server.
pub fn pending_recovery_key_name(account_user_id: &str) -> Result<String, String> {
    Ok(format!("{}:pending", recovery_key_name(account_user_id)?))
}

/// This device's key for receiving the recovery key from another device.
pub fn device_key_name(account_user_id: &str) -> Result<String, String> {
    let account_user_id = canonical_account_user_id(account_user_id)?;
    Ok(format!("account:{account_user_id}:device-enrollment-v1"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_ids_are_canonicalized_before_use() {
        assert_eq!(
            canonical_account_user_id(" 550E8400-E29B-41D4-A716-446655440000 ").unwrap(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert!(canonical_account_user_id("user-a").is_err());
    }

    #[test]
    fn e2ee_keychain_accounts_golden() {
        let user = "550E8400-E29B-41D4-A716-446655440000";
        let account = |identifier, key: String| {
            crate::secure_store::account(identifier, SECRET_SCOPE, &key)
        };

        for (identifier, prefix) in [
            ("com.hyprnote.stable", ""),
            ("com.hyprnote.nightly", ""),
            ("com.hyprnote.staging", ""),
            ("com.hyprnote.dev", "v2:"),
        ] {
            assert_eq!(
                account(identifier, recovery_key_name(user).unwrap()),
                format!("{prefix}e2ee:account:550e8400-e29b-41d4-a716-446655440000:recovery-v1")
            );
            assert_eq!(
                account(identifier, pending_recovery_key_name(user).unwrap()),
                format!(
                    "{prefix}e2ee:account:550e8400-e29b-41d4-a716-446655440000:recovery-v1:pending"
                )
            );
            assert_eq!(
                account(identifier, device_key_name(user).unwrap()),
                format!(
                    "{prefix}e2ee:account:550e8400-e29b-41d4-a716-446655440000:device-enrollment-v1"
                )
            );
        }
    }
}
