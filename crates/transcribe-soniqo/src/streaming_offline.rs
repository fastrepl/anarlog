pub(crate) fn patch_parakeet_streaming_source(source: &str) -> Result<String, &'static str> {
    const SIGNATURE: &str = "    public static func fromPretrained(\n        modelId: String? = nil,\n        progressHandler: ((Double, String) -> Void)? = nil\n    ) async throws -> ParakeetStreamingASRModel {";
    const PATCHED_SIGNATURE: &str = "    public static func fromPretrained(\n        modelId: String? = nil,\n        offlineMode: Bool = false,\n        progressHandler: ((Double, String) -> Void)? = nil\n    ) async throws -> ParakeetStreamingASRModel {";
    const DOWNLOAD: &str = "            try await HuggingFaceDownloader.downloadWeights(\n                modelId: effectiveModelId,\n                to: cacheDir,\n                additionalFiles: [";
    const PATCHED_DOWNLOAD: &str = "            try await HuggingFaceDownloader.downloadWeights(\n                modelId: effectiveModelId,\n                to: cacheDir,\n                offlineMode: offlineMode,\n                additionalFiles: [";

    let mut patched = source.to_string();

    if !patched.contains(PATCHED_SIGNATURE) {
        if !patched.contains(SIGNATURE) {
            return Err("ParakeetStreamingASR fromPretrained signature not found");
        }
        patched = patched.replacen(SIGNATURE, PATCHED_SIGNATURE, 1);
    }

    if patched.contains(PATCHED_DOWNLOAD) {
        return Ok(patched);
    }

    if patched.contains(DOWNLOAD) {
        return Ok(patched.replacen(DOWNLOAD, PATCHED_DOWNLOAD, 1));
    }

    Err("ParakeetStreamingASR downloadWeights call not found")
}

#[cfg(test)]
mod tests {
    use super::patch_parakeet_streaming_source;

    const SOURCE: &str = r#"    public static func fromPretrained(
        modelId: String? = nil,
        progressHandler: ((Double, String) -> Void)? = nil
    ) async throws -> ParakeetStreamingASRModel {
        progressHandler?(0.0, "Downloading model...")
        do {
            try await HuggingFaceDownloader.downloadWeights(
                modelId: effectiveModelId,
                to: cacheDir,
                additionalFiles: [
                    "encoder.mlmodelc/**",
                ]
            ) { fraction in
                progressHandler?(fraction * 0.7, "Downloading model...")
            }
        }
    }
"#;

    #[test]
    fn adds_offline_mode_to_streaming_from_pretrained() {
        let patched = patch_parakeet_streaming_source(SOURCE).unwrap();

        assert!(patched.contains("offlineMode: Bool = false"));
        assert!(patched.contains("offlineMode: offlineMode"));
        assert!(!patched.contains(
            "        modelId: String? = nil,\n        progressHandler: ((Double, String) -> Void)? = nil"
        ));
    }

    #[test]
    fn streaming_offline_patch_is_idempotent() {
        let patched = patch_parakeet_streaming_source(SOURCE).unwrap();

        assert_eq!(patch_parakeet_streaming_source(&patched).unwrap(), patched);
    }

    #[test]
    fn streaming_offline_patch_rejects_unknown_source() {
        let error = patch_parakeet_streaming_source("public class Unrelated {}").unwrap_err();

        assert_eq!(
            error,
            "ParakeetStreamingASR fromPretrained signature not found"
        );
    }
}
