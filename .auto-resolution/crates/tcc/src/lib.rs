use swift_rs::{Bool, Int, SRString, swift};

swift!(fn _audio_capture_permission_status() -> Int);
swift!(fn _screen_capture_permission_status() -> Int);
swift!(fn _request_screen_capture_permission() -> Bool);

swift!(fn _reset_audio_capture_permission(bundle_id: SRString) -> Bool);
swift!(fn _reset_screen_capture_permission(bundle_id: SRString) -> Bool);

swift!(fn _reset_microphone_permission(bundle_id: SRString) -> Bool);

pub const TCC_ERROR: isize = -1;
pub const NEVER_ASKED: isize = 2;
pub const DENIED: isize = 1;
pub const GRANTED: isize = 0;

pub fn audio_capture_permission_status() -> isize {
    unsafe { _audio_capture_permission_status() }
}

pub fn screen_capture_permission_status() -> isize {
    unsafe { _screen_capture_permission_status() }
}

pub fn request_screen_capture_permission() -> bool {
    unsafe { _request_screen_capture_permission() }
}

pub fn reset_audio_capture_permission(bundle_id: impl Into<SRString>) -> bool {
    unsafe { _reset_audio_capture_permission(bundle_id.into()) }
}

pub fn reset_screen_capture_permission(bundle_id: impl Into<SRString>) -> bool {
    unsafe { _reset_screen_capture_permission(bundle_id.into()) }
}

pub fn reset_microphone_permission(bundle_id: impl Into<SRString>) -> bool {
    unsafe { _reset_microphone_permission(bundle_id.into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_permission_granted() {
        let result = audio_capture_permission_status();
        assert!(result == NEVER_ASKED);
    }

    #[test]
    fn test_screen_capture_permission_status() {
        let result = screen_capture_permission_status();
        assert!(result == NEVER_ASKED || result == DENIED || result == GRANTED);
    }

    #[test]
    fn test_reset_audio_capture_permission() {
        let result = reset_audio_capture_permission("com.hyprnote.nightly");
        println!("reset_audio_capture_permission: {}", result);
    }

    #[test]
    fn test_reset_screen_capture_permission() {
        let result = reset_screen_capture_permission("com.hyprnote.nightly");
        println!("reset_screen_capture_permission: {}", result);
    }

    #[test]
    fn test_reset_microphone_permission() {
        let result = reset_microphone_permission("com.hyprnote.nightly");
        println!("reset_microphone_permission: {}", result);
    }
}
