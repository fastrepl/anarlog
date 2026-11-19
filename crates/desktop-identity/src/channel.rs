pub const DEV_IDENTIFIER: &str = "com.hyprnote.dev";
pub const STAGING_IDENTIFIER: &str = "com.hyprnote.staging";
pub const NIGHTLY_IDENTIFIER: &str = "com.hyprnote.nightly";
pub const STABLE_IDENTIFIER: &str = "com.hyprnote.stable";
/// Stable's identifier before the rename; its secrets live in stable's keychain service.
pub const LEGACY_STABLE_IDENTIFIER: &str = "com.hyprnote.Hyprnote";

/// The shipping app's release channels, as configured in `apps/desktop/src-tauri/tauri.conf*.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Dev,
    Staging,
    Nightly,
    Stable,
}

impl Channel {
    pub const ALL: [Self; 4] = [Self::Dev, Self::Staging, Self::Nightly, Self::Stable];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|channel| channel.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Staging => "staging",
            Self::Nightly => "nightly",
            Self::Stable => "stable",
        }
    }

    pub fn identifier(self) -> &'static str {
        match self {
            Self::Dev => DEV_IDENTIFIER,
            Self::Staging => STAGING_IDENTIFIER,
            Self::Nightly => NIGHTLY_IDENTIFIER,
            Self::Stable => STABLE_IDENTIFIER,
        }
    }

    pub fn product_name(self) -> &'static str {
        match self {
            Self::Dev => "Anarlog Dev",
            Self::Staging => "Anarlog Staging",
            Self::Nightly => "Anarlog Nightly",
            Self::Stable => "Anarlog",
        }
    }

    /// Deep-link schemes this channel registers; the first is the one it builds links with.
    pub fn schemes(self) -> &'static [&'static str] {
        match self {
            Self::Dev => &["anarlog-dev", "hypr", "char"],
            Self::Staging => &["anarlog-staging", "hyprnote-staging"],
            Self::Nightly => &["anarlog-nightly", "hyprnote-nightly"],
            Self::Stable => &["anarlog", "hyprnote"],
        }
    }

    /// Whether a build of this kind finds the same data as the shipping app of this channel.
    /// Data folders depend on the build kind (`anlg_storage::global::compute_default_base`), and
    /// the shipping app builds dev in debug and stable and nightly in release. A debug build
    /// claiming stable or nightly would open real notes under a bundle-ID folder, and a release
    /// build claiming dev would open stable's.
    pub fn matches_build(self, is_debug: bool) -> bool {
        match self {
            Self::Dev => is_debug,
            Self::Staging => true,
            Self::Nightly | Self::Stable => !is_debug,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tauri_config(channel: Channel) -> serde_json::Value {
        let file = match channel {
            Channel::Dev => "tauri.conf.json".to_string(),
            channel => format!("tauri.conf.{}.json", channel.name()),
        };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/desktop/src-tauri")
            .join(file);
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[test]
    fn channels_match_the_shipping_tauri_configs() {
        for channel in Channel::ALL {
            let config = tauri_config(channel);
            let schemes: Vec<&str> = config["plugins"]["deep-link"]["desktop"]["schemes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|scheme| scheme.as_str().unwrap())
                .collect();

            assert_eq!(config["identifier"], channel.identifier(), "{channel:?}");
            assert_eq!(config["productName"], channel.product_name(), "{channel:?}");
            assert_eq!(schemes, channel.schemes(), "{channel:?}");
            assert_eq!(Channel::from_name(channel.name()), Some(channel));
        }
        assert_eq!(Channel::from_name("Stable"), None);
        assert_eq!(Channel::from_name(""), None);
    }

    #[test]
    fn a_channel_matches_only_the_build_kind_that_finds_its_shipping_data() {
        let data = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        let shipping_is_debug = |channel| channel == Channel::Dev;
        for channel in Channel::ALL {
            let shipping = crate::Locations::resolve_in(
                data.path(),
                local.path(),
                channel.identifier(),
                shipping_is_debug(channel),
            );
            for is_debug in [false, true] {
                let build = crate::Locations::resolve_in(
                    data.path(),
                    local.path(),
                    channel.identifier(),
                    is_debug,
                );
                assert_eq!(
                    channel.matches_build(is_debug),
                    build == shipping,
                    "{channel:?} debug={is_debug}"
                );
            }
        }
    }
}
