use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

use anlg_desktop_updater::{Error, Result, UpdateBackend, UpdateEvents, UpdatePolicy, Updater};
use base64::Engine as _;
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use reqwest::StatusCode;
use semver::Version;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct ManifestPlatform {
    url: String,
    signature: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    #[serde(deserialize_with = "deserialize_version")]
    version: Version,
    #[allow(dead_code)]
    notes: Option<String>,
    #[allow(dead_code)]
    pub_date: Option<String>,
    platforms: Option<HashMap<String, ManifestPlatform>>,
    url: Option<String>,
    signature: Option<String>,
}

impl Manifest {
    fn platform(&self, platform_key: &str) -> Result<ManifestPlatform> {
        if let Some(platform) = self
            .platforms
            .as_ref()
            .and_then(|platforms| platforms.get(platform_key))
        {
            return Ok(platform.clone());
        }
        match (self.url.as_ref(), self.signature.as_ref()) {
            (Some(url), Some(signature)) => Ok(ManifestPlatform {
                url: url.clone(),
                signature: signature.clone(),
            }),
            _ => Err(Error::Backend(format!(
                "unsupported updater platform {platform_key}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
struct Release {
    version: Version,
    url: String,
    signature: String,
}

pub(crate) struct FeedUpdateBackend {
    client: reqwest::Client,
    endpoints: Vec<String>,
    pubkey: String,
    current_version: Version,
    target: String,
    arch: String,
    release: Mutex<Option<Release>>,
}

impl FeedUpdateBackend {
    pub(crate) fn from_environment(current_version: &str) -> Option<Self> {
        let endpoints = option_env!("ANARLOG_UPDATER_ENDPOINTS")?
            .split(',')
            .map(str::trim)
            .filter(|endpoint| !endpoint.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let pubkey = option_env!("ANARLOG_UPDATER_PUBKEY")?.to_string();
        if endpoints.is_empty() || pubkey.is_empty() {
            return None;
        }
        let current_version = Version::parse(current_version).ok()?;
        Some(Self {
            client: reqwest::Client::new(),
            endpoints,
            pubkey,
            current_version,
            target: updater_target()?.to_string(),
            arch: updater_arch()?.to_string(),
            release: Mutex::new(None),
        })
    }

    fn endpoint(&self, endpoint: &str) -> String {
        substitute_endpoint(
            endpoint,
            &self.target,
            &self.arch,
            self.current_version.to_string().as_str(),
            updater_bundle_type(),
        )
    }

    async fn check_feed(&self) -> Result<Option<Release>> {
        let platform_key = format!("{}-{}", self.target, self.arch);
        let mut last_error = None;
        for endpoint in &self.endpoints {
            let response = match self
                .client
                .get(self.endpoint(endpoint))
                .header(reqwest::header::ACCEPT, "application/json")
                .send()
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    last_error = Some(Error::Backend(error.to_string()));
                    continue;
                }
            };
            if response.status() == StatusCode::NO_CONTENT {
                return Ok(None);
            }
            if !response.status().is_success() {
                last_error = Some(Error::Backend(format!(
                    "update endpoint returned {}",
                    response.status()
                )));
                continue;
            }
            let manifest = match response.json::<Manifest>().await {
                Ok(manifest) => manifest,
                Err(error) => {
                    last_error = Some(Error::Backend(error.to_string()));
                    continue;
                }
            };
            if manifest.version <= self.current_version {
                return Ok(None);
            }
            let platform = manifest.platform(&platform_key)?;
            return Ok(Some(Release {
                version: manifest.version,
                url: platform.url.clone(),
                signature: platform.signature.clone(),
            }));
        }
        Err(last_error.unwrap_or_else(|| Error::Backend("no update endpoint configured".into())))
    }
}

impl UpdateBackend for FeedUpdateBackend {
    fn check(&self) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send + '_>> {
        Box::pin(async move {
            let release = self.check_feed().await?;
            let version = release.as_ref().map(|release| release.version.to_string());
            *self.release.lock().unwrap() = release;
            Ok(version)
        })
    }

    fn download<'a>(
        &'a self,
        version: &'a str,
        on_progress: &'a (dyn Fn(u64, Option<u64>) + Send + Sync),
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            let release = self
                .release
                .lock()
                .unwrap()
                .clone()
                .filter(|release| {
                    Version::parse(version)
                        .map(|version| release.version == version)
                        .unwrap_or(false)
                })
                .ok_or(Error::UpdateNotAvailable)?;
            let response = self
                .client
                .get(release.url)
                .header(reqwest::header::ACCEPT, "application/octet-stream")
                .send()
                .await
                .map_err(|error| Error::Backend(error.to_string()))?;
            if !response.status().is_success() {
                return Err(Error::Backend(format!(
                    "update download returned {}",
                    response.status()
                )));
            }
            let total = response.content_length();
            let mut bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| Error::Backend(error.to_string()))?;
                on_progress(chunk.len() as u64, total);
                bytes.extend_from_slice(&chunk);
            }
            verify_signature(&bytes, &release.signature, &self.pubkey)?;
            Ok(bytes)
        })
    }

    fn install(&self, _version: &str, _bytes: &[u8]) -> Result<()> {
        Err(Error::Backend(
            "install is not implemented for the GPUI shell yet".into(),
        ))
    }

    fn supports_install(&self) -> bool {
        false
    }
}

