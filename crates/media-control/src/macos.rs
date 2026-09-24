use std::ffi::CString;
use std::process::Command;

const MEDIA_REMOTE_PATH: &str =
    "/System/Library/PrivateFrameworks/MediaRemote.framework/MediaRemote";
const MEDIA_REMOTE_COMMAND_PAUSE: i64 = 1;

const SCRIPTABLE_PLAYERS: &[(&str, &str)] = &[
    ("Spotify", "player state is playing"),
    ("Music", "player state is playing"),
    ("VLC", "playing"),
];

pub(super) async fn pause_playback() -> Result<(), crate::Error> {
    send_media_remote_pause();
    for (app, playing_check) in SCRIPTABLE_PLAYERS {
        pause_scriptable_app(app, playing_check);
    }
    Ok(())
}

fn send_media_remote_pause() {
    unsafe {
        let path = match CString::new(MEDIA_REMOTE_PATH) {
            Ok(path) => path,
            Err(_) => return,
        };
        let handle = libc::dlopen(path.as_ptr(), libc::RTLD_LAZY);
        if handle.is_null() {
            tracing::warn!("media_remote_dlopen_failed");
            return;
        }
        let symbol = libc::dlsym(handle, c"MRMediaRemoteSendCommand".as_ptr());
        if symbol.is_null() {
            tracing::warn!("media_remote_dlsym_failed");
            libc::dlclose(handle);
            return;
        }
        let send: unsafe extern "C" fn(i64, *const std::ffi::c_void) -> bool =
            std::mem::transmute(symbol);
        send(MEDIA_REMOTE_COMMAND_PAUSE, std::ptr::null());
        libc::dlclose(handle);
    }
}

fn pause_scriptable_app(app: &str, playing_check: &str) {
    let script = format!(
        r#"if application "{app}" is running then tell application "{app}" to if {playing_check} then pause"#
    );
    match Command::new("osascript")
        .args(["-e", script.as_str()])
        .output()
    {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            tracing::warn!(
                app,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "applescript_media_pause_failed"
            );
        }
        Err(error) => {
            tracing::warn!(app, %error, "applescript_media_pause_failed");
        }
    }
}
