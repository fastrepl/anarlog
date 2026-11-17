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
#[cfg(not(target_os = "linux"))]
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

/// Linux portals use the Tokio-enabled zbus selected by the detect dependency.
#[cfg(target_os = "linux")]
pub fn portal_runtime() -> Result<&'static tokio::runtime::Runtime, &'static std::io::Error> {
    use std::sync::OnceLock;

    static PORTALS: OnceLock<std::io::Result<tokio::runtime::Runtime>> = OnceLock::new();
    PORTALS
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("native-portals")
                .enable_all()
                .build()
        })
        .as_ref()
}

/// Shows a Linux dialog from either GPUI executor without requiring a Tokio caller.
#[cfg(target_os = "linux")]
pub async fn dialog<T, F: Future<Output = Option<T>>>(show: impl FnOnce() -> F) -> Option<T> {
    if headless() {
        return None;
    }
    let runtime = match portal_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "could not start native dialog runtime");
            return None;
        }
    };
    // Enter for construction AND every poll; never hold an EnterGuard across an await
    // that can move to another thread.
    let mut shown = std::pin::pin!(async move { show().await });
    futures::future::poll_fn(|cx| {
        let _entered = runtime.enter();
        shown.as_mut().poll(cx)
    })
    .await
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn dialog_can_use_the_portal_reactor_outside_tokio() {
        let selected = futures::executor::block_on(super::dialog(|| {
            // Both constructing a portal request and resuming it can use Tokio.
            let request = tokio::spawn(async {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                "selected-file.txt"
            });
            async move {
                let selected = request.await.expect("portal request completes");
                Some(tokio::task::spawn_blocking(move || selected).await.unwrap())
            }
        }));
        assert_eq!(selected, Some("selected-file.txt"));
    }
}
