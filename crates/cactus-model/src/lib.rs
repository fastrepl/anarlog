#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CactusHealthStatus {
    Loading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CactusHealthResponse {
    pub service: String,
    pub live: bool,
    pub ready: bool,
    pub status: CactusHealthStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum CactusModelSource {
    Downloadable {
        url: &'static str,
        checksum: Option<u32>,
    },
    BundledResource {
        relative_path: &'static str,
    },
    Unavailable,
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    Eq,
    Hash,
    PartialEq,
    strum::EnumString,
    strum::Display,
)]
pub enum CactusSttModel {
    #[serde(rename = "cactus-whisper-small-int4")]
    #[strum(serialize = "cactus-whisper-small-int4")]
    WhisperSmallInt4,
    #[serde(rename = "cactus-whisper-small-int4-apple")]
    #[strum(serialize = "cactus-whisper-small-int4-apple")]
    WhisperSmallInt4Apple,
    #[serde(rename = "cactus-whisper-small-int8")]
    #[strum(serialize = "cactus-whisper-small-int8")]
    WhisperSmallInt8,
    #[serde(rename = "cactus-whisper-small-int8-apple")]
    #[strum(serialize = "cactus-whisper-small-int8-apple")]
    WhisperSmallInt8Apple,
    #[serde(rename = "cactus-whisper-medium-int4")]
    #[strum(serialize = "cactus-whisper-medium-int4")]
    WhisperMediumInt4,
    #[serde(rename = "cactus-whisper-medium-int4-apple")]
    #[strum(serialize = "cactus-whisper-medium-int4-apple")]
    WhisperMediumInt4Apple,
    #[serde(rename = "cactus-whisper-medium-int8")]
    #[strum(serialize = "cactus-whisper-medium-int8")]
    WhisperMediumInt8,
    #[serde(rename = "cactus-whisper-medium-int8-apple")]
    #[strum(serialize = "cactus-whisper-medium-int8-apple")]
    WhisperMediumInt8Apple,
    #[serde(rename = "cactus-parakeet-ctc-0.6b-int4")]
    #[strum(serialize = "cactus-parakeet-ctc-0.6b-int4")]
    ParakeetCtc0_6bInt4,
    #[serde(rename = "cactus-parakeet-ctc-0.6b-int4-apple")]
    #[strum(serialize = "cactus-parakeet-ctc-0.6b-int4-apple")]
    ParakeetCtc0_6bInt4Apple,
    #[serde(rename = "cactus-parakeet-ctc-0.6b-int8")]
    #[strum(serialize = "cactus-parakeet-ctc-0.6b-int8")]
    ParakeetCtc0_6bInt8,
    #[serde(rename = "cactus-parakeet-ctc-0.6b-int8-apple")]
    #[strum(serialize = "cactus-parakeet-ctc-0.6b-int8-apple")]
    ParakeetCtc0_6bInt8Apple,
    #[serde(rename = "cactus-parakeet-tdt-0.6b-v3-int4")]
    #[strum(serialize = "cactus-parakeet-tdt-0.6b-v3-int4")]
    ParakeetTdt0_6bV3Int4,
    #[serde(rename = "cactus-parakeet-tdt-0.6b-v3-int4-apple")]
    #[strum(serialize = "cactus-parakeet-tdt-0.6b-v3-int4-apple")]
    ParakeetTdt0_6bV3Int4Apple,
    #[serde(rename = "cactus-parakeet-tdt-0.6b-v3-int8")]
    #[strum(serialize = "cactus-parakeet-tdt-0.6b-v3-int8")]
    ParakeetTdt0_6bV3Int8,
    #[serde(rename = "cactus-parakeet-tdt-0.6b-v3-int8-apple")]
    #[strum(serialize = "cactus-parakeet-tdt-0.6b-v3-int8-apple")]
    ParakeetTdt0_6bV3Int8Apple,
}

