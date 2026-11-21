//! The update feed a packaged build reads: shipping's `plugins.updater` for the build's channel,
//! taken from the same `tauri.conf` files `desktop_cd.yaml` and `packaging/bundle.py` merge, with
//! the URL tokens filled in as `tauri-plugin-updater` fills them.

use anlg_desktop_identity::Channel;
use serde_json::Value;

use crate::platform::package::{BundleType, Os};

const BASE: &str = include_str!("../../../../desktop/src-tauri/tauri.conf.json");
const STAGING: &str = include_str!("../../../../desktop/src-tauri/tauri.conf.staging.json");
const NIGHTLY: &str = include_str!("../../../../desktop/src-tauri/tauri.conf.nightly.json");
const NIGHTLY_MACOS: &str =
    include_str!("../../../../desktop/src-tauri/tauri.conf.nightly-macos.json");
const STABLE: &str = include_str!("../../../../desktop/src-tauri/tauri.conf.stable.json");
const STABLE_MACOS: &str =
    include_str!("../../../../desktop/src-tauri/tauri.conf.stable-macos.json");

/// Rehearsals only: another feed for this build's signed updates. The signing key stays the
/// compiled one, so only artifacts signed for shipping install.
pub const ENDPOINT_OVERRIDE: &str = "ANARLOG_NATIVE_UPDATE_URL";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    /// With `{{current_version}}` left for the update checker.
    pub endpoints: Vec<String>,
    /// The release's platform key, as `desktop-release-platforms.json` names it.
    pub target: String,
    pub pubkey: String,
}

/// Tauri's `Installer::name`, which shipping's endpoints carry as `{{bundle_type}}`.
fn installer(bundle: BundleType) -> &'static str {
    match bundle {
        BundleType::App => "app",
        BundleType::AppImage => "appimage",
        BundleType::Deb => "deb",
        BundleType::Rpm => "rpm",
        BundleType::Nsis => "nsis",
        BundleType::Msi => "msi",
    }
}

fn os_name(os: Os) -> &'static str {
    match os {
        Os::MacOs => "darwin",
        Os::Linux => "linux",
        Os::Windows => "windows",
    }
}

/// The `--config` files CD merges over `tauri.conf.json` for the channel and OS.
fn configs(channel: Channel, os: Os) -> &'static [&'static str] {
    match (channel, os) {
        (Channel::Dev, _) => &[],
        (Channel::Staging, _) => &[STAGING],
        (Channel::Nightly, Os::MacOs) => &[NIGHTLY, NIGHTLY_MACOS],
        (Channel::Nightly, _) => &[NIGHTLY],
        (Channel::Stable, Os::MacOs) => &[STABLE, STABLE_MACOS],
        (Channel::Stable, _) => &[STABLE],
    }
}

