use hypr_version::Version;

macro_rules! version_from_name {
    () => {{
        static V: std::sync::LazyLock<hypr_version::Version> = std::sync::LazyLock::new(|| {
            let name = module_path!().rsplit("::").next().unwrap();
            $crate::version::r#macro::parse(name)
        });
        &*V
    }};
}

pub(crate) use version_from_name;

pub fn parse(name: &str) -> Version {
    const PRERELEASE_TAGS: &[&str] = &["nightly", "alpha", "beta", "rc"];

    let name = name.strip_prefix("v").unwrap_or(name);
    let parts: Vec<&str> = name.split('_').collect();

    let major = parts.first().copied().unwrap_or("0");
    let minor = parts.get(1).copied().unwrap_or("0");
    let patch = parts.get(2).copied().unwrap_or("0");

    let mut version_str = format!("{major}.{minor}.{patch}");

    if let Some(&tag) = parts.get(3)
        && PRERELEASE_TAGS.contains(&tag)
            && let Some(&num) = parts.get(4)
                && num.chars().all(|c| c.is_ascii_digit()) {
                    version_str.push_str(&format!("-{tag}.{num}"));
                }

    version_str.parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let cases = [
            ("v1_0_2_simple", "1.0.2"),
            ("v1_0_2_extract_from_sqlite", "1.0.2"),
            ("v1_0_2_nightly_3_move_uuid_folders", "1.0.2-nightly.3"),
            ("v1_0_2_nightly_4_rename_transcript", "1.0.2-nightly.4"),
        ];

        for (input, expected) in cases {
            assert_eq!(parse(input).to_string(), expected, "input: {input}");
        }
    }
}
