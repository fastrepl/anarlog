use std::collections::HashMap;
use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD};

const FORMAT_PREFIX: &str = "anarlog-auth-dpapi-v1:";

#[cfg(target_os = "windows")]
pub(crate) fn load_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> crate::Result<HashMap<String, String>> {
    let plaintext_path = crate::migrate::auth_path(app)?;
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;

    if secure_path.is_file() {
        let auth = match load_auth_file(&secure_path, unprotect) {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!(%error, "ignoring_unreadable_windows_auth");
                HashMap::new()
            }
        };
        crate::migrate::discard_windows_plaintext_auth(app);
        return Ok(auth);
    }

    if !plaintext_path.is_file() {
        return Ok(HashMap::new());
    }

    let auth = match read_plaintext_auth(&plaintext_path) {
        Ok(auth) => auth,
        Err(error) => {
            tracing::warn!(%error, "ignoring_unreadable_plaintext_auth");
            crate::migrate::discard_windows_plaintext_auth(app);
            return Ok(HashMap::new());
        }
    };

    if let Err(error) = persist_auth_file(&secure_path, &auth, protect, unprotect) {
        tracing::warn!(%error, "failed_to_migrate_auth_to_windows_data_protection");
        return Ok(auth);
    }

    crate::migrate::discard_windows_plaintext_auth(app);
    Ok(auth)
}

#[cfg(target_os = "windows")]
pub(crate) fn persist_auth<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    auth: &HashMap<String, String>,
) -> crate::Result<()> {
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;
    persist_auth_file(&secure_path, auth, protect, unprotect)?;
    crate::migrate::discard_windows_plaintext_auth(app);
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn clear_auth<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> crate::Result<()> {
    let secure_path = crate::migrate::windows_secure_auth_path(app)?;
    crate::migrate::discard_windows_plaintext_auth(app);

    if !secure_path.is_file() {
        return Ok(());
    }

    let overwrite_result = persist_auth_file(&secure_path, &HashMap::new(), protect, unprotect);
    match std::fs::remove_file(&secure_path) {
        Ok(()) => Ok(()),
        Err(remove_error) => {
            overwrite_result?;
            Err(remove_error.into())
        }
    }
}

fn read_plaintext_auth(path: &Path) -> crate::Result<HashMap<String, String>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn persist_auth_file(
    path: &Path,
    auth: &HashMap<String, String>,
    protect: impl Fn(&[u8]) -> crate::Result<Vec<u8>>,
    unprotect: impl Fn(&[u8]) -> crate::Result<Vec<u8>>,
) -> crate::Result<()> {
    let plaintext = serde_json::to_vec(auth)?;
    let protected = protect(&plaintext)?;
    let encoded = format!("{FORMAT_PREFIX}{}", STANDARD.encode(protected));
    let previous = match std::fs::read_to_string(path) {
        Ok(previous) => Some(previous),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    hypr_storage::fs::atomic_write(path, &encoded)?;

    let verified = load_auth_file(path, unprotect)
        .map(|persisted| persisted == *auth)
        .unwrap_or(false);
    if !verified {
        match previous {
            Some(previous) => hypr_storage::fs::atomic_write(path, &previous)?,
            None => match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            },
        }
        return Err(crate::Error::Storage(
            "Windows encrypted auth verification failed".to_string(),
        ));
    }

    Ok(())
}

fn load_auth_file(
    path: &Path,
    unprotect: impl Fn(&[u8]) -> crate::Result<Vec<u8>>,
) -> crate::Result<HashMap<String, String>> {
    let encoded = std::fs::read_to_string(path)?;
    let protected = encoded
        .strip_prefix(FORMAT_PREFIX)
        .ok_or_else(|| crate::Error::Storage("unsupported Windows auth format".to_string()))?;
    let protected = STANDARD
        .decode(protected)
        .map_err(|_| crate::Error::Storage("invalid Windows auth encoding".to_string()))?;
    let plaintext = unprotect(&protected)?;
    Ok(serde_json::from_slice(&plaintext)?)
}

#[cfg(target_os = "windows")]
fn protect(data: &[u8]) -> crate::Result<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };
    use windows::core::PCWSTR;

    let input_len = u32::try_from(data.len())
        .map_err(|_| crate::Error::Storage("Windows auth payload is too large".to_string()))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(data_protection_error)?;
    }

    Ok(copy_and_free(output, false))
}

