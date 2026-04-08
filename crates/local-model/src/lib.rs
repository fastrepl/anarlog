use std::path::{Path, PathBuf};

pub use hypr_am::AmModel;
pub use hypr_cactus_model::{CactusLlmModel, CactusModel, CactusModelSource, CactusSttModel};
use hypr_model_downloader::{DownloadableModel, Error, extract_zip};
pub use hypr_whisper_local_model::WhisperModel;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq)]
pub enum GgufLlmModel {
    Llama3p2_3bQ4,
    Gemma3_4bQ4,
    HyprLLM,
}

impl GgufLlmModel {
    pub fn file_name(&self) -> &str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "llm.gguf",
            GgufLlmModel::HyprLLM => "hypr-llm.gguf",
            GgufLlmModel::Gemma3_4bQ4 => "gemma-3-4b-it-Q4_K_M.gguf",
        }
    }

    pub fn model_url(&self) -> &str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/lmstudio-community/Llama-3.2-3B-Instruct-GGUF/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf"
            }
            GgufLlmModel::HyprLLM => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/yujonglee/hypr-llm-sm/model_q4_k_m.gguf"
            }
            GgufLlmModel::Gemma3_4bQ4 => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/unsloth/gemma-3-4b-it-GGUF/gemma-3-4b-it-Q4_K_M.gguf"
            }
        }
    }

    pub fn model_size(&self) -> u64 {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => 2019377440,
            GgufLlmModel::HyprLLM => 1107409056,
            GgufLlmModel::Gemma3_4bQ4 => 2489894016,
        }
    }

    pub fn model_checksum(&self) -> u32 {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => 2831308098,
            GgufLlmModel::HyprLLM => 4037351144,
            GgufLlmModel::Gemma3_4bQ4 => 2760830291,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GgufLlmModel::Llama3p2_3bQ4 => "Llama 3.2 3B Q4",
            GgufLlmModel::HyprLLM => "HyprLLM",
            GgufLlmModel::Gemma3_4bQ4 => "Gemma 3 4B Q4",
        }
    }

    pub fn description(&self) -> String {
        let mb = self.model_size() as f64 / (1024.0 * 1024.0);
        if mb >= 1024.0 {
            format!("{:.1} GB", mb / 1024.0)
        } else {
            format!("{:.0} MB", mb)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LocalModelKind {
    Stt,
    Llm,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Eq, Hash, PartialEq)]
#[serde(untagged)]
pub enum LocalModel {
    Cactus(CactusSttModel),
    Whisper(WhisperModel),
    Am(AmModel),
    GgufLlm(GgufLlmModel),
    CactusLlm(CactusLlmModel),
}

impl std::fmt::Display for LocalModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalModel::Cactus(model) => write!(f, "{model}"),
            LocalModel::Whisper(model) => write!(f, "whisper-{model}"),
            LocalModel::Am(model) => write!(f, "am-{model}"),
            LocalModel::GgufLlm(model) => write!(f, "llm-{model:?}"),
            LocalModel::CactusLlm(model) => write!(f, "{model}"),
        }
    }
}

impl LocalModel {
    pub fn all() -> Vec<LocalModel> {
        let mut models = vec![
            LocalModel::Whisper(WhisperModel::QuantizedTiny),
            LocalModel::Whisper(WhisperModel::QuantizedTinyEn),
            LocalModel::Whisper(WhisperModel::QuantizedBase),
            LocalModel::Whisper(WhisperModel::QuantizedBaseEn),
            LocalModel::Whisper(WhisperModel::QuantizedSmall),
            LocalModel::Whisper(WhisperModel::QuantizedSmallEn),
            LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo),
            LocalModel::Am(AmModel::ParakeetV2),
            LocalModel::Am(AmModel::ParakeetV3),
            LocalModel::Am(AmModel::WhisperLargeV3),
        ];

