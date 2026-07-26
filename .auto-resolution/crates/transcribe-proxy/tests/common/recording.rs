use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::{Deserialize, Serialize};

use owhisper_client::Provider;

fn encode_optional_binary(data: &[u8]) -> String {
    if data.is_empty() {
        String::new()
    } else {
        BASE64.encode(data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    ServerToClient,
    ClientToServer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    Binary,
    Close { code: u16, reason: String },
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub direction: Direction,
    pub timestamp_ms: u64,
    pub kind: MessageKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
}

impl WsMessage {
    pub fn text(direction: Direction, timestamp_ms: u64, content: impl Into<String>) -> Self {
        Self {
            direction,
            timestamp_ms,
            kind: MessageKind::Text,
            content: content.into(),
        }
    }

    pub fn binary(direction: Direction, timestamp_ms: u64, data: &[u8]) -> Self {
        Self {
            direction,
            timestamp_ms,
            kind: MessageKind::Binary,
            content: BASE64.encode(data),
        }
    }

    pub fn close(
        direction: Direction,
        timestamp_ms: u64,
        code: u16,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            direction,
            timestamp_ms,
            kind: MessageKind::Close {
                code,
                reason: reason.into(),
            },
            content: String::new(),
        }
    }

    pub fn ping(direction: Direction, timestamp_ms: u64, data: &[u8]) -> Self {
        Self {
            direction,
            timestamp_ms,
            kind: MessageKind::Ping,
            content: encode_optional_binary(data),
        }
    }

    pub fn pong(direction: Direction, timestamp_ms: u64, data: &[u8]) -> Self {
        Self {
            direction,
            timestamp_ms,
            kind: MessageKind::Pong,
            content: encode_optional_binary(data),
        }
    }

    pub fn decode_binary(&self) -> Result<Vec<u8>, base64::DecodeError> {
        BASE64.decode(&self.content)
    }

    pub fn is_from_upstream(&self) -> bool {
        self.direction == Direction::ServerToClient
    }

    pub fn is_to_upstream(&self) -> bool {
        self.direction == Direction::ClientToServer
    }
}

#[derive(Debug, Clone, Default)]
pub struct WsRecording {
    pub messages: Vec<WsMessage>,
}

impl WsRecording {
    pub fn from_jsonl_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = BufReader::new(file);
        Self::from_reader(reader)
    }

    #[allow(dead_code)]
    pub fn from_jsonl_str(jsonl: &str) -> std::io::Result<Self> {
        Self::from_reader(jsonl.as_bytes())
    }

    pub fn from_reader<R: BufRead>(reader: R) -> std::io::Result<Self> {
        let mut messages = Vec::new();
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let msg: WsMessage = serde_json::from_str(trimmed)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            messages.push(msg);
        }
        Ok(Self { messages })
    }

    pub fn to_jsonl_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        for msg in &self.messages {
            let line = serde_json::to_string(msg)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }

    pub fn server_messages(&self) -> impl Iterator<Item = &WsMessage> {
        self.messages.iter().filter(|m| m.is_from_upstream())
    }

    pub fn push(&mut self, message: WsMessage) {
        self.messages.push(message);
    }

    pub fn transform<F>(mut self, f: F) -> Self
    where
        F: Fn(WsMessage) -> WsMessage,
    {
        self.messages = self.messages.into_iter().map(f).collect();
        self
    }
}

#[derive(Debug)]
pub struct WsRecorder {
    start_time: Instant,
    recording: WsRecording,
}

impl Default for WsRecorder {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            recording: WsRecording::default(),
        }
    }
}

impl WsRecorder {
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    pub fn record_text(&mut self, direction: Direction, content: impl Into<String>) {
        let msg = WsMessage::text(direction, self.elapsed_ms(), content);
        self.recording.push(msg);
    }

    #[allow(dead_code)]
    pub fn record_close(&mut self, direction: Direction, code: u16, reason: impl Into<String>) {
        let msg = WsMessage::close(direction, self.elapsed_ms(), code, reason);
        self.recording.push(msg);
    }

    pub fn recording(&self) -> &WsRecording {
        &self.recording
    }
}

#[derive(Clone)]
pub struct RecordingSession {
    recorder: Arc<Mutex<WsRecorder>>,
    provider: Provider,
}

impl RecordingSession {
    pub fn new(provider: Provider) -> Self {
        Self {
            recorder: Arc::new(Mutex::new(WsRecorder::default())),
            provider,
        }
    }

    pub fn record_server_text(&self, content: &str) {
        let mut recorder = self.recorder.lock().unwrap();
        recorder.record_text(Direction::ServerToClient, content);
    }

    pub fn save_to_file(&self, dir: impl AsRef<Path>, suffix: &str) -> std::io::Result<()> {
        let recorder = self.recorder.lock().unwrap();
        let recording = recorder.recording();

        let filename = format!(
            "{}_{}.jsonl",
            self.provider.to_string().to_lowercase(),
            suffix
        );
        let path = dir.as_ref().join(filename);

        recording.to_jsonl_file(path)
    }
}

pub struct RecordingOptions {
    pub enabled: bool,
    pub output_dir: Option<std::path::PathBuf>,
    pub suffix: String,
}

impl RecordingOptions {
    /// Check if recording is enabled via environment variable.
    /// Set RECORD_FIXTURES=1 to enable recording during live tests.
    pub fn from_env(suffix: impl Into<String>) -> Self {
        let enabled = std::env::var("RECORD_FIXTURES")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);

        if enabled {
            Self {
                enabled: true,
                output_dir: Some(
                    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests")
                        .join("fixtures"),
                ),
                suffix: suffix.into(),
            }
        } else {
            Self {
                enabled: false,
                output_dir: None,
                suffix: suffix.into(),
            }
        }
    }
}
