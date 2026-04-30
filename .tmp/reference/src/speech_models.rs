use std::{
    collections::BTreeSet,
    env,
    path::{Path, PathBuf},
    process::Command,
};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptionMode {
    #[default]
    Realtime,
    Batch,
}

impl TranscriptionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Realtime => "realtime",
            Self::Batch => "batch",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpeechModelId {
    #[default]
    ParakeetStreaming,
    ParakeetBatch,
    Omnilingual,
    Qwen3Small,
    Qwen3Large,
}

impl SpeechModelId {
    pub const ALL: [Self; 5] = [
        Self::ParakeetStreaming,
        Self::ParakeetBatch,
        Self::Omnilingual,
        Self::Qwen3Small,
        Self::Qwen3Large,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ParakeetStreaming => "parakeetStreaming",
            Self::ParakeetBatch => "parakeetBatch",
            Self::Omnilingual => "omnilingual",
            Self::Qwen3Small => "qwen3Small",
            Self::Qwen3Large => "qwen3Large",
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Self::ParakeetStreaming => "parakeet-streaming",
            Self::ParakeetBatch => "parakeet-batch",
            Self::Omnilingual => "omnilingual",
            Self::Qwen3Small => "qwen3-small",
            Self::Qwen3Large => "qwen3-large",
        }
    }

    pub fn from_cli_name(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();

        match normalized.as_str() {
            "parakeet-streaming" | "parakeetstreaming" | "parakeet_streaming" => {
                Some(Self::ParakeetStreaming)
            }
            "parakeet-batch" | "parakeetbatch" | "parakeet_batch" => Some(Self::ParakeetBatch),
            "omnilingual" => Some(Self::Omnilingual),
            "qwen3-small" | "qwen3small" | "qwen3-small-0.6b" | "qwen3-0.6b" => {
                Some(Self::Qwen3Small)
            }
            "qwen3-large" | "qwen3large" | "qwen3-large-1.7b" | "qwen3-1.7b" => {
                Some(Self::Qwen3Large)
            }
            _ => match value {
                "parakeetStreaming" => Some(Self::ParakeetStreaming),
                "parakeetBatch" => Some(Self::ParakeetBatch),
                "omnilingual" => Some(Self::Omnilingual),
                "qwen3Small" => Some(Self::Qwen3Small),
                "qwen3Large" => Some(Self::Qwen3Large),
                _ => None,
            },
        }
    }

    pub fn batch_default() -> Self {
        Self::ParakeetBatch
    }

    pub fn is_batch_capable(self) -> bool {
        !matches!(self, Self::ParakeetStreaming)
    }

    pub fn supports_realtime(self) -> bool {
        matches!(self, Self::ParakeetStreaming | Self::ParakeetBatch)
    }

    pub fn uses_mlx(self) -> bool {
        matches!(self, Self::Qwen3Small | Self::Qwen3Large)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpeechModelSpec {
    pub id: SpeechModelId,
    pub label: &'static str,
    pub detail: &'static str,
    pub processing_mode: TranscriptionMode,
    pub repo: &'static str,
    pub languages_label: &'static str,
    pub size_label: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProfileState {
    pub chip_label: String,
    pub memory_gb: u64,
}

#[derive(Clone)]
pub struct ModelRecommendation {
    pub model_id: SpeechModelId,
    pub reason: String,
}

pub fn selected_model(mode: TranscriptionMode, batch_model_id: SpeechModelId) -> SpeechModelId {
    match effective_transcription_mode(mode, batch_model_id) {
        TranscriptionMode::Realtime => SpeechModelId::ParakeetStreaming,
        TranscriptionMode::Batch => normalize_batch_model(batch_model_id),
    }
}

pub fn effective_transcription_mode(
    mode: TranscriptionMode,
    batch_model_id: SpeechModelId,
) -> TranscriptionMode {
    let batch_model_id = normalize_batch_model(batch_model_id);

    if matches!(mode, TranscriptionMode::Realtime) && !batch_model_id.supports_realtime() {
        TranscriptionMode::Batch
    } else {
        mode
    }
}

pub fn normalize_batch_model(model_id: SpeechModelId) -> SpeechModelId {
    if model_id.is_batch_capable() {
        model_id
    } else {
        SpeechModelId::batch_default()
    }
}

pub fn speech_model_spec(model_id: SpeechModelId) -> SpeechModelSpec {
    match model_id {
        SpeechModelId::ParakeetStreaming => SpeechModelSpec {
            id: model_id,
            label: "Parakeet Streaming",
            detail: "Realtime CoreML model for live captions",
            processing_mode: TranscriptionMode::Realtime,
            repo: "aufklarer/Parakeet-EOU-120M-CoreML-INT8",
            languages_label: "25 European languages",
            size_label: "~120 MB",
        },
        SpeechModelId::ParakeetBatch => SpeechModelSpec {
            id: model_id,
            label: "Parakeet Batch",
            detail: "CoreML batch model for supported European languages",
            processing_mode: TranscriptionMode::Batch,
            repo: "aufklarer/Parakeet-TDT-v3-CoreML-INT8",
            languages_label: "25 European languages",
            size_label: "~500 MB",
        },
        SpeechModelId::Omnilingual => SpeechModelSpec {
            id: model_id,
            label: "Omnilingual",
            detail: "CoreML batch model with very broad language coverage",
            processing_mode: TranscriptionMode::Batch,
            repo: "aufklarer/Omnilingual-ASR-CTC-300M-CoreML-INT8-10s",
            languages_label: "1,672 languages",
            size_label: "~312 MB",
        },
        SpeechModelId::Qwen3Small => SpeechModelSpec {
            id: model_id,
            label: "Qwen3 0.6B",
            detail: "MLX batch model with stronger multilingual accuracy",
            processing_mode: TranscriptionMode::Batch,
            repo: "aufklarer/Qwen3-ASR-0.6B-MLX-4bit",
            languages_label: "52 languages",
            size_label: "~680 MB",
        },
        SpeechModelId::Qwen3Large => SpeechModelSpec {
            id: model_id,
            label: "Qwen3 1.7B",
            detail: "Largest local batch model for the best accuracy",
            processing_mode: TranscriptionMode::Batch,
            repo: "aufklarer/Qwen3-ASR-1.7B-MLX-8bit",
            languages_label: "52 languages",
            size_label: "~3.2 GB",
        },
    }
}

pub fn detect_device_profile() -> DeviceProfileState {
    let chip_label = read_sysctl_value("machdep.cpu.brand_string")
        .or_else(|| read_sysctl_value("hw.model"))
        .unwrap_or_else(|| "Apple Silicon".to_string());

    let memory_bytes = read_sysctl_value("hw.memsize")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(16 * 1024 * 1024 * 1024);
    let memory_gb = ((memory_bytes as f64) / 1024_f64.powi(3)).round().max(1.0) as u64;

    DeviceProfileState {
        chip_label,
        memory_gb,
    }
}

pub fn recommend_model(
    profile: &DeviceProfileState,
    languages: &[String],
    mode: TranscriptionMode,
) -> ModelRecommendation {
    if matches!(mode, TranscriptionMode::Realtime) {
        return ModelRecommendation {
            model_id: SpeechModelId::ParakeetStreaming,
            reason: "Realtime mode needs a streaming model, so unsigned uses Parakeet Streaming on CoreML.".to_string(),
        };
    }

    let normalized_languages = normalized_language_roots(languages);
    let unsupported_languages = unsupported_parakeet_languages(&normalized_languages);

    if !unsupported_languages.is_empty() {
        let joined = unsupported_languages
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        return ModelRecommendation {
            model_id: SpeechModelId::Omnilingual,
            reason: format!(
                "Omnilingual is the safer batch default for {joined} because it covers far more languages than the Parakeet family."
            ),
        };
    }

    if profile.memory_gb >= 32 {
        return ModelRecommendation {
            model_id: SpeechModelId::Qwen3Large,
            reason: format!(
                "{} with {} GB of unified memory has enough headroom for Qwen3 1.7B, which is the strongest batch option in this build.",
                profile.chip_label, profile.memory_gb
            ),
        };
    }

    if profile.memory_gb >= 16 {
        return ModelRecommendation {
            model_id: SpeechModelId::Qwen3Small,
            reason: format!(
                "{} with {} GB of unified memory is a good fit for Qwen3 0.6B: better accuracy than the smaller CoreML models without the footprint of 1.7B.",
                profile.chip_label, profile.memory_gb
            ),
        };
    }

    ModelRecommendation {
        model_id: SpeechModelId::ParakeetBatch,
        reason: format!(
            "{} with {} GB of unified memory is better matched to Parakeet Batch, which stays on CoreML and keeps memory use lower.",
            profile.chip_label, profile.memory_gb
        ),
    }
}

pub fn fallback_batch_model_without_mlx(languages: &[String]) -> SpeechModelId {
    if unsupported_parakeet_languages(&normalized_language_roots(languages)).is_empty() {
        SpeechModelId::ParakeetBatch
    } else {
        SpeechModelId::Omnilingual
    }
}

pub fn mlx_runtime_is_available() -> bool {
    mlx_runtime_search_paths()
        .into_iter()
        .any(|path| path.exists())
}

pub fn model_path_is_ready(model_id: SpeechModelId, path: &Path) -> bool {
    match model_id {
        SpeechModelId::ParakeetStreaming | SpeechModelId::ParakeetBatch => {
            required_regular_files_present(path, &["config.json", "vocab.json"])
                && compiled_coreml_model_ready(&path.join("encoder.mlmodelc"))
                && compiled_coreml_model_ready(&path.join("decoder.mlmodelc"))
                && compiled_coreml_model_ready(&path.join("joint.mlmodelc"))
        }
        SpeechModelId::Omnilingual => {
            required_regular_files_present(path, &["config.json", "tokenizer.model"])
                && package_directory_ready(&path.join("omnilingual-ctc-300m-int8.mlpackage"))
        }
        SpeechModelId::Qwen3Small | SpeechModelId::Qwen3Large => {
            required_regular_files_present(
                path,
                &["vocab.json", "merges.txt", "tokenizer_config.json"],
            ) && directory_contains_extension(path, "safetensors")
        }
    }
}

pub fn meeting_audio_file_name(meeting_id: &str) -> String {
    format!("meeting-{}.m4a", sanitize_path_component(meeting_id))
}

fn read_sysctl_value(name: &str) -> Option<String> {
    let output = Command::new("/usr/sbin/sysctl")
        .args(["-n", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn required_regular_files_present(path: &Path, relative_paths: &[&str]) -> bool {
    relative_paths
        .iter()
        .all(|relative_path| path.join(relative_path).is_file())
}

fn compiled_coreml_model_ready(path: &Path) -> bool {
    path.is_dir()
        && path.join("model.mil").is_file()
        && directory_contains_regular_file(&path.join("weights"))
}

fn package_directory_ready(path: &Path) -> bool {
    path.is_dir() && directory_contains_regular_file(path)
}

fn directory_contains_regular_file(path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };

    entries.flatten().any(|entry| {
        let entry_path = entry.path();
        match entry.file_type() {
            Ok(file_type) if file_type.is_file() => true,
            Ok(file_type) if file_type.is_dir() => directory_contains_regular_file(&entry_path),
            _ => false,
        }
    })
}

fn directory_contains_extension(path: &Path, extension: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };

    entries
        .flatten()
        .map(|entry| entry.path())
        .any(|entry_path| {
            entry_path.extension().and_then(|value| value.to_str()) == Some(extension)
        })
}

fn normalized_language_roots(languages: &[String]) -> Vec<String> {
    languages
        .iter()
        .map(|language| language.trim().to_lowercase())
        .filter(|language| !language.is_empty())
        .map(|language| {
            language
                .split(['-', '_'])
                .next()
                .unwrap_or(language.as_str())
                .to_string()
        })
        .collect()
}

fn unsupported_parakeet_languages(languages: &[String]) -> BTreeSet<String> {
    languages
        .iter()
        .filter(|language| !parakeet_supported_language_roots().contains(language.as_str()))
        .cloned()
        .collect()
}

fn parakeet_supported_language_roots() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr", "hr", "hu", "it", "lt", "lv",
        "mt", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "uk",
    ])
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '-',
        })
        .collect::<String>();

    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "meeting".to_string()
    } else {
        trimmed.to_string()
    }
}

fn mlx_runtime_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let Ok(current_exe) = env::current_exe() else {
        return paths;
    };
    let Some(binary_dir) = current_exe.parent() else {
        return paths;
    };

    paths.push(binary_dir.join("mlx.metallib"));
    paths.push(binary_dir.join("Resources/default.metallib"));
    paths.push(binary_dir.join("Resources/mlx.metallib"));
    paths.push(binary_dir.join("mlx-swift_Cmlx.bundle/default.metallib"));

    if let Some(contents_dir) = binary_dir.parent() {
        paths.push(contents_dir.join("Resources/default.metallib"));
        paths.push(contents_dir.join("Resources/mlx.metallib"));
        paths.push(contents_dir.join("Resources/mlx-swift_Cmlx.bundle/default.metallib"));
    }

    if let Some(app_dir) = current_exe
        .ancestors()
        .find(|ancestor| ancestor.extension().and_then(|value| value.to_str()) == Some("app"))
    {
        paths.push(app_dir.join("mlx-swift_Cmlx.bundle/default.metallib"));
        paths.push(app_dir.join("Contents/Resources/mlx-swift_Cmlx.bundle/default.metallib"));
    }

    paths
}
