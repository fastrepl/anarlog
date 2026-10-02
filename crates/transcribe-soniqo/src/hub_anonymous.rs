pub(crate) fn patch_huggingface_transfer_source(source: &str) -> Result<String, &'static str> {
    const ORIGINAL: &str = r#"    static func applyHubAuth(to request: inout URLRequest) {
        let env = ProcessInfo.processInfo.environment
        let token = env["HF_TOKEN"] ?? env["HUGGING_FACE_HUB_TOKEN"]
        if let token, !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
    }"#;
    const PATCHED: &str = r#"    static func applyHubAuth(to request: inout URLRequest) {
        request.setValue(nil, forHTTPHeaderField: "Authorization")
    }"#;

    if source.contains(PATCHED) {
        return Ok(source.to_string());
    }

    if !source.contains(ORIGINAL) {
        return Err("HuggingFaceTransfer applyHubAuth not found");
    }

    Ok(source.replacen(ORIGINAL, PATCHED, 1))
}

#[cfg(test)]
mod tests {
    use super::patch_huggingface_transfer_source;

    const SOURCE: &str = r#"        applyHubAuth(to: &request)
        return request
    }

    static func applyHubAuth(to request: inout URLRequest) {
        let env = ProcessInfo.processInfo.environment
        let token = env["HF_TOKEN"] ?? env["HUGGING_FACE_HUB_TOKEN"]
        if let token, !token.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
    }
}
"#;

    #[test]
    fn forces_anonymous_hub_requests_for_built_in_models() {
        let patched = patch_huggingface_transfer_source(SOURCE).unwrap();

        assert!(patched.contains("request.setValue(nil, forHTTPHeaderField: \"Authorization\")"));
        assert!(!patched.contains("HF_TOKEN"));
        assert!(!patched.contains("Bearer"));
        assert!(patched.contains("applyHubAuth(to: &request)"));

        assert_eq!(
            patch_huggingface_transfer_source(&patched).unwrap(),
            patched
        );
    }

    #[test]
    fn anonymous_hub_patch_rejects_unknown_source() {
        let error = patch_huggingface_transfer_source("public enum Unrelated {}").unwrap_err();

        assert_eq!(error, "HuggingFaceTransfer applyHubAuth not found");
    }
}
