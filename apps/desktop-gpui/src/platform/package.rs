//! What packaging decides about this binary: its version, the names the OS knows it by, the
//! package type and where the package keeps its files.
//!
//! CD bundles the binary with Tauri's bundler and the shipping app's configuration
//! (`packaging/bundle.py`), so each of these follows what the shipping Tauri app does.

use std::path::{Path, PathBuf};

use anlg_desktop_identity::Channel;

/// `APP_VERSION`, which CD sets for packaged builds as it does for the shipping app and its CLI,
/// else Cargo's. Cargo's stays fixed so `--locked` builds keep working.
pub const VERSION: &str = version(option_env!("APP_VERSION"), env!("CARGO_PKG_VERSION"));

const fn version(release: Option<&'static str>, cargo: &'static str) -> &'static str {
    match release {
        Some(release) if !release.is_empty() => release,
        _ => cargo,
    }
}

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

/// The tray's channel label, as shipping's `TrayVersion` derives it.
pub fn tray_channel(channel: Option<Channel>) -> &'static str {
    match channel {
        Some(Channel::Stable) => "stable",
        Some(Channel::Staging) => "staging",
        _ => "dev",
    }
}

/// What this build reports to `packaging/bundle.py verify` (`--print-package`), run from the
/// bundle: the channel and version it was built with, the package type the bundler patched in,
/// and where it reads its files there. Checked against the real bundle, not a layout copied
/// into a test.
pub fn report() -> serde_json::Value {
    let exe = std::env::current_exe().ok();
    let resources = exe.as_deref().and_then(Resources::installed);
    serde_json::json!({
        "channel": channel().map(Channel::name),
        "version": VERSION,
        "bundle_type": bundle_type().map(|kind| format!("{kind:?}")),
        "resources": resources.as_ref().map(|resources| &resources.dir),
        "executables": resources.as_ref().map(|resources| &resources.executables),
    })
}

/// The package type. Tauri's bundler rewrites the end of this marker in the copy of the
/// executable it puts in each Linux and Windows package, as it does for the shipping app's
/// `tauri::utils::platform::bundle_type`; the updater reads it to tell a deb from an AppImage.
#[used]
static mut BUNDLE_TYPE: &str = "__TAURI_BUNDLE_TYPE_VAR_UNK";

const BUNDLE_TYPE_PREFIX: &str = "__TAURI_BUNDLE_TYPE_VAR_";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleType {
    App,
    Deb,
    Rpm,
    AppImage,
    Nsis,
    Msi,
}

pub fn bundle_type() -> Option<BundleType> {
    // SAFETY: nothing writes the marker while the app runs. The volatile read keeps the compiler
    // from folding in the unpatched value.
    let marker = unsafe { std::ptr::read_volatile(&raw const BUNDLE_TYPE) };
    bundle_type_of(marker, cfg!(target_os = "macos"))
}

fn bundle_type_of(marker: &str, macos: bool) -> Option<BundleType> {
    match marker.strip_prefix(BUNDLE_TYPE_PREFIX) {
        Some("DEB") => Some(BundleType::Deb),
        Some("RPM") => Some(BundleType::Rpm),
        Some("APP") => Some(BundleType::AppImage),
        Some("NSS") => Some(BundleType::Nsis),
        Some("MSI") => Some(BundleType::Msi),
        // The bundler leaves macOS bundles unpatched.
        _ => macos.then_some(BundleType::App),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    pub const CURRENT: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else if cfg!(windows) {
        Self::Windows
    } else {
        Self::Linux
    };
}

/// Where the package keeps this app's files: the bundle's resources (`bundle.resources`) and the
/// executables shipped beside the app's own (`bundle.externalBin`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resources {
    dir: PathBuf,
    executables: PathBuf,
    os: Os,
    /// The shipping app's source folder, whose icons are not renamed yet.
    sources: bool,
}

impl Resources {
    /// `ANARLOG_NATIVE_RESOURCES` holds both kinds when set. Otherwise debug builds read the
    /// shipping app's sources, and release builds read their package.
    pub fn current() -> Option<Self> {
        if let Some(dir) = std::env::var_os("ANARLOG_NATIVE_RESOURCES") {
            let dir = PathBuf::from(dir);
            return Some(Self {
                executables: dir.clone(),
                dir,
                os: Os::CURRENT,
                sources: false,
            });
        }
        let exe = std::env::current_exe().ok()?;
        if cfg!(debug_assertions) {
            return Some(Self {
                dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("../desktop/src-tauri/resources"),
                executables: exe.parent()?.to_owned(),
                os: Os::CURRENT,
                sources: true,
            });
        }
        Self::installed(&exe)
    }

