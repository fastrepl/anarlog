use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl Content {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Content::Text(s) => Some(s),
            _ => None,
        }
    }
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

impl From<Vec<ContentPart>> for Content {
    fn from(parts: Vec<ContentPart>) -> Self {
        Content::Parts(parts)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlContent },
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: InputAudioContent },
    #[serde(rename = "input_video")]
    InputVideo { video_url: VideoUrlContent },
    #[serde(rename = "video_url")]
    VideoUrl { video_url: VideoUrlContent },
}

impl ContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    pub fn image_url(url: impl Into<String>) -> Self {
        Self::ImageUrl {
            image_url: ImageUrlContent {
                url: url.into(),
                detail: None,
            },
        }
    }

    pub fn image_url_with_detail(url: impl Into<String>, detail: ImageDetail) -> Self {
        Self::ImageUrl {
            image_url: ImageUrlContent {
                url: url.into(),
                detail: Some(detail),
            },
        }
    }

    pub fn input_audio(data: impl Into<String>, format: AudioFormat) -> Self {
        Self::InputAudio {
            input_audio: InputAudioContent {
                data: data.into(),
                format,
            },
        }
    }

    pub fn input_audio_from_bytes(data: &[u8], format: AudioFormat) -> Self {
        Self::InputAudio {
            input_audio: InputAudioContent {
                data: STANDARD.encode(data),
                format,
            },
        }
    }

    pub fn input_video(url: impl Into<String>) -> Self {
        Self::InputVideo {
            video_url: VideoUrlContent { url: url.into() },
        }
    }

    pub fn video_url(url: impl Into<String>) -> Self {
        Self::VideoUrl {
            video_url: VideoUrlContent { url: url.into() },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheControlTtl {
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<CacheControlTtl>,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            r#type: "ephemeral".into(),
            ttl: None,
        }
    }

    pub fn ephemeral_with_ttl(ttl: CacheControlTtl) -> Self {
        Self {
            r#type: "ephemeral".into(),
            ttl: Some(ttl),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrlContent {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<ImageDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAudioContent {
    pub data: String,
    pub format: AudioFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Wav,
    Mp3,
    Aiff,
    Aac,
    Ogg,
    Flac,
    M4a,
    Pcm16,
    Pcm24,
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "aiff" | "aif" => Some(Self::Aiff),
            "aac" => Some(Self::Aac),
            "ogg" => Some(Self::Ogg),
            "flac" => Some(Self::Flac),
            "m4a" => Some(Self::M4a),
            "pcm16" => Some(Self::Pcm16),
            "pcm24" => Some(Self::Pcm24),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoUrlContent {
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_format_serialization() {
        assert_eq!(serde_json::to_string(&AudioFormat::Wav).unwrap(), "\"wav\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Mp3).unwrap(), "\"mp3\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Aiff).unwrap(),
            "\"aiff\""
        );
        assert_eq!(serde_json::to_string(&AudioFormat::Aac).unwrap(), "\"aac\"");
        assert_eq!(serde_json::to_string(&AudioFormat::Ogg).unwrap(), "\"ogg\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Flac).unwrap(),
            "\"flac\""
        );
        assert_eq!(serde_json::to_string(&AudioFormat::M4a).unwrap(), "\"m4a\"");
        assert_eq!(
            serde_json::to_string(&AudioFormat::Pcm16).unwrap(),
            "\"pcm16\""
        );
        assert_eq!(
            serde_json::to_string(&AudioFormat::Pcm24).unwrap(),
            "\"pcm24\""
        );
    }

    #[test]
    fn audio_format_roundtrip() {
        let format: AudioFormat = serde_json::from_str("\"flac\"").unwrap();
        assert_eq!(serde_json::to_string(&format).unwrap(), "\"flac\"");
    }

    #[test]
    fn input_audio_serialization_roundtrip() {
        let part = ContentPart::input_audio("dGVzdA==", AudioFormat::Wav);
        let json = serde_json::to_string(&part).unwrap();
        let deserialized: ContentPart = serde_json::from_str(&json).unwrap();

        if let ContentPart::InputAudio { input_audio } = deserialized {
            assert_eq!(input_audio.data, "dGVzdA==");
            assert_eq!(
                serde_json::to_string(&input_audio.format).unwrap(),
                "\"wav\""
            );
        } else {
            panic!("expected InputAudio variant");
        }
    }

    #[test]
    fn input_audio_from_bytes_produces_valid_base64() {
        let raw = b"hello audio";
        let part = ContentPart::input_audio_from_bytes(raw, AudioFormat::Mp3);

        if let ContentPart::InputAudio { input_audio } = part {
            let decoded = STANDARD.decode(&input_audio.data).unwrap();
            assert_eq!(decoded, raw);
        } else {
            panic!("expected InputAudio variant");
        }
    }

    #[test]
    fn audio_format_from_extension() {
        assert!(matches!(
            AudioFormat::from_extension("wav"),
            Some(AudioFormat::Wav)
        ));
        assert!(matches!(
            AudioFormat::from_extension("MP3"),
            Some(AudioFormat::Mp3)
        ));
        assert!(matches!(
            AudioFormat::from_extension("aif"),
            Some(AudioFormat::Aiff)
        ));
        assert!(matches!(
            AudioFormat::from_extension("aiff"),
            Some(AudioFormat::Aiff)
        ));
        assert!(matches!(
            AudioFormat::from_extension("m4a"),
            Some(AudioFormat::M4a)
        ));
        assert!(matches!(
            AudioFormat::from_extension("pcm16"),
            Some(AudioFormat::Pcm16)
        ));
        assert!(AudioFormat::from_extension("unknown").is_none());
    }
}