pub(crate) struct FeedUpdateEvents {
    ready: Arc<Mutex<Option<String>>>,
}

impl FeedUpdateEvents {
    pub(crate) fn new() -> Self {
        Self {
            ready: Arc::new(Mutex::new(None)),
        }
    }
}

impl UpdateEvents for FeedUpdateEvents {
    fn available(&self, version: &str) {
        tracing::info!(version, "update_available");
    }

    fn downloading(&self, version: &str) {
        tracing::info!(version, "update_downloading");
    }

    fn progress(&self, version: &str, chunk: u64, total: Option<u64>) {
        tracing::debug!(version, chunk, ?total, "update_download_progress");
    }

    fn download_failed(&self, version: &str) {
        tracing::warn!(version, "update_download_failed");
    }

    fn ready(&self, version: &str) {
        tracing::info!(version, "update_ready");
        *self.ready.lock().unwrap() = Some(version.to_string());
    }
}

pub(crate) fn spawn_update_loop(
    handle: &tokio::runtime::Handle,
    current_version: &str,
    updates_dir: std::path::PathBuf,
    store: Arc<crate::db::Store>,
) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let Some(backend) = FeedUpdateBackend::from_environment(current_version) else {
        tracing::debug!("gpui_updater_not_configured");
        return;
    };
    let events = Arc::new(FeedUpdateEvents::new());
    let updater = Arc::new(Updater::new(
        Arc::new(backend),
        events,
        updates_dir,
        current_version,
    ));
    handle.spawn(async move {
        let mut install_at_open = true;
        loop {
            let automatic_updates_enabled = store
                .load_provider_settings()
                .await
                .ok()
                .and_then(|settings| settings.ok())
                .map(|settings| {
                    settings.bool_setting(
                        "automatic_updates",
                        &["general", "automatic_updates"],
                        true,
                    )
                })
                .unwrap_or(true);
            let policy = || UpdatePolicy {
                automatic_updates_enabled,
                meeting_active: false,
            };
            install_at_open = updater.tick(&policy, install_at_open).await;
            tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
        }
    });
}

fn deserialize_version<'de, D>(deserializer: D) -> std::result::Result<Version, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let version = String::deserialize(deserializer)?;
    Version::parse(version.trim_start_matches('v')).map_err(serde::de::Error::custom)
}

fn verify_signature(bytes: &[u8], signature: &str, pubkey: &str) -> Result<()> {
    let decode = |value: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(value)
            .map_err(|error| Error::Backend(error.to_string()))
    };
    let public_key = PublicKey::decode(
        std::str::from_utf8(&decode(pubkey)?).map_err(|error| Error::Backend(error.to_string()))?,
    )
    .map_err(|error| Error::Backend(error.to_string()))?;
    let signature = Signature::decode(
        std::str::from_utf8(&decode(signature)?)
            .map_err(|error| Error::Backend(error.to_string()))?,
    )
    .map_err(|error| Error::Backend(error.to_string()))?;
    public_key
        .verify(bytes, &signature, true)
        .map_err(|error| Error::Backend(error.to_string()))
}

