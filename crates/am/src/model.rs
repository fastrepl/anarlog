// https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3
pub const PARAKEET_V3_LANGS: &[&str] = &[
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv", "mt",
    "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
];

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    strum::Display,
    Eq,
    Hash,
    PartialEq,
)]
pub enum AmModel {
    #[serde(rename = "am-parakeet-v2")]
    #[strum(serialize = "am-parakeet-v2")]
    ParakeetV2,
    #[serde(rename = "am-parakeet-v3")]
    #[strum(serialize = "am-parakeet-v3")]
    ParakeetV3,
    #[serde(rename = "am-whisper-large-v3")]
    #[strum(serialize = "am-whisper-large-v3")]
    WhisperLargeV3,
}

impl AmModel {
    pub fn repo_name(&self) -> &str {
        match self {
            AmModel::ParakeetV2 => "argmaxinc/parakeetkit-pro",
            AmModel::ParakeetV3 => "argmaxinc/parakeetkit-pro",
            AmModel::WhisperLargeV3 => "argmaxinc/whisperkit-pro",
        }
    }

    pub fn model_dir(&self) -> &str {
        match self {
            AmModel::ParakeetV2 => "nvidia_parakeet-v2_476MB",
            AmModel::ParakeetV3 => "nvidia_parakeet-v3_494MB",
            AmModel::WhisperLargeV3 => "openai_whisper-large-v3-v20240930_626MB",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            AmModel::ParakeetV2 => "Parakeet V2 (English)",
            AmModel::ParakeetV3 => "Parakeet V3 (Multilingual)",
            AmModel::WhisperLargeV3 => "Whisper Large V3 (Multilingual)",
        }
    }

    pub fn description(&self) -> &str {
        match self {
            AmModel::ParakeetV2 => "English only. Works best for English.",
            AmModel::ParakeetV3 => "English and European languages.",
            AmModel::WhisperLargeV3 => "Broad coverage of languages.",
        }
    }

    pub fn supported_languages(&self) -> Vec<hypr_language::Language> {
        use hypr_language::ISO639;

        match self {
            AmModel::ParakeetV2 => vec![ISO639::En.into()],
            AmModel::ParakeetV3 => PARAKEET_V3_LANGS
                .iter()
                .filter_map(|code| code.parse::<ISO639>().ok())
                .map(|iso| iso.into())
                .collect(),
            AmModel::WhisperLargeV3 => hypr_language::whisper_multilingual(),
        }
    }

    pub fn model_size_bytes(&self) -> u64 {
        match self {
            AmModel::ParakeetV2 => 476134400,
            AmModel::ParakeetV3 => 494141440,
            AmModel::WhisperLargeV3 => 625990656,
        }
    }

    pub fn is_downloaded(
        &self,
        base_dir: impl AsRef<std::path::Path>,
    ) -> Result<bool, crate::Error> {
        let model_path = base_dir.as_ref().join(self.model_dir());

        if !model_path.exists() {
            return Ok(false);
        }

        if !model_path.is_dir() {
            return Ok(false);
        }

        let entries = std::fs::read_dir(&model_path)?;
        let has_files = entries.count() > 0;

        Ok(has_files)
    }

    pub fn tar_url(&self) -> &str {
        match self {
            AmModel::ParakeetV2 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/nvidia_parakeet-v2_476MB.tar"
            }
            AmModel::ParakeetV3 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/nvidia_parakeet-v3_494MB.tar"
            }
            AmModel::WhisperLargeV3 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/openai_whisper-large-v3-v20240930_626MB.tar"
            }
        }
    }

    pub fn tar_checksum(&self) -> u32 {
        match self {
            AmModel::ParakeetV2 => 1906983049,
            AmModel::ParakeetV3 => 3016060540,
            AmModel::WhisperLargeV3 => 1964673816,
        }
    }

    pub fn tar_unpack_and_cleanup(
        &self,
        input_path: impl AsRef<std::path::Path>,
        output_path: impl AsRef<std::path::Path>,
    ) -> Result<(), crate::Error> {
        if !input_path.as_ref().exists() {
            return Err(crate::Error::TarFileNotFound);
        }

        extract_tar_file(&input_path, output_path)?;
        let _ = std::fs::remove_file(&input_path);
        Ok(())
    }

    pub async fn download<F: Fn(hypr_download_interface::DownloadProgress) + Send + Sync>(
        &self,
        output_path: impl AsRef<std::path::Path>,
        progress_callback: F,
    ) -> Result<(), crate::Error> {
        hypr_file::download_file_parallel(self.tar_url(), output_path, progress_callback).await?;
        Ok(())
    }
}

fn extract_tar_file(
    tar_path: impl AsRef<std::path::Path>,
    extract_to: impl AsRef<std::path::Path>,
) -> Result<(), crate::Error> {
    let file = std::fs::File::open(tar_path.as_ref())?;
    let mut archive = tar::Archive::new(file);
    std::fs::create_dir_all(extract_to.as_ref())?;
    archive.unpack(extract_to.as_ref())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tar_unpack_and_cleanup_skips_checksum_verification() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_tar = temp_dir.path().join("model.tar");
        let output_dir = temp_dir.path().join("out");

        let source_file = temp_dir.path().join("source.txt");
        std::fs::write(&source_file, b"weights").unwrap();

        {
            let tar_file = std::fs::File::create(&input_tar).unwrap();
            let mut builder = tar::Builder::new(tar_file);
            builder
                .append_path_with_name(&source_file, "fake-model/weights.bin")
                .unwrap();
            builder.finish().unwrap();
        }

        let model = AmModel::ParakeetV2;
        model
            .tar_unpack_and_cleanup(&input_tar, &output_dir)
            .unwrap();

        assert!(!input_tar.exists());
        assert!(output_dir.join("fake-model/weights.bin").exists());
    }
}