        models.extend(
            CactusSttModel::all()
                .iter()
                .cloned()
                .map(LocalModel::Cactus),
        );
        models.extend([
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4),
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM),
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4),
        ]);
        models.extend(
            CactusLlmModel::all()
                .iter()
                .cloned()
                .map(LocalModel::CactusLlm),
        );

        models
    }

    pub fn kind(&self) -> &'static str {
        match self {
            LocalModel::Whisper(_) => "stt-whisper",
            LocalModel::Am(_) => "stt-am",
            LocalModel::Cactus(_) => "stt-cactus",
            LocalModel::GgufLlm(_) => "llm",
            LocalModel::CactusLlm(_) => "llm-cactus",
        }
    }

    pub fn model_kind(&self) -> LocalModelKind {
        match self {
            LocalModel::Whisper(_) | LocalModel::Am(_) | LocalModel::Cactus(_) => {
                LocalModelKind::Stt
            }
            LocalModel::GgufLlm(_) | LocalModel::CactusLlm(_) => LocalModelKind::Llm,
        }
    }

    pub fn cli_name(&self) -> &'static str {
        match self {
            LocalModel::Whisper(WhisperModel::QuantizedTiny) => "whisper-tiny",
            LocalModel::Whisper(WhisperModel::QuantizedTinyEn) => "whisper-tiny-en",
            LocalModel::Whisper(WhisperModel::QuantizedBase) => "whisper-base",
            LocalModel::Whisper(WhisperModel::QuantizedBaseEn) => "whisper-base-en",
            LocalModel::Whisper(WhisperModel::QuantizedSmall) => "whisper-small",
            LocalModel::Whisper(WhisperModel::QuantizedSmallEn) => "whisper-small-en",
            LocalModel::Whisper(WhisperModel::QuantizedLargeTurbo) => "whisper-large-turbo",
            LocalModel::Am(AmModel::ParakeetV2) => "am-parakeet-v2",
            LocalModel::Am(AmModel::ParakeetV3) => "am-parakeet-v3",
            LocalModel::Am(AmModel::WhisperLargeV3) => "am-whisper-large-v3",
            LocalModel::Cactus(model) => match model {
                CactusSttModel::WhisperSmallInt4 => "cactus-whisper-small-int4",
                CactusSttModel::WhisperSmallInt4Apple => "cactus-whisper-small-int4-apple",
                CactusSttModel::WhisperSmallInt8 => "cactus-whisper-small-int8",
                CactusSttModel::WhisperSmallInt8Apple => "cactus-whisper-small-int8-apple",
                CactusSttModel::WhisperMediumInt4 => "cactus-whisper-medium-int4",
                CactusSttModel::WhisperMediumInt4Apple => "cactus-whisper-medium-int4-apple",
                CactusSttModel::WhisperMediumInt8 => "cactus-whisper-medium-int8",
                CactusSttModel::WhisperMediumInt8Apple => "cactus-whisper-medium-int8-apple",
                CactusSttModel::ParakeetCtc0_6bInt4 => "cactus-parakeet-ctc-0.6b-int4",
                CactusSttModel::ParakeetCtc0_6bInt4Apple => "cactus-parakeet-ctc-0.6b-int4-apple",
                CactusSttModel::ParakeetCtc0_6bInt8 => "cactus-parakeet-ctc-0.6b-int8",
                CactusSttModel::ParakeetCtc0_6bInt8Apple => "cactus-parakeet-ctc-0.6b-int8-apple",
                CactusSttModel::ParakeetTdt0_6bV3Int4 => "cactus-parakeet-tdt-0.6b-v3-int4",
                CactusSttModel::ParakeetTdt0_6bV3Int4Apple => {
                    "cactus-parakeet-tdt-0.6b-v3-int4-apple"
                }
                CactusSttModel::ParakeetTdt0_6bV3Int8 => "cactus-parakeet-tdt-0.6b-v3-int8",
                CactusSttModel::ParakeetTdt0_6bV3Int8Apple => {
                    "cactus-parakeet-tdt-0.6b-v3-int8-apple"
                }
            },
            LocalModel::GgufLlm(GgufLlmModel::Llama3p2_3bQ4) => "llm-llama3-2-3b-q4",
            LocalModel::GgufLlm(GgufLlmModel::HyprLLM) => "llm-hypr-llm",
            LocalModel::GgufLlm(GgufLlmModel::Gemma3_4bQ4) => "llm-gemma3-4b-q4",
            LocalModel::CactusLlm(model) => match model {
                CactusLlmModel::Gemma3_270m => "cactus-gemma3-270m",
                CactusLlmModel::Lfm2_350m => "cactus-lfm2-350m",
                CactusLlmModel::Qwen3_0_6b => "cactus-qwen3-0.6b",
                CactusLlmModel::Lfm2_700m => "cactus-lfm2-700m",
                CactusLlmModel::Gemma3_1b => "cactus-gemma3-1b",
                CactusLlmModel::Lfm2_5_1_2bInstruct => "cactus-lfm2.5-1.2b-instruct",
                CactusLlmModel::Qwen3_1_7b => "cactus-qwen3-1.7b",
                CactusLlmModel::Lfm2Vl450mApple => "cactus-lfm2-vl-450m-apple",
                CactusLlmModel::Lfm2_5Vl1_6bApple => "cactus-lfm2.5-vl-1.6b-apple",
            },
        }
    }

    pub fn install_path(&self, models_base: &Path) -> PathBuf {
        match self {
            LocalModel::Whisper(model) => models_base.join("stt").join(model.file_name()),
            LocalModel::Am(model) => models_base.join("stt").join(model.model_dir()),
            LocalModel::Cactus(model) => models_base
                .join("cactus")
                .join(CactusModel::Stt(model.clone()).dir_name()),
            LocalModel::GgufLlm(model) => models_base.join("llm").join(model.file_name()),
            LocalModel::CactusLlm(model) => models_base
                .join("cactus")
                .join(CactusModel::Llm(model.clone()).dir_name()),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            LocalModel::Whisper(model) => model.display_name().to_string(),
            LocalModel::Am(model) => model.display_name().to_string(),
            LocalModel::Cactus(model) => model.display_name().to_string(),
            LocalModel::GgufLlm(model) => model.display_name().to_string(),
            LocalModel::CactusLlm(model) => model.display_name().to_string(),
        }
    }

    pub fn description(&self) -> String {
        match self {
            LocalModel::Whisper(model) => model.description(),
            LocalModel::Am(model) => model.description().to_string(),
            LocalModel::Cactus(model) => model.description().to_string(),
            LocalModel::GgufLlm(model) => model.description(),
            LocalModel::CactusLlm(model) => model.description().to_string(),
        }
    }

    pub fn is_available_on_current_platform(&self) -> bool {
        let is_macos = cfg!(target_os = "macos");
        let is_apple_silicon = cfg!(target_arch = "aarch64") && cfg!(target_os = "macos");

        match self {
            LocalModel::Whisper(_) => is_macos,
            LocalModel::Am(_) => is_apple_silicon,
            LocalModel::Cactus(model) => {
                if model.is_apple() {
                    is_apple_silicon
                } else {
                    !is_apple_silicon
                }
            }
            LocalModel::GgufLlm(_) => cfg!(target_arch = "aarch64"),
            LocalModel::CactusLlm(model) => {
                if model.is_apple() {
                    is_apple_silicon
                } else {
                    !is_apple_silicon
                }
            }
        }
    }
}

