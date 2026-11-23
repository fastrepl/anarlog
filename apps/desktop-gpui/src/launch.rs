//! Launch arguments, and whose data this run uses.
//!
//! A sandbox run keeps an isolated profile directory. A channel run uses the shipping app's
//! data for the channel CD compiled in, so an update from the Tauri app keeps it. The channel
//! is compiled in rather than read at runtime so a stray environment variable can't point a
//! development build at real data.

use std::{ffi::OsString, path::PathBuf, sync::Arc};

use anlg_desktop_identity::{
    Channel, Locations,
    paths::STORE_FILENAME,
    store_json::StoreJson,
    webview_storage::{self, WebviewStorage},
};

use crate::platform::deeplinks::SCHEMES;

/// Set by CD for packaged builds; unset builds, including `cargo run` and tests, are sandboxed.
pub const COMPILED_CHANNEL: Option<&str> = option_env!("ANARLOG_CHANNEL");

const SANDBOX_DIRECTORY: &str = ".anarlog-gpui-sandbox";
const CRASH_REPORTER_SERVER_ARG: &str = "--crash-reporter-server";
const PRINT_PACKAGE_ARG: &str = "--print-package";
// `bundle.py verify` runs only a binary that holds the flag's literal bytes; a plain match arm
// compiles to integer compares in release builds and drops them.
#[used]
static PRINT_PACKAGE_ARG_BYTES: &str = PRINT_PACKAGE_ARG;

pub const HELP: &str = "Anarlog native preview\nUsage: desktop-gpui [--profile DIRECTORY]\nDefault: $HOME/.anarlog-gpui-sandbox/library.sqlite\nUse only an isolated profile or a backup copy, never the shipping app's live profile.";

#[derive(Debug, PartialEq)]
pub enum Launch {
    Help,
    /// Shipping re-executes itself with `--crash-reporter-server` to serve crash dumps; this
    /// app has no such helper, so that process exits without starting a second app.
    CrashReporter,
    /// `--print-package`: prints [`crate::platform::package::report`] for the bundle check.
    Package,
    Run {
        mode: Mode,
        urls: Vec<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// An isolated profile directory with this app's own layout.
    Sandbox(PathBuf),
    /// The shipping app's data for this channel, in place.
    Channel(Channel),
}

/// Where this run keeps its data, for the views and services.
#[derive(Clone, Debug)]
pub struct Storage {
    /// The shipping channel whose data this is; `None` for a sandbox profile.
    pub channel: Option<Channel>,
    /// Settings, models and this app's own files.
    pub root: PathBuf,
    /// Where a sandbox records that its profile moved. Channel data never moves.
    pub pointer: Option<PathBuf>,
}

impl Storage {
    /// The shipping channel's data in place.
    pub fn channel(channel: Channel, locations: &Locations) -> Self {
        Self {
            channel: Some(channel),
            root: locations.base.clone(),
            pointer: None,
        }
    }

    /// Notes, recordings and attachments. Shipping keeps them in its data folder itself; a
    /// sandbox profile keeps them in `vault`.
    pub fn vault(&self) -> PathBuf {
        match self.channel {
            Some(_) => self.root.clone(),
            None => self.root.join("vault"),
        }
    }

    /// The shipping app's `store.json`, which a channel run reads and writes in place.
    pub fn shipping_store(&self) -> Option<Arc<StoreJson>> {
        self.channel
            .map(|_| Arc::new(StoreJson::at(self.root.join(STORE_FILENAME))))
    }

