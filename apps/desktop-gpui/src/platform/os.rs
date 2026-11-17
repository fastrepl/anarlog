//! What reaches past GPUI to the OS: system file dialogs and alerts, files, folders and links
//! opened in other apps, permission prompts and completion sounds. The screens harness renders
//! without a window and calls [`go_headless`] first, so no capture can show a system dialog,
//! launch an app or play a sound, whatever it clicks. GPUI's own calls (`open_url`,
//! `prompt_for_paths`, `reveal_path`, ...) already go to its test platform there.

use std::{
    ffi::OsStr,
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};

static HEADLESS: AtomicBool = AtomicBool::new(false);

/// Turns every dialog, launch, prompt and sound above into a no-op for the rest of the process.
pub fn go_headless() {
    HEADLESS.store(true, Ordering::Relaxed);
}

pub fn headless() -> bool {
    HEADLESS.load(Ordering::Relaxed)
}

/// Opens a file, folder or URL in its default app.
pub fn open(target: impl AsRef<OsStr>) -> std::io::Result<()> {
    if headless() {
        return Ok(());
    }
    open::that(target)
}

/// Shows an `rfd` dialog; headless, it answers as if cancelled without showing anything.
pub fn dialog<T, F: Future<Output = Option<T>>>(
    show: impl FnOnce() -> F,
) -> impl Future<Output = Option<T>> {
    let shown = (!headless()).then(show);
    async move {
        match shown {
            Some(shown) => shown.await,
            None => None,
        }
    }
}
