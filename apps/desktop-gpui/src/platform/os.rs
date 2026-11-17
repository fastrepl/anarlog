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

/// GPUI's Linux reveal task runs on workers without the Tokio context zbus requires.
#[cfg(target_os = "linux")]
pub fn reveal_path(path: &std::path::Path) {
    if headless() {
        return;
    }
    let runtime = match portal_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "could not start native reveal runtime");
            return;
        }
    };
    let path = path.to_path_buf();
    drop(runtime.spawn(async move {
        let result = async {
            use std::os::fd::AsFd;

            let file = std::fs::File::open(&path)?;
            ashpd::desktop::open_uri::OpenDirectoryRequest::default()
                .send(&file.as_fd())
                .await?
                .response()?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            tracing::warn!(%error, "could not reveal file through desktop portal");
            let directory = if path.is_dir() {
                Some(path.as_path())
            } else {
                path.parent()
            };
            if let Some(directory) = directory
                && let Err(error) = open(directory)
            {
                tracing::error!(%error, "could not open containing folder");
            }
        }
    }));
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
