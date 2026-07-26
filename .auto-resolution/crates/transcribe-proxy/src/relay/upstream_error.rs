pub use owhisper_client::ProviderError as UpstreamError;

pub fn detect_upstream_error(data: &[u8]) -> Option<UpstreamError> {
    owhisper_client::Provider::detect_any_error(data)
}
