//! One GTK main loop for the process: the tray icon and the notification
//! windows both live on GLib's default main context, which a single thread
//! has to drive.

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::OnceLock;

    static RUNNING: OnceLock<bool> = OnceLock::new();

    /// Start the GTK main loop on its own thread the first time; `false` when
    /// GTK cannot initialise (no display).
    pub fn ensure_running() -> bool {
        *RUNNING.get_or_init(|| {
            let (ready, started) = std::sync::mpsc::channel::<bool>();
            let spawned = std::thread::Builder::new()
                .name("gtk".into())
                .spawn(move || {
                    let ok = gtk::init().is_ok();
                    let _ = ready.send(ok);
                    if ok {
                        gtk::main();
                    }
                });
            if spawned.is_err() {
                return false;
            }
            started.recv().unwrap_or(false)
        })
    }

    /// Run `f` on the GTK thread.
    pub fn invoke(f: impl FnOnce() + Send + 'static) {
        gtk::glib::MainContext::default().invoke(f);
    }
}

#[cfg(target_os = "linux")]
pub use linux::{ensure_running, invoke};