impl DownloadableModel for GgufLlmModel {
    fn download_key(&self) -> String {
        format!("llm:{}", self.file_name())
    }

    fn download_url(&self) -> Option<String> {
        Some(self.model_url().to_string())
    }

    fn download_checksum(&self) -> Option<u32> {
        Some(self.model_checksum())
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        models_base.join("llm").join(self.file_name())
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        let path = models_base.join("llm").join(self.file_name());
        if !path.exists() {
            return Ok(false);
        }

        let actual =
            hypr_file::file_size(&path).map_err(|e| Error::OperationFailed(e.to_string()))?;
        Ok(actual == self.model_size())
    }

    fn finalize_download(&self, _downloaded_path: &Path, _models_base: &Path) -> Result<(), Error> {
        Ok(())
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        let path = models_base.join("llm").join(self.file_name());
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| Error::DeleteFailed(e.to_string()))?;
        }
        Ok(())
    }
}

impl DownloadableModel for LocalModel {
    fn download_key(&self) -> String {
        match self {
            LocalModel::Cactus(model) => {
                format!("cactus:{}", CactusModel::Stt(model.clone()).asset_id())
            }
            LocalModel::Whisper(model) => format!("whisper:{}", model.file_name()),
            LocalModel::Am(model) => format!("am:{}", model.model_dir()),
            LocalModel::GgufLlm(model) => model.download_key(),
            LocalModel::CactusLlm(model) => {
                format!("cactus:{}", CactusModel::Llm(model.clone()).asset_id())
            }
        }
    }

    fn download_url(&self) -> Option<String> {
        match self {
            LocalModel::Cactus(model) => CactusModel::Stt(model.clone())
                .model_url()
                .map(ToString::to_string),
            LocalModel::Whisper(model) => Some(model.model_url().to_string()),
            LocalModel::Am(model) => Some(model.tar_url().to_string()),
            LocalModel::GgufLlm(model) => model.download_url(),
            LocalModel::CactusLlm(model) => CactusModel::Llm(model.clone())
                .model_url()
                .map(ToString::to_string),
        }
    }

