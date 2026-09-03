const ANONYMOUS_DOWNLOAD_FAILED: &str =
    "Couldn't download this on-device model. Check your internet connection and try again.";

pub(crate) fn user_facing_download_error(error: &str) -> String {
    if is_huggingface_auth_error(error) {
        ANONYMOUS_DOWNLOAD_FAILED.to_string()
    } else {
        error.to_string()
    }
}

fn is_huggingface_auth_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("authentication required")
        || lower.contains("hugging face token")
        || lower.contains("huggingface token")
}

#[cfg(test)]
mod tests {
    use super::{ANONYMOUS_DOWNLOAD_FAILED, user_facing_download_error};

    #[test]
    fn rewrites_hub_auth_errors_without_asking_for_a_token() {
        assert_eq!(
            user_facing_download_error(
                "Failed to download: aufklarer/Parakeet-TDT-v3-CoreML-INT8-30s after 5 attempts (target: /Users/adam/Library/Caches/qwen3-speech-models/aufklarer/Parakeet-TDT-v3-CoreML-INT8-30s): Authentication required. Please provide a valid Hugging Face token."
            ),
            ANONYMOUS_DOWNLOAD_FAILED
        );
    }

    #[test]
    fn leaves_unrelated_download_errors_unchanged() {
        assert_eq!(
            user_facing_download_error("Downloaded Soniqo Parakeet Batch files are incomplete."),
            "Downloaded Soniqo Parakeet Batch files are incomplete."
        );
    }
}
