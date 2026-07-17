#[derive(
    Debug,
    Eq,
    Hash,
    PartialEq,
    Clone,
    strum::EnumString,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
)]

pub enum WhisperModel {
    #[serde(rename = "QuantizedTiny")]
    QuantizedTiny,
    #[serde(rename = "QuantizedTinyEn")]
    QuantizedTinyEn,
    #[serde(rename = "QuantizedBase")]
    QuantizedBase,
    #[serde(rename = "QuantizedBaseEn")]
    QuantizedBaseEn,
    #[serde(rename = "QuantizedSmall")]
    QuantizedSmall,
    #[serde(rename = "QuantizedSmallEn")]
    QuantizedSmallEn,
    #[serde(rename = "QuantizedLargeTurbo")]
    QuantizedLargeTurbo,
}

impl WhisperModel {
    pub fn file_name(&self) -> &str {
        match self {
            WhisperModel::QuantizedTiny => "ggml-tiny-q8_0.bin",
            WhisperModel::QuantizedTinyEn => "ggml-tiny.en-q8_0.bin",
            WhisperModel::QuantizedBase => "ggml-base-q8_0.bin",
            WhisperModel::QuantizedBaseEn => "ggml-base.en-q8_0.bin",
            WhisperModel::QuantizedSmall => "ggml-small-q8_0.bin",
            WhisperModel::QuantizedSmallEn => "ggml-small.en-q8_0.bin",
            WhisperModel::QuantizedLargeTurbo => "ggml-large-v3-turbo-q8_0.bin",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            WhisperModel::QuantizedTiny => "Whisper Tiny (Multilingual)",
            WhisperModel::QuantizedTinyEn => "Whisper Tiny (English)",
            WhisperModel::QuantizedBase => "Whisper Base (Multilingual)",
            WhisperModel::QuantizedBaseEn => "Whisper Base (English)",
            WhisperModel::QuantizedSmall => "Whisper Small (Multilingual)",
            WhisperModel::QuantizedSmallEn => "Whisper Small (English)",
            WhisperModel::QuantizedLargeTurbo => "Whisper Large Turbo (Multilingual)",
        }
    }

    pub fn model_url(&self) -> &str {
        match self {
            WhisperModel::QuantizedTiny => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-tiny-q8_0.bin"
            }
            WhisperModel::QuantizedTinyEn => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-tiny.en-q8_0.bin"
            }
            WhisperModel::QuantizedBase => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-base-q8_0.bin"
            }
            WhisperModel::QuantizedBaseEn => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-base.en-q8_0.bin"
            }
            WhisperModel::QuantizedSmall => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-small-q8_0.bin"
            }
            WhisperModel::QuantizedSmallEn => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-small.en-q8_0.bin"
            }
            WhisperModel::QuantizedLargeTurbo => {
                "https://hyprnote.s3.us-east-1.amazonaws.com/v0/ggerganov/whisper.cpp/main/ggml-large-v3-turbo-q8_0.bin"
            }
        }
    }

    pub fn description(&self) -> String {
        let mb = self.model_size_bytes() / (1024 * 1024);
        if mb >= 1024 {
            format!("{:.1} GB", mb as f64 / 1024.0)
        } else {
            format!("{} MB", mb)
        }
    }

    pub fn model_size_bytes(&self) -> u64 {
        match self {
            WhisperModel::QuantizedTiny => 43537433,
            WhisperModel::QuantizedTinyEn => 43550795,
            WhisperModel::QuantizedBase => 81768585,
            WhisperModel::QuantizedBaseEn => 81781811,
            WhisperModel::QuantizedSmall => 264464607,
            WhisperModel::QuantizedSmallEn => 264477561,
            WhisperModel::QuantizedLargeTurbo => 874188075,
        }
    }

    pub fn checksum(&self) -> u32 {
        match self {
            WhisperModel::QuantizedTiny => 1235175537,
            WhisperModel::QuantizedTinyEn => 230334082,
            WhisperModel::QuantizedBase => 4019564439,
            WhisperModel::QuantizedBaseEn => 2554759952,
            WhisperModel::QuantizedSmall => 3764849512,
            WhisperModel::QuantizedSmallEn => 3958576310,
            WhisperModel::QuantizedLargeTurbo => 3055274469,
        }
    }

    pub fn supported_languages(&self) -> Vec<hypr_language::Language> {
        match self {
            WhisperModel::QuantizedTinyEn
            | WhisperModel::QuantizedBaseEn
            | WhisperModel::QuantizedSmallEn => vec![hypr_language::ISO639::En.into()],
            WhisperModel::QuantizedTiny
            | WhisperModel::QuantizedBase
            | WhisperModel::QuantizedSmall
            | WhisperModel::QuantizedLargeTurbo => hypr_language::whisper_multilingual(),
        }
    }
}