    fn download_checksum(&self) -> Option<u32> {
        match self {
            LocalModel::Cactus(model) => CactusModel::Stt(model.clone()).checksum(),
            LocalModel::Whisper(model) => Some(model.checksum()),
            LocalModel::Am(model) => Some(model.tar_checksum()),
            LocalModel::GgufLlm(model) => model.download_checksum(),
            LocalModel::CactusLlm(model) => CactusModel::Llm(model.clone()).checksum(),
        }
    }

    fn download_destination(&self, models_base: &Path) -> PathBuf {
        match self {
            LocalModel::Cactus(model) => models_base
                .join("cactus")
                .join(CactusModel::Stt(model.clone()).zip_name()),
            LocalModel::Whisper(model) => models_base.join("stt").join(model.file_name()),
            LocalModel::Am(model) => models_base
                .join("stt")
                .join(format!("{}.tar", model.model_dir())),
            LocalModel::GgufLlm(model) => model.download_destination(models_base),
            LocalModel::CactusLlm(model) => models_base
                .join("cactus")
                .join(CactusModel::Llm(model.clone()).zip_name()),
        }
    }

    fn is_downloaded(&self, models_base: &Path) -> Result<bool, Error> {
        match self {
            LocalModel::Cactus(model) => {
                let model_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Stt(model.clone()).dir_name());
                Ok(model_dir.is_dir()
                    && std::fs::read_dir(&model_dir)
                        .map(|mut d| d.next().is_some())
                        .unwrap_or(false))
            }
            LocalModel::Whisper(model) => {
                Ok(models_base.join("stt").join(model.file_name()).exists())
            }
            LocalModel::Am(model) => model
                .is_downloaded(models_base.join("stt"))
                .map_err(|e| Error::OperationFailed(e.to_string())),
            LocalModel::GgufLlm(model) => model.is_downloaded(models_base),
            LocalModel::CactusLlm(model) => {
                let model_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Llm(model.clone()).dir_name());
                Ok(model_dir.is_dir()
                    && std::fs::read_dir(&model_dir)
                        .map(|mut d| d.next().is_some())
                        .unwrap_or(false))
            }
        }
    }

    fn finalize_download(&self, downloaded_path: &Path, models_base: &Path) -> Result<(), Error> {
        match self {
            LocalModel::Cactus(model) => {
                let output_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Stt(model.clone()).dir_name());
                extract_zip(downloaded_path, output_dir)?;
                Ok(())
            }
            LocalModel::Whisper(_) => Ok(()),
            LocalModel::Am(model) => {
                let final_path = models_base.join("stt");
                model
                    .tar_unpack_and_cleanup(downloaded_path, &final_path)
                    .map_err(|e| Error::FinalizeFailed(e.to_string()))
            }
            LocalModel::GgufLlm(model) => model.finalize_download(downloaded_path, models_base),
            LocalModel::CactusLlm(model) => {
                let output_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Llm(model.clone()).dir_name());
                extract_zip(downloaded_path, output_dir)?;
                Ok(())
            }
        }
    }

    fn delete_downloaded(&self, models_base: &Path) -> Result<(), Error> {
        match self {
            LocalModel::Cactus(model) => {
                let model_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Stt(model.clone()).dir_name());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::Whisper(model) => {
                let model_path = models_base.join("stt").join(model.file_name());
                if model_path.exists() {
                    std::fs::remove_file(&model_path)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::Am(model) => {
                let model_dir = models_base.join("stt").join(model.model_dir());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
            LocalModel::GgufLlm(model) => model.delete_downloaded(models_base),
            LocalModel::CactusLlm(model) => {
                let model_dir = models_base
                    .join("cactus")
                    .join(CactusModel::Llm(model.clone()).dir_name());
                if model_dir.exists() {
                    std::fs::remove_dir_all(&model_dir)
                        .map_err(|e| Error::DeleteFailed(e.to_string()))?;
                }
                Ok(())
            }
        }
    }

    fn remove_destination_after_finalize(&self) -> bool {
        matches!(
            self,
            LocalModel::Cactus(_) | LocalModel::Am(_) | LocalModel::CactusLlm(_)
        )
    }
}