#[cfg(target_os = "windows")]
fn unprotect(data: &[u8]) -> crate::Result<Vec<u8>> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    let input_len = u32::try_from(data.len())
        .map_err(|_| crate::Error::Storage("Windows auth payload is too large".to_string()))?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: input_len,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(data_protection_error)?;
    }

    Ok(copy_and_free(output, true))
}

#[cfg(target_os = "windows")]
fn copy_and_free(
    output: windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB,
    clear_source: bool,
) -> Vec<u8> {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};

    if output.pbData.is_null() || output.cbData == 0 {
        if !output.pbData.is_null() {
            unsafe {
                LocalFree(Some(HLOCAL(output.pbData.cast())));
            }
        }
        return Vec::new();
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    if clear_source {
        unsafe {
            std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        }
    }
    unsafe {
        LocalFree(Some(HLOCAL(output.pbData.cast())));
    }
    bytes
}

#[cfg(target_os = "windows")]
fn data_protection_error(error: windows::core::Error) -> crate::Error {
    crate::Error::Storage(format!(
        "Windows data protection failed ({:#010X})",
        error.code().0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn xor(data: &[u8]) -> crate::Result<Vec<u8>> {
        Ok(data.iter().map(|byte| byte ^ 0xa5).collect())
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dpapi_protects_and_round_trips_for_current_user() {
        let plaintext = b"anarlog-windows-dpapi-round-trip";

        let protected = protect(plaintext).unwrap();

        assert_ne!(protected, plaintext);
        assert_eq!(unprotect(&protected).unwrap(), plaintext);
    }

    #[test]
    fn encrypted_auth_round_trips_sessions_larger_than_credential_manager_limit() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join("auth.dpapi");
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            format!(r#"{{"access_token":"{}"}}"#, "token".repeat(2_000)),
        )]);

        persist_auth_file(&secure_path, &auth, xor, xor).unwrap();

        assert_eq!(load_auth_file(&secure_path, xor).unwrap(), auth);
        assert!(
            !std::fs::read_to_string(secure_path)
                .unwrap()
                .contains("access_token")
        );
    }

    #[test]
    fn plaintext_migration_writes_verified_ciphertext_before_removal() {
        let temp = tempdir().unwrap();
        let plaintext_path = temp.path().join("auth.json");
        let secure_path = temp.path().join("auth.dpapi");
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"secret"}"#.to_string(),
        )]);
        std::fs::write(&plaintext_path, serde_json::to_string(&auth).unwrap()).unwrap();

        let loaded = read_plaintext_auth(&plaintext_path).unwrap();
        persist_auth_file(&secure_path, &loaded, xor, xor).unwrap();
        crate::migrate::discard_plaintext_auth(&plaintext_path);

        assert!(!plaintext_path.exists());
        assert_eq!(load_auth_file(&secure_path, xor).unwrap(), auth);
    }

    #[test]
    fn failed_secure_write_leaves_plaintext_available_for_retry() {
        let temp = tempdir().unwrap();
        let plaintext_path = temp.path().join("auth.json");
        let secure_path = temp.path().join("missing-parent").join("auth.dpapi");
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"secret"}"#.to_string(),
        )]);
        std::fs::write(&plaintext_path, serde_json::to_string(&auth).unwrap()).unwrap();

        let result = persist_auth_file(
            &secure_path,
            &auth,
            |_| Err(crate::Error::Storage("unavailable".to_string())),
            xor,
        );

        assert!(result.is_err());
        assert!(plaintext_path.is_file());
    }

    #[test]
    fn failed_verification_does_not_leave_a_new_secure_file() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join("auth.dpapi");
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"secret"}"#.to_string(),
        )]);

        let result = persist_auth_file(&secure_path, &auth, xor, |_| {
            Err(crate::Error::Storage("unavailable".to_string()))
        });

        assert!(result.is_err());
        assert!(!secure_path.exists());
    }

    #[test]
    fn failed_verification_restores_previous_secure_file() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join("auth.dpapi");
        let first = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"first"}"#.to_string(),
        )]);
        let second = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"second"}"#.to_string(),
        )]);
        persist_auth_file(&secure_path, &first, xor, xor).unwrap();

        let result = persist_auth_file(&secure_path, &second, xor, |_| {
            Err(crate::Error::Storage("unavailable".to_string()))
        });

        assert!(result.is_err());
        assert_eq!(load_auth_file(&secure_path, xor).unwrap(), first);
    }

    #[test]
    fn rejects_unversioned_encrypted_auth() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join("auth.dpapi");
        std::fs::write(&secure_path, STANDARD.encode(b"ciphertext")).unwrap();

        assert!(load_auth_file(&secure_path, xor).is_err());
    }
}