fn updater_target() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        Some("linux")
    } else if cfg!(target_os = "macos") {
        Some("darwin")
    } else if cfg!(target_os = "windows") {
        Some("windows")
    } else {
        None
    }
}

fn updater_arch() -> Option<&'static str> {
    if cfg!(target_arch = "x86") {
        Some("i686")
    } else if cfg!(target_arch = "x86_64") {
        Some("x86_64")
    } else if cfg!(target_arch = "arm") {
        Some("armv7")
    } else if cfg!(target_arch = "aarch64") {
        Some("aarch64")
    } else if cfg!(target_arch = "riscv64") {
        Some("riscv64")
    } else {
        None
    }
}

fn updater_bundle_type() -> &'static str {
    if cfg!(target_os = "linux") {
        "appimage"
    } else if cfg!(target_os = "macos") {
        "app"
    } else if cfg!(target_os = "windows") {
        "nsis"
    } else {
        "unknown"
    }
}

fn substitute_endpoint(
    endpoint: &str,
    target: &str,
    arch: &str,
    current_version: &str,
    bundle_type: &str,
) -> String {
    endpoint
        .replace("{{target}}", target)
        .replace("{{arch}}", arch)
        .replace("{{current_version}}", current_version)
        .replace("{{bundle_type}}", bundle_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_platform_keys() {
        for target in ["darwin-aarch64", "linux-x86_64", "windows-x86_64"] {
            let manifest: Manifest = serde_json::from_value(serde_json::json!({
                "version": "v1.2.3",
                "notes": "notes",
                "pub_date": "2025-01-01T00:00:00Z",
                "platforms": {(target): {"url": "https://example.com/update", "signature": "sig"}}
            }))
            .unwrap();
            assert!(manifest.platforms.unwrap().contains_key(target));
        }
    }

    #[test]
    fn resolves_dynamic_manifest_shape() {
        let manifest: Manifest = serde_json::from_value(serde_json::json!({
            "version": "v1.2.3",
            "url": "https://example.com/update",
            "signature": "sig"
        }))
        .unwrap();
        let platform = manifest.platform("linux-x86_64").unwrap();
        assert_eq!(platform.url, "https://example.com/update");
        assert_eq!(platform.signature, "sig");
    }

    #[test]
    fn substitutes_all_endpoint_placeholders() {
        assert_eq!(
            substitute_endpoint(
                "https://example.test/{{target}}/{{arch}}/{{bundle_type}}/{{current_version}}",
                "linux",
                "x86_64",
                "1.2.3",
                "appimage",
            ),
            "https://example.test/linux/x86_64/appimage/1.2.3"
        );
    }

    #[test]
    fn verifies_tauri_minisign_vector() {
        let public_key = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgRTc2MjBGMTg0MkI0RTgxRgpSV1FmNkxSQ0dBOWk1M21sWWVjTzRJelQ1MVRHUHB2V3VjTlNDaDFDQk0wUVRhTG43M1k3R0ZPMw==";
        let signature = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIG1pbmlzaWduIHNlY3JldCBrZXkKUldRZjZMUkNHQTlpNTlTTE9GeHo2Tnh2QVNYREplUnR1Wnlrd1FlcGJERUd0ODdpZzFCTnBXYVZXdU5ybTczWWlJaUpicTcxV2krZFA5ZUtMOE9DMzUxdndJYXNTU2JYeHdBPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNTU1Nzc5OTY2CWZpbGU6dGVzdApRdEtNWFd5WWN3ZHBaQWxQRjd0RTJFTkprUmQxdWp2S2psajFtOVJ0SFRCblpQYTVXS1U1dVdSczVHb1A1TS9WcUU4MVFGdU1LSTVrL1NmTlFVYU9BQT09";
        verify_signature(b"test", signature, public_key).unwrap();
    }

    #[tokio::test]
    async fn treats_no_content_as_no_update() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut request).await;
            tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n",
            )
            .await
            .unwrap();
        });
        let backend = FeedUpdateBackend {
            client: reqwest::Client::new(),
            endpoints: vec![format!("http://{address}")],
            pubkey: "unused".into(),
            current_version: Version::new(1, 0, 0),
            target: "linux".into(),
            arch: "x86_64".into(),
            release: Mutex::new(None),
        };
        assert!(backend.check_feed().await.unwrap().is_none());
    }
}
