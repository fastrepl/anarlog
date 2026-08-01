use owhisper_interface::stream;

use crate::{SoniqoModel, stream_response_from_text};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadState {
    pub status: String,
    pub current_file: Option<String>,
    pub progress_percent: Option<u8>,
    pub local_path: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTranscript {
    pub text: String,
    pub duration_seconds: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunks: Vec<FileTranscriptChunk>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub speaker_segments: Vec<DiarizationSegment>,
}

impl FileTranscript {
    pub fn new(text: String, duration_seconds: f64) -> Self {
        Self {
            text,
            duration_seconds,
            chunks: Vec::new(),
            speaker_segments: Vec::new(),
        }
    }

    pub fn from_chunks(chunks: Vec<FileTranscriptChunk>, duration_seconds: f64) -> Self {
        let text = chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        Self {
            text,
            duration_seconds,
            chunks,
            speaker_segments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTranscriptChunk {
    pub text: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiarizationSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub speaker_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptSource {
    Microphone,
    System,
}

impl TranscriptSource {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::System => "system",
        }
    }

    pub const fn channel_index(self) -> i32 {
        match self {
            Self::Microphone => 0,
            Self::System => 1,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePartial {
    pub source: String,
    pub text: String,
    pub is_final: bool,
}

impl LivePartial {
    pub fn source(&self) -> TranscriptSource {
        match self.source.as_str() {
            "system" => TranscriptSource::System,
            _ => TranscriptSource::Microphone,
        }
    }

    pub fn into_stream_response(
        self,
        model: SoniqoModel,
        start: f64,
        duration: f64,
    ) -> stream::StreamResponse {
        let source = self.source();
        stream_response_from_text(
            model,
            self.text,
            start,
            duration,
            self.is_final,
            &[source.channel_index(), 2],
        )
    }
}