impl CactusSttModel {
    pub fn all() -> &'static [CactusSttModel] {
        &[
            CactusSttModel::WhisperSmallInt4,
            CactusSttModel::WhisperSmallInt4Apple,
            CactusSttModel::WhisperSmallInt8,
            CactusSttModel::WhisperSmallInt8Apple,
            CactusSttModel::WhisperMediumInt4,
            CactusSttModel::WhisperMediumInt4Apple,
            CactusSttModel::WhisperMediumInt8,
            CactusSttModel::WhisperMediumInt8Apple,
            CactusSttModel::ParakeetCtc0_6bInt4,
            CactusSttModel::ParakeetCtc0_6bInt4Apple,
            CactusSttModel::ParakeetCtc0_6bInt8,
            CactusSttModel::ParakeetCtc0_6bInt8Apple,
            CactusSttModel::ParakeetTdt0_6bV3Int4,
            CactusSttModel::ParakeetTdt0_6bV3Int4Apple,
            CactusSttModel::ParakeetTdt0_6bV3Int8,
            CactusSttModel::ParakeetTdt0_6bV3Int8Apple,
        ]
    }

    pub fn is_apple(&self) -> bool {
        matches!(
            self,
            CactusSttModel::WhisperSmallInt4Apple
                | CactusSttModel::WhisperSmallInt8Apple
                | CactusSttModel::WhisperMediumInt4Apple
                | CactusSttModel::WhisperMediumInt8Apple
                | CactusSttModel::ParakeetCtc0_6bInt4Apple
                | CactusSttModel::ParakeetCtc0_6bInt8Apple
                | CactusSttModel::ParakeetTdt0_6bV3Int4Apple
                | CactusSttModel::ParakeetTdt0_6bV3Int8Apple
        )
    }

    pub fn is_cross_platform(&self) -> bool {
        matches!(
            self,
            CactusSttModel::WhisperSmallInt4
                | CactusSttModel::WhisperSmallInt8
                | CactusSttModel::ParakeetTdt0_6bV3Int4
                | CactusSttModel::ParakeetTdt0_6bV3Int8
        )
    }

    pub fn asset_id(&self) -> &str {
        match self {
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
            CactusSttModel::ParakeetTdt0_6bV3Int4Apple => "cactus-parakeet-tdt-0.6b-v3-int4-apple",
            CactusSttModel::ParakeetTdt0_6bV3Int8 => "cactus-parakeet-tdt-0.6b-v3-int8",
            CactusSttModel::ParakeetTdt0_6bV3Int8Apple => "cactus-parakeet-tdt-0.6b-v3-int8-apple",
        }
    }

    pub fn dir_name(&self) -> &str {
        match self {
            CactusSttModel::WhisperSmallInt4 => "whisper-small-int4",
            CactusSttModel::WhisperSmallInt4Apple => "whisper-small-int4-apple",
            CactusSttModel::WhisperSmallInt8 => "whisper-small-int8",
            CactusSttModel::WhisperSmallInt8Apple => "whisper-small-int8-apple",
            CactusSttModel::WhisperMediumInt4 => "whisper-medium-int4",
            CactusSttModel::WhisperMediumInt4Apple => "whisper-medium-int4-apple",
            CactusSttModel::WhisperMediumInt8 => "whisper-medium-int8",
            CactusSttModel::WhisperMediumInt8Apple => "whisper-medium-int8-apple",
            CactusSttModel::ParakeetCtc0_6bInt4 => "parakeet-ctc-0.6b-int4",
            CactusSttModel::ParakeetCtc0_6bInt4Apple => "parakeet-ctc-0.6b-int4-apple",
            CactusSttModel::ParakeetCtc0_6bInt8 => "parakeet-ctc-0.6b-int8",
            CactusSttModel::ParakeetCtc0_6bInt8Apple => "parakeet-ctc-0.6b-int8-apple",
            CactusSttModel::ParakeetTdt0_6bV3Int4 => "parakeet-tdt-0.6b-v3-int4",
            CactusSttModel::ParakeetTdt0_6bV3Int4Apple => "parakeet-tdt-0.6b-v3-int4-apple",
            CactusSttModel::ParakeetTdt0_6bV3Int8 => "parakeet-tdt-0.6b-v3-int8",
            CactusSttModel::ParakeetTdt0_6bV3Int8Apple => "parakeet-tdt-0.6b-v3-int8-apple",
        }
    }

    pub fn zip_name(&self) -> String {
        format!("{}.zip", self.dir_name())
    }

    pub fn source(&self) -> CactusModelSource {
        match self {
            CactusSttModel::WhisperSmallInt4 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int4.zip",
                checksum: Some(3458434299),
            },
            CactusSttModel::WhisperSmallInt4Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int4-apple.zip",
                checksum: Some(978654274),
            },
            CactusSttModel::WhisperSmallInt8 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int8.zip",
                checksum: Some(4195045602),
            },
            CactusSttModel::WhisperSmallInt8Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int8-apple.zip",
                checksum: Some(3401367684),
            },
            CactusSttModel::WhisperMediumInt4 | CactusSttModel::WhisperMediumInt4Apple => {
                CactusModelSource::Unavailable
            }
            CactusSttModel::WhisperMediumInt8 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-medium-int8.zip",
                checksum: Some(472491622),
            },
            CactusSttModel::WhisperMediumInt8Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-medium-int8-apple.zip",
                checksum: Some(3175773054),
            },
            CactusSttModel::ParakeetCtc0_6bInt4 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-ctc-0.6b-int4.zip",
                checksum: Some(110471502),
            },
            CactusSttModel::ParakeetCtc0_6bInt4Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-ctc-0.6b-int4-apple.zip",
                checksum: Some(3331532191),
            },
            CactusSttModel::ParakeetCtc0_6bInt8 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-ctc-0.6b-int8.zip",
                checksum: Some(1392408083),
            },
            CactusSttModel::ParakeetCtc0_6bInt8Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-ctc-0.6b-int8-apple.zip",
                checksum: Some(3465716349),
            },
            CactusSttModel::ParakeetTdt0_6bV3Int4 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-tdt-0.6b-v3-int4.zip",
                checksum: Some(4186460235),
            },
            CactusSttModel::ParakeetTdt0_6bV3Int4Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-tdt-0.6b-v3-int4-apple.zip",
                checksum: Some(215115681),
            },
            CactusSttModel::ParakeetTdt0_6bV3Int8 => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-tdt-0.6b-v3-int8.zip",
                checksum: Some(1102737485),
            },
            CactusSttModel::ParakeetTdt0_6bV3Int8Apple => CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/parakeet-tdt-0.6b-v3-int8-apple.zip",
                checksum: Some(1011398120),
            },
        }
    }

    pub fn model_url(&self) -> Option<&str> {
        match self.source() {
            CactusModelSource::Downloadable { url, .. } => Some(url),
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn checksum(&self) -> Option<u32> {
        match self.source() {
            CactusModelSource::Downloadable { checksum, .. } => checksum,
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            CactusSttModel::WhisperSmallInt4Apple
            | CactusSttModel::WhisperSmallInt8Apple
            | CactusSttModel::WhisperMediumInt4Apple
            | CactusSttModel::WhisperMediumInt8Apple
            | CactusSttModel::ParakeetCtc0_6bInt4Apple
            | CactusSttModel::ParakeetCtc0_6bInt8Apple
            | CactusSttModel::ParakeetTdt0_6bV3Int4Apple
            | CactusSttModel::ParakeetTdt0_6bV3Int8Apple => "Apple Neural Engine",
            _ => "",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            CactusSttModel::WhisperSmallInt4 => "Whisper Small (INT4)",
            CactusSttModel::WhisperSmallInt4Apple => "Whisper Small (INT4, Apple NPU)",
            CactusSttModel::WhisperSmallInt8 => "Whisper Small (INT8)",
            CactusSttModel::WhisperSmallInt8Apple => "Whisper Small (INT8, Apple NPU)",
            CactusSttModel::WhisperMediumInt4 => "Whisper Medium (INT4)",
            CactusSttModel::WhisperMediumInt4Apple => "Whisper Medium (INT4, Apple NPU)",
            CactusSttModel::WhisperMediumInt8 => "Whisper Medium (INT8)",
            CactusSttModel::WhisperMediumInt8Apple => "Whisper Medium (INT8, Apple NPU)",
            CactusSttModel::ParakeetCtc0_6bInt4 => "Parakeet CTC 0.6B (INT4)",
            CactusSttModel::ParakeetCtc0_6bInt4Apple => "Parakeet CTC 0.6B (INT4, Apple NPU)",
            CactusSttModel::ParakeetCtc0_6bInt8 => "Parakeet CTC 0.6B (INT8)",
            CactusSttModel::ParakeetCtc0_6bInt8Apple => "Parakeet CTC 0.6B (INT8, Apple NPU)",
            CactusSttModel::ParakeetTdt0_6bV3Int4 => "Parakeet TDT 0.6B v3 (INT4)",
            CactusSttModel::ParakeetTdt0_6bV3Int4Apple => "Parakeet TDT 0.6B v3 (INT4, Apple NPU)",
            CactusSttModel::ParakeetTdt0_6bV3Int8 => "Parakeet TDT 0.6B v3 (INT8)",
            CactusSttModel::ParakeetTdt0_6bV3Int8Apple => "Parakeet TDT 0.6B v3 (INT8, Apple NPU)",
        }
    }

    pub fn supported_languages(&self) -> Vec<hypr_language::Language> {
        match self {
            CactusSttModel::ParakeetCtc0_6bInt4
            | CactusSttModel::ParakeetCtc0_6bInt4Apple
            | CactusSttModel::ParakeetCtc0_6bInt8
            | CactusSttModel::ParakeetCtc0_6bInt8Apple
            | CactusSttModel::ParakeetTdt0_6bV3Int4
            | CactusSttModel::ParakeetTdt0_6bV3Int4Apple
            | CactusSttModel::ParakeetTdt0_6bV3Int8
            | CactusSttModel::ParakeetTdt0_6bV3Int8Apple => {
                vec!["en".parse().unwrap()]
            }
            _ => hypr_language::whisper_multilingual(),
        }
    }
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    Eq,
    Hash,
    PartialEq,
    strum::EnumString,
    strum::Display,
)]
pub enum CactusLlmModel {
    #[serde(rename = "cactus-gemma3-270m")]
    #[strum(serialize = "cactus-gemma3-270m")]
    Gemma3_270m,
    #[serde(rename = "cactus-lfm2-350m")]
    #[strum(serialize = "cactus-lfm2-350m")]
    Lfm2_350m,
    #[serde(rename = "cactus-qwen3-0.6b")]
    #[strum(serialize = "cactus-qwen3-0.6b")]
    Qwen3_0_6b,
    #[serde(rename = "cactus-lfm2-700m")]
    #[strum(serialize = "cactus-lfm2-700m")]
    Lfm2_700m,
    #[serde(rename = "cactus-gemma3-1b")]
    #[strum(serialize = "cactus-gemma3-1b")]
    Gemma3_1b,
    #[serde(rename = "cactus-lfm2.5-1.2b-instruct")]
    #[strum(serialize = "cactus-lfm2.5-1.2b-instruct")]
    Lfm2_5_1_2bInstruct,
    #[serde(rename = "cactus-qwen3-1.7b")]
    #[strum(serialize = "cactus-qwen3-1.7b")]
    Qwen3_1_7b,
    #[serde(rename = "cactus-lfm2-vl-450m-apple")]
    #[strum(serialize = "cactus-lfm2-vl-450m-apple")]
    Lfm2Vl450mApple,
    #[serde(rename = "cactus-lfm2.5-vl-1.6b-apple")]
    #[strum(serialize = "cactus-lfm2.5-vl-1.6b-apple")]
    Lfm2_5Vl1_6bApple,
}

