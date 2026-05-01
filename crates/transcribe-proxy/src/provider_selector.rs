use std::collections::{HashMap, HashSet};
use std::fmt;

use owhisper_client::Provider;

use crate::error::SelectionError;

pub struct SelectedProvider {
    provider: Provider,
    api_key: String,
    upstream_url: Option<String>,
}

impl fmt::Debug for SelectedProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let redacted_key = if self.api_key.len() <= 3 {
            "[REDACTED]".to_string()
        } else {
            format!("{}...[REDACTED]", &self.api_key[..3])
        };
        f.debug_struct("SelectedProvider")
            .field("provider", &self.provider)
            .field("api_key", &redacted_key)
            .field("upstream_url", &self.upstream_url)
            .finish()
    }
}

impl SelectedProvider {
    pub fn provider(&self) -> Provider {
        self.provider
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn upstream_url(&self) -> Option<&str> {
        self.upstream_url.as_deref()
    }
}

#[derive(Clone)]
pub struct ProviderSelector {
    api_keys: HashMap<Provider, String>,
    default_provider: Provider,
    upstream_urls: HashMap<Provider, String>,
}

impl ProviderSelector {
    pub fn new(
        api_keys: HashMap<Provider, String>,
        default_provider: Provider,
        upstream_urls: HashMap<Provider, String>,
    ) -> Self {
        Self {
            api_keys,
            default_provider,
            upstream_urls,
        }
    }

    pub fn select(&self, requested: Option<Provider>) -> Result<SelectedProvider, SelectionError> {
        let provider = requested.unwrap_or(self.default_provider);

        let api_key = self
            .api_keys
            .get(&provider)
            .cloned()
            .ok_or(SelectionError::ProviderNotAvailable(provider))?;

        let upstream_url = self.upstream_urls.get(&provider).cloned();

        Ok(SelectedProvider {
            provider,
            api_key,
            upstream_url,
        })
    }

    pub fn default_provider(&self) -> Provider {
        self.default_provider
    }

    pub fn available_providers(&self) -> HashSet<Provider> {
        self.api_keys.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_selector(providers: &[Provider]) -> ProviderSelector {
        let api_keys: HashMap<Provider, String> = providers
            .iter()
            .map(|p| (*p, format!("{:?}_key", p).to_lowercase()))
            .collect();

        ProviderSelector::new(api_keys, Provider::Deepgram, HashMap::new())
    }

    #[test]
    fn test_select_default_provider() {
        let selector = make_selector(&[Provider::Deepgram, Provider::Soniox]);
        let result = selector.select(None).unwrap();

        assert_eq!(result.provider(), Provider::Deepgram);
        assert_eq!(result.api_key(), "deepgram_key");
    }

    #[test]
    fn test_select_requested_provider() {
        let selector = make_selector(&[Provider::Deepgram, Provider::Soniox]);
        let result = selector.select(Some(Provider::Soniox)).unwrap();

        assert_eq!(result.provider(), Provider::Soniox);
        assert_eq!(result.api_key(), "soniox_key");
    }

    #[test]
    fn test_select_unavailable_provider() {
        let selector = make_selector(&[Provider::Deepgram]);
        let result = selector.select(Some(Provider::Soniox));

        assert_eq!(
            result.unwrap_err(),
            SelectionError::ProviderNotAvailable(Provider::Soniox)
        );
    }

    #[test]
    fn test_select_with_custom_upstream_url() {
        let mut upstream_urls = HashMap::new();
        upstream_urls.insert(Provider::Deepgram, "wss://custom.example.com".to_string());

        let mut api_keys = HashMap::new();
        api_keys.insert(Provider::Deepgram, "test_key".to_string());

        let selector = ProviderSelector::new(api_keys, Provider::Deepgram, upstream_urls);
        let result = selector.select(None).unwrap();

        assert_eq!(result.upstream_url(), Some("wss://custom.example.com"));
    }

    #[test]
    fn test_select_without_custom_upstream_url() {
        let selector = make_selector(&[Provider::Deepgram]);
        let result = selector.select(None).unwrap();

        assert_eq!(result.upstream_url(), None);
    }

    #[test]
    fn test_default_provider_not_available_with_explicit_request() {
        let mut api_keys = HashMap::new();
        api_keys.insert(Provider::Soniox, "soniox_key".to_string());

        let selector = ProviderSelector::new(api_keys, Provider::Deepgram, HashMap::new());

        let result = selector.select(None);
        assert_eq!(
            result.unwrap_err(),
            SelectionError::ProviderNotAvailable(Provider::Deepgram)
        );

        let result = selector.select(Some(Provider::Soniox)).unwrap();
        assert_eq!(result.provider(), Provider::Soniox);
    }
}
