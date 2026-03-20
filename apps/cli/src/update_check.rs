use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const STALE_THRESHOLD_SECS: u64 = 20 * 60 * 60; // 20 hours

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Nightly,
}

impl Channel {
    fn detect(version: &str) -> Self {
        if version.contains("-nightly") {
            Channel::Nightly
        } else {
            Channel::Stable
        }
    }
}

pub enum UpdateAction {
    Npm { package: String },
}

impl UpdateAction {
    pub fn detect(channel: Channel) -> Option<Self> {
        if std::env::var("CHAR_MANAGED_BY_NPM").unwrap_or_default() != "1" {
            return None;
        }
        let tag = match channel {
            Channel::Stable => "latest",
            Channel::Nightly => "nightly",
        };
        Some(UpdateAction::Npm {
            package: format!("char@{tag}"),
        })
    }

    pub fn command_str(&self) -> String {
        match self {
            UpdateAction::Npm { package } => format!("npm install -g {package}"),
        }
    }

    pub fn run(&self) -> std::io::Result<std::process::ExitStatus> {
        match self {
            UpdateAction::Npm { package } => std::process::Command::new("npm")
                .args(["install", "-g", package])
                .status(),
        }
    }
}

pub enum UpdateStatus {
    UpdateAvailable {
        current: String,
        latest: String,
        channel: Channel,
    },
    NoUpdate,
}

#[derive(Serialize, Deserialize, Default)]
struct Cache {
    latest_version: String,
    checked_at: u64,
    skipped_version: Option<String>,
}

fn cache_path(channel: Channel) -> PathBuf {
    let data_dir = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let filename = match channel {
        Channel::Stable => "update_cache.json",
        Channel::Nightly => "update_cache_nightly.json",
    };
    data_dir.join("char").join(filename)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_cache(channel: Channel) -> Option<Cache> {
    let data = std::fs::read_to_string(cache_path(channel)).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_cache(channel: Channel, cache: &Cache) {
    let path = cache_path(channel);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, data);
    }
}

pub fn save_skipped_version(version: &str) {
    let channel = Channel::detect(version);
    let mut cache = load_cache(channel).unwrap_or_default();
    cache.skipped_version = Some(version.to_string());
    save_cache(channel, &cache);
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// Parse a version string into a comparable tuple.
/// Handles both `0.1.2` and `0.1.2-nightly.5` formats.
fn parse_version(s: &str) -> Option<(u64, u64, u64, u64)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let (base, nightly_num) = if let Some((base, suffix)) = s.split_once("-nightly.") {
        (base, suffix.parse::<u64>().unwrap_or(0))
    } else {
        (s, 0)
    };
    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        nightly_num,
    ))
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
}

async fn fetch_latest_cli_version(channel: Channel) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let releases: Vec<GithubRelease> = client
        .get("https://api.github.com/repos/fastrepl/char/releases?per_page=20")
        .header("User-Agent", "char-cli")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    releases.iter().find_map(|r| {
        let version = r.tag_name.strip_prefix("cli_v")?;
        if Channel::detect(version) != channel {
            return None;
        }
        if channel == Channel::Stable && r.prerelease {
            return None;
        }
        Some(version.to_string())
    })
}

pub async fn check_for_update() -> UpdateStatus {
    let current = match option_env!("APP_VERSION") {
        Some(v) if v != "dev" && !v.is_empty() => v,
        _ => return UpdateStatus::NoUpdate,
    };

    let channel = Channel::detect(current);
    let cache = load_cache(channel);
    let now = now_secs();

    let latest = if cache
        .as_ref()
        .is_some_and(|c| now.saturating_sub(c.checked_at) < STALE_THRESHOLD_SECS)
    {
        cache.as_ref().unwrap().latest_version.clone()
    } else {
        match fetch_latest_cli_version(channel).await {
            Some(v) => {
                save_cache(
                    channel,
                    &Cache {
                        latest_version: v.clone(),
                        checked_at: now,
                        skipped_version: cache.as_ref().and_then(|c| c.skipped_version.clone()),
                    },
                );
                v
            }
            None => return UpdateStatus::NoUpdate,
        }
    };

    if !is_newer(&latest, current) {
        return UpdateStatus::NoUpdate;
    }

    if cache.as_ref().and_then(|c| c.skipped_version.as_deref()) == Some(latest.as_str()) {
        return UpdateStatus::NoUpdate;
    }

    UpdateStatus::UpdateAvailable {
        current: current.to_string(),
        latest,
        channel,
    }
}