    /// Where the package this executable runs from keeps its files.
    pub fn installed(exe: &Path) -> Option<Self> {
        Self::packaged(
            Os::CURRENT,
            exe,
            channel().map(Channel::product_name),
            std::env::var_os("APPDIR").map(PathBuf::from),
        )
    }

    /// Tauri's `resource_dir`: `Contents/Resources` on macOS, the executable's folder on Windows,
    /// and on Linux `lib/<product>` beside the executable's `bin` (the deb's `/usr/lib/<product>`),
    /// else the AppImage's, else `/usr/lib/<product>`. Executables sit beside the app's own.
    pub fn packaged(
        os: Os,
        exe: &Path,
        product: Option<&str>,
        appdir: Option<PathBuf>,
    ) -> Option<Self> {
        let executables = exe.parent()?.to_owned();
        let dir = match os {
            Os::MacOs => executables.parent()?.join("Resources"),
            Os::Windows => executables.clone(),
            // Without a product, as in a build CD didn't channel, nothing names a package folder,
            // so resources sit with the executables, as Tauri's are in a cargo target folder.
            Os::Linux => match product {
                None => executables.clone(),
                Some(product) => {
                    let lib = Path::new("lib").join(product);
                    let beside = executables.parent()?.join(&lib);
                    if beside.is_dir() {
                        beside
                    } else {
                        appdir
                            .unwrap_or_else(|| PathBuf::from("/"))
                            .join("usr")
                            .join(lib)
                    }
                }
            },
        };
        Some(Self {
            dir,
            executables,
            os,
            sources: false,
        })
    }

    /// An app icon by its name, such as `staging-dark`.
    pub fn icon(&self, name: &str) -> PathBuf {
        if self.sources {
            self.dir.join(name).join("AppIcon.icns")
        } else {
            self.dir.join("icons").join(format!("{name}.icns"))
        }
    }

    pub fn notification_icons(&self) -> PathBuf {
        self.dir.join("notification-icons")
    }

    /// An executable shipped beside the app's own, such as `anarlog-cli`.
    pub fn executable(&self, name: &str) -> PathBuf {
        self.executables.join(if self.os == Os::Windows {
            format!("{name}.exe")
        } else {
            name.to_owned()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_release_version_comes_from_the_build_environment() {
        assert_eq!(version(Some("1.5.0"), "0.1.0"), "1.5.0");
        assert_eq!(version(Some(""), "0.1.0"), "0.1.0");
        assert_eq!(version(None, "0.1.0"), "0.1.0");
    }

    #[test]
    fn the_bundlers_marker_is_in_the_binary_once_and_reads_as_its_package() {
        // Built here so this test's own bytes don't add a second copy to the binary.
        let marker = format!("{BUNDLE_TYPE_PREFIX}UNK");
        let binary = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        let copies = regex::bytes::Regex::new(&regex::escape(&marker))
            .unwrap()
            .find_iter(&binary)
            .count();
        assert_eq!(copies, 1, "the bundler patches the first copy it finds");

        let patched = |kind: &str| bundle_type_of(&format!("{BUNDLE_TYPE_PREFIX}{kind}"), false);
        assert_eq!(patched("DEB"), Some(BundleType::Deb));
        assert_eq!(patched("APP"), Some(BundleType::AppImage));
        assert_eq!(patched("NSS"), Some(BundleType::Nsis));
        assert_eq!(patched("MSI"), Some(BundleType::Msi));
        assert_eq!(patched("UNK"), None);
        assert_eq!(bundle_type_of(&marker, true), Some(BundleType::App));
    }

    #[test]
    fn an_unchanneled_linux_release_finds_the_sidecars_beside_it() {
        let resources =
            Resources::packaged(Os::Linux, Path::new("/opt/gpui/desktop-gpui"), None, None)
                .unwrap();
        assert_eq!(
            resources.executable("llama-server"),
            Path::new("/opt/gpui/llama-server")
        );
    }
}