impl CactusLlmModel {
    pub fn all() -> &'static [CactusLlmModel] {
        &[
            CactusLlmModel::Gemma3_270m,
            CactusLlmModel::Lfm2_350m,
            CactusLlmModel::Qwen3_0_6b,
            CactusLlmModel::Lfm2_700m,
            CactusLlmModel::Gemma3_1b,
            CactusLlmModel::Lfm2_5_1_2bInstruct,
            CactusLlmModel::Qwen3_1_7b,
            CactusLlmModel::Lfm2Vl450mApple,
            CactusLlmModel::Lfm2_5Vl1_6bApple,
        ]
    }

    pub fn is_apple(&self) -> bool {
        matches!(
            self,
            CactusLlmModel::Lfm2Vl450mApple | CactusLlmModel::Lfm2_5Vl1_6bApple
        )
    }

    pub fn asset_id(&self) -> &str {
        match self {
            CactusLlmModel::Gemma3_270m => "cactus-gemma3-270m",
            CactusLlmModel::Lfm2_350m => "cactus-lfm2-350m",
            CactusLlmModel::Qwen3_0_6b => "cactus-qwen3-0.6b",
            CactusLlmModel::Lfm2_700m => "cactus-lfm2-700m",
            CactusLlmModel::Gemma3_1b => "cactus-gemma3-1b",
            CactusLlmModel::Lfm2_5_1_2bInstruct => "cactus-lfm2.5-1.2b-instruct",
            CactusLlmModel::Qwen3_1_7b => "cactus-qwen3-1.7b",
            CactusLlmModel::Lfm2Vl450mApple => "cactus-lfm2-vl-450m-apple",
            CactusLlmModel::Lfm2_5Vl1_6bApple => "cactus-lfm2.5-vl-1.6b-apple",
        }
    }

    pub fn dir_name(&self) -> &str {
        match self {
            CactusLlmModel::Gemma3_270m => "gemma3-270m",
            CactusLlmModel::Lfm2_350m => "lfm2-350m",
            CactusLlmModel::Qwen3_0_6b => "qwen3-0.6b",
            CactusLlmModel::Lfm2_700m => "lfm2-700m",
            CactusLlmModel::Gemma3_1b => "gemma3-1b",
            CactusLlmModel::Lfm2_5_1_2bInstruct => "lfm2.5-1.2b-instruct",
            CactusLlmModel::Qwen3_1_7b => "qwen3-1.7b",
            CactusLlmModel::Lfm2Vl450mApple => "lfm2-vl-450m-apple",
            CactusLlmModel::Lfm2_5Vl1_6bApple => "lfm2.5-vl-1.6b-apple",
        }
    }

    pub fn zip_name(&self) -> String {
        format!("{}.zip", self.dir_name())
    }

    pub fn source(&self) -> CactusModelSource {
        match self {
            CactusLlmModel::Lfm2Vl450mApple => CactusModelSource::BundledResource {
                relative_path: "models/cactus/char-vlm/weight",
            },
            _ => CactusModelSource::Unavailable,
        }
    }

    pub fn model_url(&self) -> Option<&str> {
        match self.source() {
            CactusModelSource::Downloadable { url, .. } => Some(url),
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn checksum(&self) -> Option<u32> {
        match self.source() {
            CactusModelSource::Downloadable { checksum, .. } => checksum,
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            CactusLlmModel::Lfm2Vl450mApple | CactusLlmModel::Lfm2_5Vl1_6bApple => {
                "Apple Neural Engine"
            }
            _ => "",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            CactusLlmModel::Gemma3_270m => "Gemma 3 (270M)",
            CactusLlmModel::Lfm2_350m => "LFM2 (350M)",
            CactusLlmModel::Qwen3_0_6b => "Qwen3 (0.6B)",
            CactusLlmModel::Lfm2_700m => "LFM2 (700M)",
            CactusLlmModel::Gemma3_1b => "Gemma 3 (1B)",
            CactusLlmModel::Lfm2_5_1_2bInstruct => "LFM2.5 Instruct (1.2B)",
            CactusLlmModel::Qwen3_1_7b => "Qwen3 (1.7B)",
            CactusLlmModel::Lfm2Vl450mApple => "LFM2 VL (450M, Apple NPU)",
            CactusLlmModel::Lfm2_5Vl1_6bApple => "LFM2.5 VL (1.6B, Apple NPU)",
        }
    }
}

/// Unified enum for code that handles both STT and LLM Cactus models together.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, Hash, PartialEq)]
#[serde(untagged)]
pub enum CactusModel {
    Stt(CactusSttModel),
    Llm(CactusLlmModel),
}

impl CactusModel {
    pub fn all() -> Vec<CactusModel> {
        CactusSttModel::all()
            .iter()
            .cloned()
            .map(CactusModel::Stt)
            .chain(CactusLlmModel::all().iter().cloned().map(CactusModel::Llm))
            .collect()
    }

    pub fn stt() -> &'static [CactusSttModel] {
        CactusSttModel::all()
    }

    pub fn llm() -> &'static [CactusLlmModel] {
        CactusLlmModel::all()
    }

    pub fn is_apple(&self) -> bool {
        match self {
            CactusModel::Stt(m) => m.is_apple(),
            CactusModel::Llm(m) => m.is_apple(),
        }
    }

    pub fn asset_id(&self) -> &str {
        match self {
            CactusModel::Stt(m) => m.asset_id(),
            CactusModel::Llm(m) => m.asset_id(),
        }
    }

    pub fn dir_name(&self) -> &str {
        match self {
            CactusModel::Stt(m) => m.dir_name(),
            CactusModel::Llm(m) => m.dir_name(),
        }
    }

    pub fn zip_name(&self) -> String {
        match self {
            CactusModel::Stt(m) => m.zip_name(),
            CactusModel::Llm(m) => m.zip_name(),
        }
    }

    pub fn source(&self) -> CactusModelSource {
        match self {
            CactusModel::Stt(m) => m.source(),
            CactusModel::Llm(m) => m.source(),
        }
    }

    pub fn model_url(&self) -> Option<&str> {
        match self.source() {
            CactusModelSource::Downloadable { url, .. } => Some(url),
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn checksum(&self) -> Option<u32> {
        match self.source() {
            CactusModelSource::Downloadable { checksum, .. } => checksum,
            CactusModelSource::BundledResource { .. } | CactusModelSource::Unavailable => None,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            CactusModel::Stt(m) => m.description(),
            CactusModel::Llm(m) => m.description(),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            CactusModel::Stt(m) => m.display_name(),
            CactusModel::Llm(m) => m.display_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_llm_uses_resource_source() {
        assert_eq!(
            CactusLlmModel::Lfm2Vl450mApple.source(),
            CactusModelSource::BundledResource {
                relative_path: "models/cactus/char-vlm/weight",
            }
        );
    }

    #[test]
    fn downloadable_stt_reports_source_metadata() {
        assert_eq!(
            CactusSttModel::WhisperSmallInt4.source(),
            CactusModelSource::Downloadable {
                url: "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int4.zip",
                checksum: Some(3458434299),
            }
        );
    }

    #[test]
    fn bundled_models_do_not_expose_download_metadata() {
        assert_eq!(CactusLlmModel::Lfm2Vl450mApple.model_url(), None);
        assert_eq!(CactusLlmModel::Lfm2Vl450mApple.checksum(), None);
    }

    #[test]
    fn downloadable_models_keep_existing_metadata() {
        assert_eq!(
            CactusSttModel::WhisperSmallInt4.model_url(),
            Some(
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/Cactus-Compute/weights/whisper-small-int4.zip",
            )
        );
        assert_eq!(
            CactusSttModel::WhisperSmallInt4.checksum(),
            Some(3458434299)
        );
    }
}
