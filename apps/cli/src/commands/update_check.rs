const RELEASES_PER_PAGE: u32 = 100;

#[derive(serde::Deserialize)]
struct Release {
    tag_name: String,
    prerelease: bool,
}

pub struct UpdateHandle(tokio::task::JoinHandle<Option<String>>);

impl UpdateHandle {
    pub fn spawn() -> Self {
        Self(tokio::spawn(async {
            let current = option_env!("APP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
            let client = reqwest::Client::builder()
                .user_agent("char-cli")
                .timeout(std::time::Duration::from_secs(3))
                .build()
                .ok()?;
            let latest = fetch_latest_cli_release(&client).await?;
            is_newer(&latest, current).then_some(latest)
        }))
    }

    pub async fn result(self) -> Option<String> {
        self.0.await.ok().flatten()
    }
}

async fn fetch_latest_cli_release(client: &reqwest::Client) -> Option<String> {
    let mut best: Option<((u64, u64, u64), String)> = None;
    let mut page = 1;

    loop {
        let releases = fetch_release_page(client, page).await?;
        let is_last = releases.len() < RELEASES_PER_PAGE as usize;

        for release in &releases {
            if let Some((triple, version)) = parse_cli_release(release)
                && best.as_ref().is_none_or(|(b, _)| triple > *b)
            {
                best = Some((triple, version.to_string()));
            }
        }

        if is_last {
            break;
        }
        page += 1;
    }

    best.map(|(_, version)| version)
}

async fn fetch_release_page(client: &reqwest::Client, page: u32) -> Option<Vec<Release>> {
    client
        .get("https://api.github.com/repos/fastrepl/char/releases")
        .query(&[("per_page", RELEASES_PER_PAGE), ("page", page)])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()
}

#[cfg(test)]
fn find_latest_cli_release_on_page(releases: &[Release]) -> Option<String> {
    releases
        .iter()
        .filter_map(parse_cli_release)
        .max_by_key(|(triple, _)| *triple)
        .map(|(_, version)| version.to_string())
}

fn parse_cli_release(release: &Release) -> Option<((u64, u64, u64), &str)> {
    if release.prerelease {
        return None;
    }

    let version = release.tag_name.strip_prefix("cli_v")?;
    Some((parse_triple(version)?, version))
}

fn parse_triple(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.split('-').next().unwrap_or(s);
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_triple(latest), parse_triple(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag_name: &str, prerelease: bool) -> Release {
        Release {
            tag_name: tag_name.to_string(),
            prerelease,
        }
    }

    #[test]
    fn newer_version_detected() {
        assert!(is_newer("1.2.0", "1.1.0"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn handles_prerelease_suffix() {
        assert!(is_newer("1.1.0-beta", "1.0.0"));
    }

    #[test]
    fn selects_highest_stable_cli_release_on_page() {
        assert_eq!(
            find_latest_cli_release_on_page(&[
                release("desktop_v9.9.9", false),
                release("cli_v1.1.0-beta", true),
                release("cli_v1.0.3", false),
                release("cli_v1.0.2", false),
            ]),
            Some("1.0.3".to_string())
        );
    }

    #[test]
    fn picks_highest_version_regardless_of_order() {
        assert_eq!(
            find_latest_cli_release_on_page(&[
                release("cli_v1.0.9", false),
                release("cli_v1.0.8", false),
                release("cli_v1.0.11", false),
                release("cli_v1.0.10", false),
            ]),
            Some("1.0.11".to_string())
        );
    }

    #[test]
    fn returns_none_when_page_has_no_stable_cli_release() {
        assert_eq!(
            find_latest_cli_release_on_page(&[release("desktop_v9.9.9", false)]),
            None
        );
    }
}
