//! tao's Linux `Window::theme()` behind `appWindow.theme()` /
//! `onThemeChanged`: the XDG Settings portal's `org.freedesktop.appearance`
//! `color-scheme` (`1` is dark, anything else light, light when the portal
//! is missing) and its `SettingChanged` signal. gpui reads the same portal
//! for `window.appearance()`, but on its own executor, where the workspace's
//! tokio-flavoured zbus panics, so the shell asks the portal itself on its
//! tokio runtime. Other platforms keep gpui's appearance.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The system colour scheme as the theme setting's `system` sees it.
#[derive(Clone, Default)]
pub struct SystemTheme {
    dark: Arc<AtomicBool>,
    changed: Arc<AtomicBool>,
}

impl gpui::Global for SystemTheme {}

impl SystemTheme {
    /// Reads the portal once and follows its changes; `dark` flips and
    /// [`take_changed`](Self::take_changed) reports each change.
    pub fn start(runtime: &tokio::runtime::Handle) -> Self {
        let theme = Self::default();
        #[cfg(target_os = "linux")]
        {
            let theme = theme.clone();
            runtime.spawn(async move {
                if let Err(error) = theme.follow_portal().await {
                    tracing::debug!(%error, "settings portal colour scheme unavailable");
                }
            });
        }
        #[cfg(not(target_os = "linux"))]
        let _ = runtime;
        theme
    }

    /// `isDarkTheme(await appWindow.theme())`, or gpui's appearance where
    /// the shell does not read the portal itself.
    pub fn dark(&self, window: &gpui::Window) -> bool {
        if cfg!(target_os = "linux") {
            let _ = window;
            self.dark.load(Ordering::SeqCst)
        } else {
            matches!(
                window.appearance(),
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark
            )
        }
    }

    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }

    fn set(&self, dark: bool) {
        if self.dark.swap(dark, Ordering::SeqCst) != dark {
            self.changed.store(true, Ordering::SeqCst);
        }
    }

    /// `portal::theme()` then `receive_theme_changed`.
    #[cfg(target_os = "linux")]
    async fn follow_portal(&self) -> anyhow::Result<()> {
        use futures_util::StreamExt as _;
        use zbus::zvariant::OwnedValue;

        const NAMESPACE: &str = "org.freedesktop.appearance";
        const KEY: &str = "color-scheme";

        let connection = zbus::Connection::session().await?;
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.portal.Settings")?
            .member("SettingChanged")?
            .build();
        let mut changes = zbus::MessageStream::for_match_rule(rule, &connection, None).await?;
        // tao subscribes whether or not the read works: a desktop without the
        // Settings portal is light until a portal says otherwise.
        let read: anyhow::Result<OwnedValue> = async {
            Ok(connection
                .call_method(
                    Some("org.freedesktop.portal.Desktop"),
                    "/org/freedesktop/portal/desktop",
                    Some("org.freedesktop.portal.Settings"),
                    "Read",
                    &(NAMESPACE, KEY),
                )
                .await?
                .body()
                .deserialize()?)
        }
        .await;
        match read {
            Ok(scheme) => {
                let dark = color_scheme_is_dark(&scheme);
                tracing::debug!(dark, "settings portal colour scheme");
                self.set(dark);
            }
            Err(error) => tracing::debug!(%error, "settings portal has no colour scheme"),
        }
        while let Some(message) = changes.next().await {
            let Ok(message) = message else { continue };
            let Ok((namespace, key, value)) =
                message.body().deserialize::<(String, String, OwnedValue)>()
            else {
                continue;
            };
            if namespace == NAMESPACE && key == KEY {
                let dark = color_scheme_is_dark(&value);
                tracing::debug!(dark, "settings portal colour scheme changed");
                self.set(dark);
            }
        }
        Ok(())
    }
}

/// `color_scheme_to_theme`: `1` is dark; the `Read` reply wraps the value in
/// a variant, `SettingChanged` carries it bare.
#[cfg(target_os = "linux")]
fn color_scheme_is_dark(value: &zbus::zvariant::OwnedValue) -> bool {
    use zbus::zvariant::Value;
    fn unwrap(value: &Value<'_>) -> Option<u32> {
        match value {
            Value::U32(scheme) => Some(*scheme),
            Value::Value(inner) => unwrap(inner),
            _ => None,
        }
    }
    unwrap(value) == Some(1)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use zbus::zvariant::{OwnedValue, Value};

    #[test]
    fn color_scheme_one_is_dark_through_nested_variants() {
        let dark = OwnedValue::try_from(Value::Value(Box::new(Value::U32(1)))).unwrap();
        let light = OwnedValue::try_from(Value::Value(Box::new(Value::U32(2)))).unwrap();
        let bare_dark = OwnedValue::try_from(Value::U32(1)).unwrap();
        let unrelated = OwnedValue::try_from(Value::Str("dark".into())).unwrap();
        assert!(color_scheme_is_dark(&dark));
        assert!(!color_scheme_is_dark(&light));
        assert!(color_scheme_is_dark(&bare_dark));
        assert!(!color_scheme_is_dark(&unrelated));
    }
}