/// Shipping's feed for this channel and package, or `None` where shipping's updater is off
/// (dev and staging) and in a `--profile` run (not `channel_run`), which shares the package
/// with the channel's own runs. An override endpoint replaces the feed even there.
pub fn feed(
    channel: Channel,
    channel_run: bool,
    os: Os,
    arch: &str,
    bundle: BundleType,
    endpoint_override: Option<String>,
) -> Option<Feed> {
    let mut active = false;
    let mut endpoints = Vec::new();
    let mut pubkey = String::new();
    for source in std::iter::once(&BASE).chain(configs(channel, os)) {
        let config: Value = serde_json::from_str(source).expect("shipping's tauri.conf is JSON");
        let updater = &config["plugins"]["updater"];
        if let Some(value) = updater["active"].as_bool() {
            active = value;
        }
        if let Some(value) = updater["endpoints"].as_array() {
            endpoints = value
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        if let Some(value) = updater["pubkey"].as_str() {
            pubkey = value.to_owned();
        }
    }
    match endpoint_override {
        Some(endpoint) => endpoints = vec![endpoint],
        None if !active || !channel_run => return None,
        None => {}
    }
    let (os, installer) = (os_name(os), installer(bundle));
    let endpoints = endpoints
        .into_iter()
        .map(|endpoint| {
            endpoint
                .replace("{{target}}", os)
                .replace("{{arch}}", arch)
                .replace("{{bundle_type}}", installer)
        })
        .collect();
    let target = match bundle {
        BundleType::App => format!("{os}-{arch}"),
        _ => format!("{os}-{arch}-{installer}"),
    };
    Some(Feed {
        endpoints,
        target,
        pubkey,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFFNUI0ODhBNzhBMTEwQQpSV1FLRVlxbmlMVGxDdXJvaUVTTnlwNEF5R1Bodzh3WThMZUpNMVk4K0hhbnV4TDZ1TFFucC9Gbwo=";

    fn url(channel: Channel, os: Os, arch: &str, bundle: BundleType) -> Option<(String, String)> {
        let feed = feed(channel, true, os, arch, bundle, None)?;
        assert_eq!(feed.pubkey, PUBKEY, "shipping's signing key");
        let [endpoint] = <[String; 1]>::try_from(feed.endpoints).unwrap();
        Some((endpoint, feed.target))
    }

    #[test]
    fn channel_builds_read_shipping_feed_for_their_package() {
        let base = "https://desktop.anarlog.so/update";
        for (channel, os, arch, bundle, path, query) in [
            (
                Channel::Stable,
                Os::MacOs,
                "aarch64",
                BundleType::App,
                "darwin-aarch64",
                "stable",
            ),
            (
                Channel::Stable,
                Os::MacOs,
                "x86_64",
                BundleType::App,
                "darwin-x86_64",
                "stable",
            ),
            (
                Channel::Stable,
                Os::Linux,
                "x86_64",
                BundleType::AppImage,
                "linux-x86_64-appimage",
                "stable",
            ),
            (
                Channel::Stable,
                Os::Linux,
                "aarch64",
                BundleType::Deb,
                "linux-aarch64-deb",
                "stable",
            ),
            (
                Channel::Stable,
                Os::Windows,
                "x86_64",
                BundleType::Nsis,
                "windows-x86_64-nsis",
                "stable",
            ),
            (
                Channel::Nightly,
                Os::MacOs,
                "aarch64",
                BundleType::App,
                "darwin-aarch64",
                "nightly",
            ),
            (
                Channel::Nightly,
                Os::Linux,
                "x86_64",
                BundleType::Deb,
                "linux-x86_64-deb",
                "nightly",
            ),
        ] {
            assert_eq!(
                url(channel, os, arch, bundle),
                Some((
                    format!("{base}/{path}/{{{{current_version}}}}?channel={query}"),
                    path.to_owned()
                )),
                "{channel:?} {path}"
            );
        }
        for channel in [Channel::Staging, Channel::Dev] {
            assert_eq!(url(channel, Os::MacOs, "aarch64", BundleType::App), None);
        }
    }

    #[test]
    fn stable_targets_are_the_platforms_releases_publish() {
        let plan: Value = serde_json::from_str(include_str!(
            "../../../../../scripts/desktop-release-platforms.json"
        ))
        .unwrap();
        let published = |os: &str| {
            plan[os]["updatePlatforms"]
                .as_array()
                .unwrap()
                .iter()
                .map(|platform| platform.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        let packages: [(&str, Os, &[BundleType]); 3] = [
            ("macos", Os::MacOs, &[BundleType::App]),
            ("linux", Os::Linux, &[BundleType::AppImage, BundleType::Deb]),
            ("windows", Os::Windows, &[BundleType::Nsis]),
        ];
        for (plan_os, os, bundles) in packages {
            let arches: &[&str] = if os == Os::Windows {
                &["x86_64"]
            } else {
                &["aarch64", "x86_64"]
            };
            let mut targets = arches
                .iter()
                .flat_map(|arch| bundles.iter().map(move |bundle| (arch, bundle)))
                .map(|(arch, bundle)| {
                    feed(Channel::Stable, true, os, arch, *bundle, None)
                        .unwrap()
                        .target
                })
                .collect::<Vec<_>>();
            targets.sort();
            assert_eq!(targets, published(plan_os));
        }
    }

    #[test]
    fn a_profile_run_reads_no_feed_unless_rehearsing() {
        let stable = |endpoint: Option<&str>| {
            feed(
                Channel::Stable,
                false,
                Os::MacOs,
                "aarch64",
                BundleType::App,
                endpoint.map(str::to_owned),
            )
        };
        assert_eq!(stable(None), None);
        assert_eq!(
            stable(Some(
                "https://feed.example/{{target}}-{{arch}}/{{current_version}}"
            ))
            .unwrap()
            .endpoints,
            ["https://feed.example/darwin-aarch64/{{current_version}}"]
        );
    }

    #[test]
    fn a_rehearsal_endpoint_keeps_the_signing_key_and_tokens() {
        let feed = feed(
            Channel::Staging,
            true,
            Os::Linux,
            "x86_64",
            BundleType::AppImage,
            Some(
                "https://feed.example/{{target}}-{{arch}}-{{bundle_type}}/{{current_version}}"
                    .into(),
            ),
        )
        .unwrap();
        assert_eq!(
            feed.endpoints,
            ["https://feed.example/linux-x86_64-appimage/{{current_version}}"]
        );
        assert_eq!(feed.pubkey, PUBKEY);
    }
}
