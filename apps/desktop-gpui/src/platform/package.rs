//! What packaging decides about this binary: the names the OS knows it by.
//!
//! CD bundles the binary with Tauri's bundler and the shipping app's configuration
//! (`packaging/bundle.py`), so these follow what the shipping Tauri app does.

use anlg_desktop_identity::Channel;

/// The channel CD compiled in, which names the package this binary ships in. It decides the
/// package's names even for a `--profile` run, whose data alone is sandboxed.
pub fn channel() -> Option<Channel> {
    crate::launch::COMPILED_CHANNEL.and_then(Channel::from_name)
}

const SANDBOX_ID: &str = "com.anarlog.gpui.sandbox";

/// The names the OS knows this app by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppIds {
    /// Windows' AppUserModelID. Tauri's NSIS installer gives the Start-menu shortcut the
    /// channel's identifier, which taskbar grouping and toasts go by.
    pub app_user_model_id: &'static str,
    /// The Linux window `app_id` and X11 `WM_CLASS`. The package's desktop entry names the
    /// executable as `StartupWMClass`, which is also the class GTK gives shipping's windows.
    pub window_class: &'static str,
    /// The app name notifications show.
    pub name: &'static str,
}

impl AppIds {
    pub fn current() -> Self {
        Self::of(channel())
    }

    pub fn of(channel: Option<Channel>) -> Self {
        match channel {
            Some(channel) => Self {
                app_user_model_id: channel.identifier(),
                window_class: channel.executable(),
                name: channel.product_name(),
            },
            None => Self {
                app_user_model_id: SANDBOX_ID,
                window_class: SANDBOX_ID,
                name: "Anarlog GPUI",
            },
        }
    }
}

/// The `app_id` every window of this app opens with.
pub fn window_app_id() -> Option<String> {
    Some(AppIds::current().window_class.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_packages_use_the_shipping_app_ids() {
        let ids = |channel| {
            let ids = AppIds::of(channel);
            (ids.app_user_model_id, ids.window_class, ids.name)
        };
        assert_eq!(
            ids(Some(Channel::Stable)),
            ("com.hyprnote.stable", "anarlog", "Anarlog")
        );
        assert_eq!(
            ids(Some(Channel::Staging)),
            ("com.hyprnote.staging", "anarlog-staging", "Anarlog Staging")
        );
        assert_eq!(
            ids(Some(Channel::Nightly)),
            ("com.hyprnote.nightly", "anarlog-nightly", "Anarlog Nightly")
        );
        assert_eq!(
            ids(None),
            (SANDBOX_ID, SANDBOX_ID, "Anarlog GPUI"),
            "an unpackaged build keeps its own names"
        );
    }
}