    /// What the shipping app's bridge release mirrored from its web view's localStorage, which a
    /// channel run imports (`shipping_webview.rs`).
    pub fn shipping_webview(&self) -> Option<WebviewStorage> {
        self.channel
            .map(|_| WebviewStorage::at(self.root.join(webview_storage::FILENAME)))
    }
}

/// Accepts every argument the shipping app is launched with: the login item's `--background`,
/// the updater relaunch's `--onboarding` and `--updater2-*` flags, and deep-link URLs on
/// Windows and Linux. Unknown arguments are logged and ignored, so a relaunch never fails.
pub fn parse(
    args: impl IntoIterator<Item = OsString>,
    compiled_channel: Option<&str>,
    is_debug: bool,
    home: Option<PathBuf>,
) -> anyhow::Result<Launch> {
    let args: Vec<OsString> = args.into_iter().collect();
    if args.iter().any(|arg| {
        arg.to_str()
            .is_some_and(|arg| arg.starts_with(CRASH_REPORTER_SERVER_ARG))
    }) {
        return Ok(Launch::CrashReporter);
    }

    let mut profile = None;
    let mut urls = Vec::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let Some(text) = arg.to_str() else {
            tracing::warn!(argument = ?arg, "ignoring a launch argument that is not UTF-8");
            continue;
        };
        match text {
            "--help" => return Ok(Launch::Help),
            text if text == std::hint::black_box(PRINT_PACKAGE_ARG_BYTES) => {
                return Ok(Launch::Package);
            }
            "--profile" => {
                let directory = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--profile requires a directory"))?;
                profile = Some(PathBuf::from(directory));
            }
            // Shipping starts normally at login too; the flag only marks the login item.
            "--background" => {}
            "--onboarding" => {
                args.next();
            }
            text if text.starts_with("--onboarding=") || text.starts_with("--updater2-") => {}
            text if is_deep_link(text) => urls.push(text.to_owned()),
            text => tracing::warn!(argument = text, "ignoring an unknown launch argument"),
        }
    }

    let mode = match (profile, compiled_channel) {
        (Some(profile), _) => Mode::Sandbox(profile),
        (None, Some(name)) => {
            let channel = Channel::from_name(name)
                .ok_or_else(|| anyhow::anyhow!("This build names an unknown channel: {name}"))?;
            anyhow::ensure!(
                channel.matches_build(is_debug),
                "A {} build can't open the {name} channel's data. Pass --profile DIRECTORY.",
                if is_debug { "debug" } else { "release" },
            );
            Mode::Channel(channel)
        }
        (None, None) => Mode::Sandbox(
            home.ok_or_else(|| anyhow::anyhow!("No home directory; pass --profile DIRECTORY"))?
                .join(SANDBOX_DIRECTORY),
        ),
    };
    Ok(Launch::Run { mode, urls })
}

/// This launch's arguments for the process that replaces it. Its links were already handled,
/// so a restart drops them instead of opening them again.
pub fn relaunch_args(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    args.into_iter()
        .filter(|arg| !arg.to_str().is_some_and(is_deep_link))
        .collect()
}

