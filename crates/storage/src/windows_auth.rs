use std::path::{Path, PathBuf};

#[cfg(any(target_os = "windows", test))]
use base64::{Engine as _, engine::general_purpose::STANDARD};
#[cfg(any(target_os = "windows", test))]
use std::collections::HashMap;

const FILENAME: &str = "auth.dpapi";
#[cfg(any(target_os = "windows", test))]
const FORMAT_PREFIX: &str = "anarlog-auth-dpapi-v1:";

pub fn secure_path(plaintext_path: &Path) -> PathBuf {
    plaintext_path.with_file_name(FILENAME)
}

#[cfg(target_os = "windows")]
pub fn load(path: &Path) -> Result<HashMap<String, String>, crate::Error> {
    load_with(path, unprotect)
}

#[cfg(target_os = "windows")]
pub fn persist(path: &Path, auth: &HashMap<String, String>) -> Result<(), crate::Error> {
    persist_with(path, auth, protect, unprotect)
}

#[cfg(target_os = "windows")]
pub fn clear(path: &Path) -> Result<(), crate::Error> {
    if !path.is_file() {
        return Ok(());
    }

    let overwrite_result = persist(path, &HashMap::new());
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(remove_error) => {
            overwrite_result?;
            Err(remove_error.into())
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn persist_with(
    path: &Path,
    auth: &HashMap<String, String>,
    protect: impl Fn(&[u8]) -> Result<Vec<u8>, crate::Error>,
    unprotect: impl Fn(&[u8]) -> Result<Vec<u8>, crate::Error>,
) -> Result<(), crate::Error> {
    let plaintext = serde_json::to_vec(auth)?;
    let protected = protect(&plaintext)?;
    let encoded = format!("{FORMAT_PREFIX}{}", STANDARD.encode(protected));
    let previous = match std::fs::read_to_string(path) {
        Ok(previous) => Some(previous),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    crate::fs::atomic_write(path, &encoded)?;

    let verified = load_with(path, unprotect)
        .map(|persisted| persisted == *auth)
        .unwrap_or(false);
    if !verified {
        match previous {
            Some(previous) => crate::fs::atomic_write(path, &previous)?,
            None => match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            },
        }
        return Err(crate::Error::SecureStorage(
            "Windows encrypted auth verification failed".to_string(),
        ));
    }

    Ok(())
}

#[cfg(any(target_os = "windows", test))]
fn load_with(
    path: &Path,
    unprotect: impl Fn(&[u8]) -> Result<Vec<u8>, crate::Error>,
) -> Result<HashMap<String, String>, crate::Error> {
    let encoded = std::fs::read_to_string(path)?;
    let protected = encoded.strip_prefix(FORMAT_PREFIX).ok_or_else(|| {
        crate::Error::SecureStorage("unsupported Windows auth format".to_string())
    })?;
    let protected = STANDARD
        .decode(protected)
        .map_err(|_| crate::Error::SecureStorage("invalid Windows auth encoding".to_string()))?;
    let plaintext = unprotect(&protected)?;
    Ok(serde_json::from_slice(&plaintext)?)
}

#[cfg(target_os = "windows")]
fn protect(data: &[u8]) -> Result<Vec<u8>, crate::Error> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };
    use windows::core::PCWSTR;

    let input_len = u32::try_from(data.len()).map_err(|_| {
        crate::Error::SecureStorage("Windows auth payload is too large".to_string())
    })?;
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
fn unprotect(data: &[u8]) -> Result<Vec<u8>, crate::Error> {
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    let input_len = u32::try_from(data.len()).map_err(|_| {
        crate::Error::SecureStorage("Windows auth payload is too large".to_string())
    })?;
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
    crate::Error::SecureStorage(format!(
        "Windows data protection failed ({:#010X})",
        error.code().0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn xor(data: &[u8]) -> Result<Vec<u8>, crate::Error> {
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
        let secure_path = temp.path().join(FILENAME);
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            format!(r#"{{"access_token":"{}"}}"#, "token".repeat(2_000)),
        )]);

        persist_with(&secure_path, &auth, xor, xor).unwrap();

        assert_eq!(load_with(&secure_path, xor).unwrap(), auth);
        assert!(
            !std::fs::read_to_string(secure_path)
                .unwrap()
                .contains("access_token")
        );
    }

    #[test]
    fn failed_verification_does_not_leave_a_new_secure_file() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join(FILENAME);
        let auth = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"secret"}"#.to_string(),
        )]);

        let result = persist_with(&secure_path, &auth, xor, |_| {
            Err(crate::Error::SecureStorage("unavailable".to_string()))
        });

        assert!(result.is_err());
        assert!(!secure_path.exists());
    }

    #[test]
    fn failed_verification_restores_previous_secure_file() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join(FILENAME);
        let first = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"first"}"#.to_string(),
        )]);
        let second = HashMap::from([(
            "sb-project-auth-token".to_string(),
            r#"{"access_token":"second"}"#.to_string(),
        )]);
        persist_with(&secure_path, &first, xor, xor).unwrap();

        let result = persist_with(&secure_path, &second, xor, |_| {
            Err(crate::Error::SecureStorage("unavailable".to_string()))
        });

        assert!(result.is_err());
        assert_eq!(load_with(&secure_path, xor).unwrap(), first);
    }

    #[test]
    fn rejects_unversioned_encrypted_auth() {
        let temp = tempdir().unwrap();
        let secure_path = temp.path().join(FILENAME);
        std::fs::write(&secure_path, STANDARD.encode(b"ciphertext")).unwrap();

        assert!(load_with(&secure_path, xor).is_err());
    }
}
