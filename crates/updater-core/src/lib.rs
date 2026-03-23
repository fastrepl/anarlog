use std::str::FromStr;
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use http::{HeaderName, header::ACCEPT};
use minisign_verify::{PublicKey, Signature};
use percent_encoding::{AsciiSet, CONTROLS};
use reqwest::{
    ClientBuilder, StatusCode,
    header::{HeaderMap, HeaderValue},
};
pub use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use time::OffsetDateTime;
use url::Url;

const UPDATER_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no update endpoints configured")]
    EmptyEndpoints,
    #[error("unsupported operating system")]
    UnsupportedOs,
    #[error("unsupported architecture")]
    UnsupportedArch,
    #[error("no release found")]
    ReleaseNotFound,
    #[error("target not found in release metadata: {0}")]
    TargetNotFound(String),
    #[error("request failed with status {0}")]
    BadStatus(StatusCode),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Semver(#[from] semver::Error),
    #[error(transparent)]
    Time(#[from] time::error::Parse),
    #[error(transparent)]
    HttpHeader(#[from] http::Error),
    #[error(transparent)]
    HeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    SignatureVerify(#[from] minisign_verify::Error),
    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct UpdateConfig {
    pub endpoints: Vec<Url>,
    pub current_version: Version,
    pub target: String,
    pub pubkey: String,
    pub headers: HeaderMap,
    pub timeout: Option<Duration>,
    pub proxy: Option<Url>,
    pub no_proxy: bool,
    pub allow_downgrade: bool,
}

impl UpdateConfig {
    pub fn new(
        endpoints: Vec<Url>,
        current_version: Version,
        target: impl Into<String>,
        pubkey: impl Into<String>,
    ) -> Self {
        Self {
            endpoints,
            current_version,
            target: target.into(),
            pubkey: pubkey.into(),
            headers: HeaderMap::new(),
            timeout: None,
            proxy: None,
            no_proxy: false,
            allow_downgrade: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReleaseManifestPlatform {
    pub url: Url,
    pub signature: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum RemoteReleaseInner {
    Dynamic(ReleaseManifestPlatform),
    Static {
        platforms: std::collections::HashMap<String, ReleaseManifestPlatform>,
    },
}

#[derive(Debug, Clone)]
pub struct RemoteRelease {
    pub version: Version,
    pub notes: Option<String>,
    pub pub_date: Option<OffsetDateTime>,
    pub data: RemoteReleaseInner,
    pub raw_json: serde_json::Value,
}

impl RemoteRelease {
    pub fn download_url(&self, target: &str) -> Result<&Url> {
        match self.data {
            RemoteReleaseInner::Dynamic(ref platform) => Ok(&platform.url),
            RemoteReleaseInner::Static { ref platforms } => platforms.get(target).map_or_else(
                || Err(Error::TargetNotFound(target.to_string())),
                |p| Ok(&p.url),
            ),
        }
    }

    pub fn signature(&self, target: &str) -> Result<&str> {
        match self.data {
            RemoteReleaseInner::Dynamic(ref platform) => Ok(&platform.signature),
            RemoteReleaseInner::Static { ref platforms } => platforms.get(target).map_or_else(
                || Err(Error::TargetNotFound(target.to_string())),
                |p| Ok(p.signature.as_str()),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRelease {
    pub version: Version,
    pub notes: Option<String>,
    pub pub_date: Option<OffsetDateTime>,
    pub target: String,
    pub download_url: Url,
    pub signature: String,
    pub raw_json: serde_json::Value,
}

pub struct UpdateChecker {
    config: UpdateConfig,
}

impl UpdateChecker {
    pub fn new(config: UpdateConfig) -> Result<Self> {
        if config.endpoints.is_empty() {
            return Err(Error::EmptyEndpoints);
        }
        Ok(Self { config })
    }

    pub async fn check(&self) -> Result<Option<ResolvedRelease>> {
        let mut headers = self.config.headers.clone();
        if !headers.contains_key(ACCEPT) {
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        }

        let mut remote_release: Option<RemoteRelease> = None;
        let mut last_error: Option<Error> = None;

        for endpoint in &self.config.endpoints {
            let endpoint = replace_url_tokens(
                endpoint,
                &self.config.current_version,
                &self.config.target,
                updater_arch().ok_or(Error::UnsupportedArch)?,
            )?;

            let response = self
                .client_builder()?
                .build()?
                .get(endpoint)
                .headers(headers.clone())
                .send()
                .await;

            match response {
                Ok(res) if res.status() == StatusCode::NO_CONTENT => return Ok(None),
                Ok(res) if res.status().is_success() => {
                    let value: serde_json::Value = res.json().await?;
                    let mut parsed: RemoteRelease = serde_json::from_value(value.clone())?;
                    parsed.raw_json = value;
                    remote_release = Some(parsed);
                    last_error = None;
                    break;
                }
                Ok(res) => {
                    last_error = Some(Error::BadStatus(res.status()));
                }
                Err(err) => {
                    last_error = Some(err.into());
                }
            }
        }

        if let Some(err) = last_error {
            return Err(err);
        }

        let release = remote_release.ok_or(Error::ReleaseNotFound)?;
        let is_newer = release.version > self.config.current_version;
        let should_update = if self.config.allow_downgrade {
            release.version != self.config.current_version
        } else {
            is_newer
        };

        if !should_update {
            return Ok(None);
        }

        let download_url = release.download_url(&self.config.target)?.clone();
        let signature = release.signature(&self.config.target)?.to_string();

        Ok(Some(ResolvedRelease {
            version: release.version,
            notes: release.notes,
            pub_date: release.pub_date,
            target: self.config.target.clone(),
            download_url,
            signature,
            raw_json: release.raw_json,
        }))
    }

    pub async fn download<C>(&self, release: &ResolvedRelease, mut on_chunk: C) -> Result<Vec<u8>>
    where
        C: FnMut(usize, Option<u64>),
    {
        let mut headers = self.config.headers.clone();
        if !headers.contains_key(ACCEPT) {
            headers.insert(ACCEPT, HeaderValue::from_static("application/octet-stream"));
        }

        let response = self
            .client_builder()?
            .build()?
            .get(release.download_url.clone())
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::BadStatus(response.status()));
        }

        let content_length = response
            .headers()
            .get(HeaderName::from_static("content-length"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            on_chunk(chunk.len(), content_length);
            bytes.extend(chunk);
        }

        verify_signature(&bytes, &release.signature, &self.config.pubkey)?;

        Ok(bytes)
    }

    fn client_builder(&self) -> Result<ClientBuilder> {
        let mut builder = ClientBuilder::new().user_agent(UPDATER_USER_AGENT);
        if let Some(timeout) = self.config.timeout {
            builder = builder.timeout(timeout);
        }
        if self.config.no_proxy {
            builder = builder.no_proxy();
        } else if let Some(ref proxy) = self.config.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy.as_str())?);
        }
        Ok(builder)
    }
}

pub fn target() -> Option<String> {
    Some(format!("{}-{}", updater_os()?, updater_arch()?))
}

pub fn updater_os() -> Option<&'static str> {
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

pub fn updater_arch() -> Option<&'static str> {
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

fn replace_url_tokens(
    endpoint: &Url,
    current_version: &Version,
    target: &str,
    arch: &str,
) -> Result<Url> {
    let version = current_version.to_string();
    const CONTROLS_ADD: &AsciiSet = &CONTROLS.add(b'+');
    let encoded_version = percent_encoding::percent_encode(version.as_bytes(), CONTROLS_ADD);
    let encoded_version = encoded_version.to_string();
    let replaced = endpoint
        .to_string()
        .replace("%7B%7Bcurrent_version%7D%7D", &encoded_version)
        .replace("%7B%7Btarget%7D%7D", target)
        .replace("%7B%7Barch%7D%7D", arch)
        .replace("{{current_version}}", &encoded_version)
        .replace("{{target}}", target)
        .replace("{{arch}}", arch);
    Ok(replaced.parse()?)
}

fn verify_signature(data: &[u8], release_signature: &str, pubkey: &str) -> Result<()> {
    let pubkey_decoded = base64_to_utf8(pubkey)?;
    let public_key = PublicKey::decode(&pubkey_decoded)?;
    let signature_decoded = base64_to_utf8(release_signature)?;
    let signature = Signature::decode(&signature_decoded)?;
    public_key.verify(data, &signature, true)?;
    Ok(())
}

fn base64_to_utf8(value: &str) -> Result<String> {
    let decoded = base64::engine::general_purpose::STANDARD.decode(value)?;
    Ok(std::str::from_utf8(&decoded)?.to_string())
}

impl<'de> Deserialize<'de> for RemoteRelease {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct InnerRemoteRelease {
            #[serde(alias = "name", deserialize_with = "parse_version")]
            version: Version,
            notes: Option<String>,
            pub_date: Option<String>,
            platforms: Option<std::collections::HashMap<String, ReleaseManifestPlatform>>,
            url: Option<Url>,
            signature: Option<String>,
        }

        let release = InnerRemoteRelease::deserialize(deserializer)?;
        let pub_date = if let Some(date) = release.pub_date {
            Some(
                OffsetDateTime::parse(&date, &time::format_description::well_known::Rfc3339)
                    .map_err(|e| D::Error::custom(format!("invalid pub_date: {e}")))?,
            )
        } else {
            None
        };

        let data = if let Some(platforms) = release.platforms {
            RemoteReleaseInner::Static { platforms }
        } else {
            RemoteReleaseInner::Dynamic(ReleaseManifestPlatform {
                url: release
                    .url
                    .ok_or_else(|| D::Error::custom("missing `url` in update response"))?,
                signature: release
                    .signature
                    .ok_or_else(|| D::Error::custom("missing `signature` in update response"))?,
            })
        };

        Ok(Self {
            version: release.version,
            notes: release.notes,
            pub_date,
            data,
            raw_json: serde_json::Value::Null,
        })
    }
}

fn parse_version<'de, D>(deserializer: D) -> std::result::Result<Version, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Version::from_str(value.trim_start_matches('v')).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_url_tokens() {
        let endpoint = Url::parse(
            "https://example.com/update/{{target}}/{{arch}}/{{current_version}}?v={{current_version}}",
        )
        .unwrap();
        let replaced = replace_url_tokens(
            &endpoint,
            &Version::from_str("1.2.3").unwrap(),
            "darwin-aarch64",
            "aarch64",
        )
        .unwrap();

        assert_eq!(
            replaced.as_str(),
            "https://example.com/update/darwin-aarch64/aarch64/1.2.3?v=1.2.3"
        );
    }

    #[test]
    fn deserializes_dynamic_release_shape() {
        let json = serde_json::json!({
            "version": "v1.2.3",
            "notes": "hello",
            "pub_date": "2024-03-19T02:35:10.440Z",
            "url": "https://example.com/app.tar.gz",
            "signature": "Zm9v"
        });

        let release = serde_json::from_value::<RemoteRelease>(json).unwrap();
        assert_eq!(release.version, Version::from_str("1.2.3").unwrap());
        assert!(matches!(release.data, RemoteReleaseInner::Dynamic(_)));
    }
}