fn is_deep_link(argument: &str) -> bool {
    argument
        .split_once(':')
        .is_some_and(|(scheme, _)| SCHEMES.contains(&scheme))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn run(values: &[&str], channel: Option<&str>, is_debug: bool) -> anyhow::Result<Launch> {
        parse(
            args(values),
            channel,
            is_debug,
            Some(PathBuf::from("/home/user")),
        )
    }

    fn sandbox(path: &str) -> Launch {
        Launch::Run {
            mode: Mode::Sandbox(PathBuf::from(path)),
            urls: vec![],
        }
    }

    #[test]
    fn unconfigured_builds_run_in_the_sandbox() {
        assert_eq!(
            run(&[], None, false).unwrap(),
            sandbox("/home/user/.anarlog-gpui-sandbox")
        );
        assert!(parse(args(&[]), None, true, None).is_err());
    }

    #[test]
    fn a_profile_always_runs_in_the_sandbox() {
        // Even over a compiled-in channel.
        assert_eq!(
            run(&["--profile", "/tmp/p"], Some("stable"), false).unwrap(),
            sandbox("/tmp/p")
        );
        assert!(run(&["--profile"], None, true).is_err());
    }

    #[test]
    fn channel_builds_use_their_channel_only_on_the_matching_build_kind() {
        let channel = |channel| Launch::Run {
            mode: Mode::Channel(channel),
            urls: vec![],
        };
        assert_eq!(
            run(&[], Some("stable"), false).unwrap(),
            channel(Channel::Stable)
        );
        assert_eq!(
            run(&[], Some("nightly"), false).unwrap(),
            channel(Channel::Nightly)
        );
        assert_eq!(
            run(&[], Some("staging"), true).unwrap(),
            channel(Channel::Staging)
        );
        assert_eq!(run(&[], Some("dev"), true).unwrap(), channel(Channel::Dev));

        for (name, is_debug) in [("stable", true), ("nightly", true), ("dev", false)] {
            assert!(run(&[], Some(name), is_debug).is_err(), "{name} {is_debug}");
        }
        assert!(run(&[], Some("beta"), false).is_err());
    }

    #[test]
    fn accepts_the_arguments_shipping_is_launched_with() {
        assert_eq!(
            run(
                &[
                    "--background",
                    "--onboarding",
                    "0",
                    "--onboarding=123",
                    "--updater2-skip-startup-migration=1",
                    "-psn_0_12345",
                    "--unknown",
                    "anarlog://auth/callback?code=1",
                    "C:\\Users\\me",
                    "mailto:me@example.com",
                ],
                Some("stable"),
                false,
            )
            .unwrap(),
            Launch::Run {
                mode: Mode::Channel(Channel::Stable),
                urls: vec!["anarlog://auth/callback?code=1".into()],
            }
        );
        assert_eq!(
            run(&["--background", "--help"], None, true).unwrap(),
            Launch::Help
        );
    }

    #[test]
    fn a_restart_does_not_open_the_launch_link_again() {
        assert_eq!(
            relaunch_args(args(&[
                "--profile",
                "/tmp/p",
                "anarlog://auth/callback?code=1",
                "--background",
            ])),
            args(&["--profile", "/tmp/p", "--background"])
        );
    }

    #[test]
    fn a_crash_reporter_relaunch_exits() {
        for argument in [
            "--crash-reporter-server",
            "--crash-reporter-server=/tmp/socket",
        ] {
            assert_eq!(
                run(&["--help", argument], Some("stable"), true).unwrap(),
                Launch::CrashReporter
            );
        }
    }

    #[test]
    fn print_package_flag_stays_in_binary_for_bundle_verification() {
        let flag = ["--print", "package"].join("-");
        assert_eq!(
            run(&[&flag], Some("staging"), false).unwrap(),
            Launch::Package
        );
        let binary = std::fs::read(std::env::current_exe().unwrap()).unwrap();
        assert!(
            binary
                .windows(flag.len())
                .any(|window| window == flag.as_bytes()),
            "compiled binary must retain {flag} bytes for bundle.py verify"
        );
    }

    /// Shipping's `global_base` (settings, models) and `vault_base` (notes, recordings,
    /// attachments) are both its settings base; nightly's differs from the database folder.
    #[test]
    fn channel_files_are_where_shipping_keeps_them() {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        for channel in [
            Channel::Dev,
            Channel::Staging,
            Channel::Nightly,
            Channel::Stable,
        ] {
            let is_debug = channel == Channel::Dev;
            let shipping = anlg_storage::global::compute_default_base_in(
                data.path(),
                channel.identifier(),
                is_debug,
            );
            let locations =
                Locations::resolve_in(data.path(), local.path(), channel.identifier(), is_debug);
            let storage = Storage::channel(channel, &locations);
            assert_eq!(storage.root, shipping, "{channel:?}");
            assert_eq!(storage.vault(), shipping, "{channel:?}");
            assert_eq!(storage.pointer, None);
        }
        let sandbox = Storage {
            channel: None,
            root: PathBuf::from("/profile"),
            pointer: None,
        };
        assert_eq!(sandbox.vault(), PathBuf::from("/profile/vault"));
    }
}
