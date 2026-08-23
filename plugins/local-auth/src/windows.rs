use windows::Security::Credentials::UI::{
    UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
};
use windows::core::HSTRING;

use crate::Error;

pub fn available() -> Result<bool, Error> {
    let availability = UserConsentVerifier::CheckAvailabilityAsync()
        .map_err(win_error)?
        .join()
        .map_err(win_error)?;
    Ok(availability == UserConsentVerifierAvailability::Available)
}

pub fn authenticate(reason: &str) -> Result<bool, Error> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(Error::Platform(
            "authentication reason must not be empty".into(),
        ));
    }

    let result = UserConsentVerifier::RequestVerificationAsync(&HSTRING::from(reason))
        .map_err(win_error)?
        .join()
        .map_err(win_error)?;

    Ok(result == UserConsentVerificationResult::Verified)
}

fn win_error(error: windows::core::Error) -> Error {
    Error::Platform(error.to_string())
}
