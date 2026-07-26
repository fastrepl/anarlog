use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(semver::Version);

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver::Version::new(major, minor, patch))
    }
}

impl FromStr for Version {
    type Err = semver::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        semver::Version::parse(s).map(Self)
    }
}

impl Deref for Version {
    type Target = semver::Version;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_order {
        ($name:ident: $($version:literal),+ $(,)?) => {
            #[test]
            fn $name() {
                let versions = [$($version),+];
                for i in 0..versions.len() {
                    for j in (i + 1)..versions.len() {
                        let left: Version = versions[i].parse().unwrap();
                        let right: Version = versions[j].parse().unwrap();
                        assert!(
                            left < right,
                            "Expected {} < {}, but got {} >= {}",
                            versions[i], versions[j], versions[i], versions[j]
                        );
                    }
                }
            }
        };
    }

    test_order!(basic_semver: "1.0.0", "1.0.1", "1.1.0", "2.0.0");

    test_order!(prerelease_vs_release: "1.0.0-alpha", "1.0.0-beta", "1.0.0-rc.1", "1.0.0");

    test_order!(numeric_prerelease_identifiers: "1.0.0-pre.1", "1.0.0-pre.2", "1.0.0-pre.10", "1.0.0-pre.100");

    test_order!(nightly_releases: "1.0.0-nightly.1", "1.0.0-nightly.2", "1.0.0-nightly.10", "1.0.0-nightly.32", "1.0.0");

    test_order!(
        nightly_across_patches:
        "1.0.0-nightly.1",
        "1.0.0-nightly.32",
        "1.0.0",
        "1.0.1-nightly.1",
        "1.0.1-nightly.4",
        "1.0.1",
        "1.0.2-nightly.1",
        "1.0.2-nightly.12",
    );

    test_order!(
        dev_builds_extend_prerelease:
        "1.0.2-nightly.12",
        "1.0.2-nightly.12.dev.1",
        "1.0.2-nightly.12.dev.100",
        "1.0.2-nightly.12.dev.5169",
        "1.0.2-nightly.13",
    );

    test_order!(
        full_release_cycle:
        "1.0.1",
        "1.0.2-nightly.1",
        "1.0.2-nightly.1.dev.10",
        "1.0.2-nightly.1.dev.100",
        "1.0.2-nightly.2",
        "1.0.2-nightly.12",
        "1.0.2-nightly.12.dev.1",
        "1.0.2-nightly.12.dev.5169",
        "1.0.2-nightly.13",
        "1.0.2",
        "1.0.3-nightly.1",
    );

    test_order!(string_vs_numeric_identifiers: "1.0.0-1", "1.0.0-2", "1.0.0-10", "1.0.0-alpha", "1.0.0-beta");

    test_order!(longer_prerelease_is_greater: "1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-alpha.1.beta");

    #[test]
    fn build_metadata_comparison() {
        let base: Version = "1.0.2-nightly.12.dev.5169".parse().unwrap();
        let with_meta: Version = "1.0.2-nightly.12.dev.5169+8797281".parse().unwrap();
        let with_other_meta: Version = "1.0.2-nightly.12.dev.5169+abcdef0".parse().unwrap();

        assert!(base < with_meta);
        assert!(with_meta < with_other_meta);
    }

    #[test]
    fn real_world_staging_build() {
        let nightly_tag: Version = "1.0.2-nightly.12".parse().unwrap();
        let staging_build: Version = "1.0.2-nightly.12.dev.5169+8797281".parse().unwrap();
        let next_nightly: Version = "1.0.2-nightly.13".parse().unwrap();
        let stable: Version = "1.0.2".parse().unwrap();

        assert!(nightly_tag < staging_build);
        assert!(staging_build < next_nightly);
        assert!(next_nightly < stable);
    }
}
