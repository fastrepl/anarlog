pub(crate) fn patch_huggingface_downloader_source(source: &str) -> Result<String, &'static str> {
    const ORIGINAL: &str = "        return HubApi(downloadBase: downloadBase, endpoint: resolvedEndpoint(), useOfflineMode: offlineMode)";
    const PATCHED: &str = "        return HubApi(downloadBase: downloadBase, hfToken: \"\", endpoint: resolvedEndpoint(), useOfflineMode: offlineMode)";

    if source.contains(PATCHED) {
        return Ok(source.to_string());
    }

    if !source.contains(ORIGINAL) {
        return Err("HuggingFaceDownloader HubApi construction not found");
    }

    Ok(source.replacen(ORIGINAL, PATCHED, 1))
}

#[cfg(test)]
mod tests {
    use super::patch_huggingface_downloader_source;

    const SOURCE: &str = r#"    private static func makeHubApi(for modelId: String, repoDir: URL, offlineMode: Bool) -> HubApi {
        return HubApi(downloadBase: downloadBase, endpoint: resolvedEndpoint(), useOfflineMode: offlineMode)
    }
"#;

    #[test]
    fn forces_anonymous_hub_api_for_built_in_models() {
        let patched = patch_huggingface_downloader_source(SOURCE).unwrap();

        assert!(patched.contains("hfToken: \"\""));
        assert!(!patched.contains(
            "return HubApi(downloadBase: downloadBase, endpoint: resolvedEndpoint(), useOfflineMode: offlineMode)"
        ));
    }

    #[test]
    fn anonymous_hub_api_patch_is_idempotent() {
        let patched = patch_huggingface_downloader_source(SOURCE).unwrap();

        assert_eq!(
            patch_huggingface_downloader_source(&patched).unwrap(),
            patched
        );
    }

    #[test]
    fn anonymous_hub_api_patch_rejects_unknown_source() {
        let error = patch_huggingface_downloader_source("public enum Unrelated {}").unwrap_err();

        assert_eq!(error, "HuggingFaceDownloader HubApi construction not found");
    }
}
