#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub(crate) use macos::open_assisted;

#[cfg(not(target_os = "macos"))]
pub(crate) fn open_assisted(_permission: crate::Permission) -> crate::Result<bool> {
    Ok(false)
}

/// Dismiss the assisted drag overlay, if one is showing.
pub(crate) fn dismiss_current() {
    // The overlay is tracked in a main-thread-local, so dismissing from a
    // command thread would look at an empty slot and leave the panel up.
    #[cfg(target_os = "macos")]
    anlg_overlay_kit::macos::main_thread::run_on_main_thread(macos::dismiss_current);
}
