//! Native side effects of settings toggles, like shipping's `applySettingSideEffects` in
//! `settings/queries.ts`: applied after each write and once at startup from stored values.
//! The host installs [`NativeSettings`]; without it (tests, headless captures) nothing
//! touches the system.

use std::{path::PathBuf, sync::Arc};

use desktop_runtime::{Result, ServiceError};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// `tauri-plugin-autostart` with a LaunchAgent.
    LaunchAtLogin(bool),
    /// `windows::set_show_app_in_dock`: the macOS activation policy.
    ShowInDock(bool),
    /// `tray::set_tray_icon_visible`.
    ShowTrayIcon(bool),
}

/// Settings whose stored values are re-applied at startup.
pub const KEYS: [&str; 3] = ["autostart", "show_app_in_dock", "show_tray_icon"];

/// The side effect a stored value has on this platform, if any.
pub fn effect(key: &str, value: &Value) -> Option<Effect> {
    let on = value.as_bool()?;
    match key {
        "autostart" => Some(Effect::LaunchAtLogin(on)),
        "show_app_in_dock" if cfg!(target_os = "macos") => Some(Effect::ShowInDock(on)),
        "show_tray_icon" => Some(Effect::ShowTrayIcon(on)),
        _ => None,
    }
}

pub trait SystemSettings: Send + Sync {
    fn apply(&self, effect: Effect) -> Result<()>;
}

#[derive(Clone)]
pub struct Installed(pub Arc<dyn SystemSettings>);

impl gpui::Global for Installed {}

/// Applies `key`'s side effect through the installed adapter.
pub fn apply(key: &str, value: &Value, cx: &gpui::App) -> Result<()> {
    match (effect(key, value), cx.try_global::<Installed>()) {
        (Some(effect), Some(adapter)) => adapter.0.apply(effect),
        _ => Ok(()),
    }
}

pub struct NativeSettings {
    pub login_item: LoginItem,
}

impl NativeSettings {
    pub fn current() -> Result<Self> {
        Ok(Self {
            login_item: LoginItem::current()?,
        })
    }
}

impl SystemSettings for NativeSettings {
    fn apply(&self, effect: Effect) -> Result<()> {
        match effect {
            Effect::LaunchAtLogin(on) => self.login_item.set(on),
            Effect::ShowInDock(on) => set_show_in_dock(on),
            Effect::ShowTrayIcon(on) => {
                super::tray::set_visible(on);
                Ok(())
            }
        }
    }
}

/// A per-user login entry: a LaunchAgent on macOS, an XDG autostart entry on Linux.
/// Named apart from the shipping app's entry so the preview never replaces it.
pub struct LoginItem {
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub directory: PathBuf,
}

impl LoginItem {
    /// Relaunches this executable with the arguments it was started with (its profile).
    pub fn current() -> Result<Self> {
        let program = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(failure)?;
        let home = std::env::home_dir().ok_or_else(|| failure("No home directory"))?;
        let directory = if cfg!(target_os = "macos") {
            home.join("Library/LaunchAgents")
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| home.join(".config"))
                .join("autostart")
        };
        Ok(Self {
            name: "Anarlog Native Preview".into(),
            program,
            args: std::env::args().skip(1).collect(),
            directory,
        })
    }

    pub fn path(&self) -> PathBuf {
        let extension = if cfg!(target_os = "macos") {
            "plist"
        } else {
            "desktop"
        };
        self.directory.join(format!("{}.{extension}", self.name))
    }

    pub fn contents(&self) -> String {
        let command = std::iter::once(self.program.to_string_lossy().into_owned())
            .chain(self.args.iter().cloned());
        if cfg!(target_os = "macos") {
            let arguments = command
                .map(|argument| format!("<string>{}</string>", xml_escape(&argument)))
                .collect::<String>();
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n<dict>\n\
                 <key>Label</key>\n<string>{}</string>\n\
                 <key>ProgramArguments</key>\n<array>{arguments}</array>\n\
                 <key>RunAtLoad</key>\n<true/>\n\
                 </dict>\n</plist>\n",
                xml_escape(&self.name)
            )
        } else {
            let exec = command
                .map(|argument| desktop_quote(&argument))
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "[Desktop Entry]\nType=Application\nVersion=1.0\nName={}\nExec={exec}\nStartupNotify=false\nTerminal=false\n",
                self.name
            )
        }
    }

    pub fn set(&self, enabled: bool) -> Result<()> {
        if cfg!(target_os = "windows") {
            return Err(ServiceError::Unsupported(
                "Start at login is not available on Windows yet.".into(),
            ));
        }
        let path = self.path();
        if enabled {
            std::fs::create_dir_all(&self.directory).map_err(failure)?;
            std::fs::write(&path, self.contents()).map_err(failure)
        } else {
            match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(failure(error)),
                _ => Ok(()),
            }
        }
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Desktop Entry `Exec` quoting: arguments with reserved characters are double-quoted.
fn desktop_quote(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/-_.=:,+@".contains(c))
    {
        return argument.to_owned();
    }
    let escaped = argument
        .replace('\\', "\\\\\\\\")
        .replace('"', "\\\\\"")
        .replace('`', "\\\\`")
        .replace('$', "\\\\$");
    format!("\"{escaped}\"")
}

#[cfg(target_os = "macos")]
fn set_show_in_dock(show: bool) -> Result<()> {
    use objc2::{class, msg_send, runtime::AnyObject};
    // NSApplicationActivationPolicyRegular and NSApplicationActivationPolicyAccessory.
    let policy: isize = if show { 0 } else { 1 };
    let applied = unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let applied: bool = msg_send![app, setActivationPolicy: policy];
        applied
    };
    if applied {
        Ok(())
    } else {
        Err(failure("macOS did not change the Dock visibility."))
    }
}

#[cfg(not(target_os = "macos"))]
fn set_show_in_dock(_: bool) -> Result<()> {
    Ok(())
}

fn failure(error: impl ToString) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_boolean_toggles_have_side_effects() {
        assert_eq!(
            effect("autostart", &json!(true)),
            Some(Effect::LaunchAtLogin(true))
        );
        assert_eq!(
            effect("show_tray_icon", &json!(false)),
            Some(Effect::ShowTrayIcon(false))
        );
        assert_eq!(
            effect("show_app_in_dock", &json!(false)),
            cfg!(target_os = "macos").then_some(Effect::ShowInDock(false))
        );
        assert_eq!(effect("autostart", &json!("yes")), None);
        assert_eq!(effect("use_24_hour_time", &json!(true)), None);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn login_item_is_written_and_removed_in_its_directory() {
        let directory = tempfile::tempdir().unwrap();
        let item = LoginItem {
            name: "Anarlog Native Preview".into(),
            program: "/Applications/A & B.app/Contents/MacOS/desktop-gpui".into(),
            args: vec!["--profile".into(), "/tmp/my profile".into()],
            directory: directory.path().join("agents"),
        };
        item.set(true).unwrap();
        let written = std::fs::read_to_string(item.path()).unwrap();
        if cfg!(target_os = "macos") {
            assert!(written.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/desktop-gpui</string><string>--profile</string><string>/tmp/my profile</string>"));
            assert!(written.contains("<key>RunAtLoad</key>\n<true/>"));
        } else {
            assert!(written.contains("Exec=\"/Applications/A & B.app/Contents/MacOS/desktop-gpui\" --profile \"/tmp/my profile\""));
        }
        item.set(false).unwrap();
        assert!(!item.path().exists());
        item.set(false).unwrap();
    }
}
