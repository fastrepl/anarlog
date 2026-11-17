use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    #[cfg(target_os = "macos")]
    swift_rs::SwiftLinker::new("14.2")
        .with_package("local-llm-swift", "../../plugins/local-llm/swift-lib")
        .link();
    let source = "../../packages/design-system/src/tokens.css";
    println!("cargo:rerun-if-changed={source}");
    let css = fs::read_to_string(source).expect("design system tokens");
    let mut dark = false;
    let mut light = BTreeMap::new();
    let mut night = BTreeMap::new();
    for line in css.lines().map(str::trim) {
        if line == ".dark {" {
            dark = true;
        }
        let Some((name, value)) = line
            .strip_prefix("--")
            .and_then(|line| line.split_once(':'))
        else {
            continue;
        };
        if name == "radius" || value.contains("rgba") {
            continue;
        }
        let channels: Vec<f32> = value
            .trim()
            .trim_end_matches(';')
            .split_whitespace()
            .map(|part| part.trim_end_matches('%').parse().expect("HSL channel"))
            .collect();
        assert_eq!(channels.len(), 3, "Unsupported token {name}");
        let value = format!(
            "gpui::hsla({:?}, {:?}, {:?}, 1.0)",
            channels[0] / 360.0,
            channels[1] / 100.0,
            channels[2] / 100.0
        );
        if dark {
            night.insert(name.replace('-', "_"), value);
        } else {
            light.insert(name.replace('-', "_"), value);
        }
    }
    assert_eq!(
        light.keys().collect::<Vec<_>>(),
        night.keys().collect::<Vec<_>>()
    );
    let mut output = String::from("#[derive(Clone, Copy)]\npub struct Theme {\n");
    for key in light.keys() {
        output.push_str(&format!("pub {key}: gpui::Hsla,\n"));
    }
    output.push_str("}\nimpl Theme {\n");
    for (name, values) in [("light", light), ("dark", night)] {
        output.push_str(&format!("pub fn {name}() -> Self {{ Self {{\n"));
        for (key, value) in values {
            output.push_str(&format!("{key}: {value},\n"));
        }
        output.push_str("} }\n");
    }
    output.push_str("}\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("tokens.rs"),
        output,
    )
    .expect("generated palette");
    bundle_changelog();
    emit_git_sha();
}

/// About's SHA: `ANARLOG_GIT_SHA` when set, else the checkout's short commit, else "unknown".
fn emit_git_sha() {
    println!("cargo:rerun-if-env-changed=ANARLOG_GIT_SHA");
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let sha = env::var("ANARLOG_GIT_SHA")
        .ok()
        .filter(|sha| !sha.trim().is_empty())
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=ANARLOG_GIT_SHA={sha}");
    // Re-run when the checkout moves. Missing paths are skipped: cargo would re-run every build.
    let mut paths = vec![
        git(&["rev-parse", "--git-path", "HEAD"]),
        git(&["rev-parse", "--git-path", "packed-refs"]),
    ];
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        paths.push(git(&["rev-parse", "--git-path", &reference]));
    }
    for path in paths.into_iter().flatten() {
        if Path::new(&path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

/// Shipping's changelog Vite plugin: the newest stable release notes ship with the app.
fn bundle_changelog() {
    let directory = "../../packages/changelog/content/desktop";
    println!("cargo:rerun-if-changed={directory}");
    let latest = fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let version = name.strip_suffix(".md")?.to_owned();
            let parts = version
                .split('.')
                .map(|part| part.parse::<u64>().ok())
                .collect::<Option<Vec<_>>>()?;
            (parts.len() == 3).then_some((parts, version))
        })
        .max();
    let (version, content) = latest.map_or_else(Default::default, |(_, version)| {
        let content = fs::read_to_string(format!("{directory}/{version}.md")).unwrap_or_default();
        (version, content)
    });
    println!("cargo:rustc-env=ANLG_CHANGELOG_VERSION={version}");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("changelog.md"),
        content,
    )
    .expect("bundled changelog");
}
