use crate::adapter::assemblyai;
use crate::adapter::cartesia;
use crate::adapter::deepgram;
use crate::adapter::elevenlabs;
use crate::adapter::soniox;
use crate::error_detection::ProviderError;

pub fn is_meta_model(model: &str) -> bool {
    matches!(model, "cloud" | "auto")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Auth {
    Header {
        name: &'static str,
        prefix: Option<&'static str>,
    },
    FirstMessage {
        field_name: &'static str,
    },
    SessionInit {
        header_name: &'static str,
    },
}

impl Auth {
    pub fn build_header(&self, api_key: &str) -> Option<(&'static str, String)> {
        match self {
            Auth::Header { name, prefix } => {
                let value = match prefix {
                    Some(p) => format!("{}{}", p, api_key),
                    None => api_key.to_string(),
                };
                Some((name, value))
            }
            Auth::FirstMessage { .. } | Auth::SessionInit { .. } => None,
        }
    }

    pub fn build_session_init_header(&self, api_key: &str) -> Option<(&'static str, String)> {
        match self {
            Auth::SessionInit { header_name } => Some((header_name, api_key.to_string())),
            _ => None,
        }
    }

    pub fn transform_first_message(&self, payload: String, api_key: &str) -> String {
        match self {
            Auth::FirstMessage { field_name } => {
                match serde_json::from_str::<serde_json::Value>(&payload) {
                    Ok(mut json) => {
                        if let Some(obj) = json.as_object_mut() {
                            obj.insert(
                                (*field_name).to_string(),
                                serde_json::Value::String(api_key.to_string()),
                            );
                        }
                        serde_json::to_string(&json).unwrap_or(payload)
                    }
                    Err(_) => payload,
                }
            }
            Auth::Header { .. } | Auth::SessionInit { .. } => payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::EnumString, strum::Display)]
pub enum Provider {
    #[strum(serialize = "aquavoice")]
    AquaVoice,
    #[strum(serialize = "cartesia")]
    Cartesia,
    #[strum(serialize = "deepgram")]
    Deepgram,
    #[strum(serialize = "assemblyai")]
    AssemblyAI,
    #[strum(serialize = "soniox")]
    Soniox,
    #[strum(serialize = "fireworks")]
    Fireworks,
    #[strum(serialize = "openai")]
    OpenAI,
    #[strum(serialize = "gladia")]
    Gladia,
    #[strum(serialize = "elevenlabs")]
    ElevenLabs,
    #[strum(serialize = "dashscope")]
    DashScope,
    #[strum(serialize = "mistral")]
    Mistral,
    #[strum(serialize = "pyannote")]
    Pyannote,
    #[strum(serialize = "cohere")]
    Cohere,
    #[strum(serialize = "aws_transcribe")]
    AwsTranscribe,
    #[strum(serialize = "azure_speech")]
    AzureSpeech,
    #[strum(serialize = "google_cloud")]
    GoogleCloud,
    #[strum(serialize = "google_generative_ai")]
    GoogleGenerativeAi,
    #[strum(serialize = "groq")]
    Groq,
    #[strum(serialize = "revai")]
    RevAi,
    #[strum(serialize = "speechmatics")]
    Speechmatics,
    #[strum(serialize = "together")]
    Together,
    #[strum(serialize = "xai")]
    Xai,
    #[strum(serialize = "smallestai")]
    SmallestAI,
}

impl Provider {
    const ALL: [Provider; 23] = [
        Self::AquaVoice,
        Self::Cartesia,
        Self::Deepgram,
        Self::AssemblyAI,
        Self::Soniox,
        Self::Fireworks,
        Self::OpenAI,
        Self::Gladia,
        Self::ElevenLabs,
        Self::DashScope,
        Self::Mistral,
        Self::Pyannote,
        Self::Cohere,
        Self::AwsTranscribe,
        Self::AzureSpeech,
        Self::GoogleCloud,
        Self::GoogleGenerativeAi,
        Self::Groq,
        Self::RevAi,
        Self::Speechmatics,
        Self::Together,
        Self::Xai,
        Self::SmallestAI,
    ];

    pub fn from_host(host: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.is_host(host))
    }

    pub fn auth(&self) -> Auth {
        match self {
            Self::AquaVoice => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::Cartesia => Auth::Header {
                name: "X-API-Key",
                prefix: None,
            },
            Self::Deepgram => Auth::Header {
                name: "Authorization",
                prefix: Some("Token "),
            },
            Self::AssemblyAI => Auth::Header {
                name: "Authorization",
                prefix: None,
            },
            Self::Fireworks => Auth::Header {
                name: "Authorization",
                prefix: None,
            },
            Self::OpenAI => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::Gladia => Auth::SessionInit {
                header_name: "x-gladia-key",
            },
            Self::Soniox => Auth::FirstMessage {
                field_name: "api_key",
            },
            Self::ElevenLabs => Auth::Header {
                name: "xi-api-key",
                prefix: None,
            },
            Self::DashScope => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::Mistral => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::Pyannote => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::Cohere => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::AzureSpeech => Auth::Header {
                name: "Ocp-Apim-Subscription-Key",
                prefix: None,
            },
            Self::GoogleCloud => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
            Self::GoogleGenerativeAi => Auth::Header {
                name: "x-goog-api-key",
                prefix: None,
            },
            Self::AwsTranscribe
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => Auth::Header {
                name: "Authorization",
                prefix: Some("Bearer "),
            },
        }
    }

    pub fn build_auth_header(&self, api_key: &str) -> Option<(&'static str, String)> {
        self.auth().build_header(api_key)
    }

    pub fn default_ws_url(&self) -> String {
        format!("wss://{}{}", self.default_ws_host(), self.ws_path())
    }

    pub fn default_api_host(&self) -> &'static str {
        match self {
            Self::AquaVoice => "api.aquavoice.com",
            Self::Cartesia => "api.cartesia.ai",
            Self::Deepgram => "api.deepgram.com",
            Self::AssemblyAI => "api.assemblyai.com",
            Self::Soniox => "api.soniox.com",
            Self::Fireworks => "api.fireworks.ai",
            Self::OpenAI => "api.openai.com",
            Self::Gladia => "api.gladia.io",
            Self::ElevenLabs => "api.elevenlabs.io",
            Self::DashScope => "dashscope-intl.aliyuncs.com",
            Self::Mistral => "api.mistral.ai",
            Self::Pyannote => "api.pyannote.ai",
            Self::Cohere => "api.cohere.com",
            Self::AwsTranscribe => "transcribe.us-east-1.amazonaws.com",
            Self::AzureSpeech => "api.cognitive.microsoft.com",
            Self::GoogleCloud => "speech.googleapis.com",
            Self::GoogleGenerativeAi => "generativelanguage.googleapis.com",
            Self::Groq => "api.groq.com",
            Self::RevAi => "api.rev.ai",
            Self::Speechmatics => "eu1.asr.api.speechmatics.com",
            Self::Together => "api.together.xyz",
            Self::Xai => "api.x.ai",
            Self::SmallestAI => "api.smallest.ai",
        }
    }

    pub fn default_ws_host(&self) -> &'static str {
        match self {
            Self::AquaVoice => "api.aquavoice.com",
            Self::Cartesia => "api.cartesia.ai",
            Self::Deepgram => "api.deepgram.com",
            Self::AssemblyAI => "streaming.assemblyai.com",
            Self::Soniox => "stt-rt.soniox.com",
            Self::Fireworks => "audio-streaming-v2.api.fireworks.ai",
            Self::OpenAI => "api.openai.com",
            Self::Gladia => "api.gladia.io",
            Self::ElevenLabs => "api.elevenlabs.io",
            Self::DashScope => "dashscope-intl.aliyuncs.com",
            Self::Mistral => "api.mistral.ai",
            Self::Pyannote => "api.pyannote.ai",
            Self::Cohere => "api.cohere.com",
            Self::AwsTranscribe => "transcribestreaming.us-east-1.amazonaws.com",
            Self::AzureSpeech => "api.cognitive.microsoft.com",
            Self::GoogleCloud => "speech.googleapis.com",
            Self::GoogleGenerativeAi => "generativelanguage.googleapis.com",
            Self::Groq => "api.groq.com",
            Self::RevAi => "api.rev.ai",
            Self::Speechmatics => "eu2.rt.speechmatics.com",
            Self::Together => "api.together.xyz",
            Self::Xai => "api.x.ai",
            Self::SmallestAI => "api.smallest.ai",
        }
    }

    pub fn ws_path(&self) -> &'static str {
        match self {
            Self::AquaVoice => "",
            Self::Cartesia => "/stt/turns/websocket",
            Self::Deepgram => "/v1/listen",
            Self::AssemblyAI => "/v3/ws",
            Self::Soniox => "/transcribe-websocket",
            Self::Fireworks => "/v1/audio/transcriptions/streaming",
            Self::OpenAI => "/v1/realtime",
            Self::Gladia => "/v2/live",
            Self::ElevenLabs => "/v1/speech-to-text/realtime",
            Self::DashScope => "/api-ws/v1/realtime",
            Self::Mistral => "/v1/audio/transcriptions/realtime",
            Self::Pyannote => "/v1/diarize",
            Self::Cohere => "",
            Self::Xai => "/v1/stt",
            Self::SmallestAI => crate::adapter::smallestai::LIVE_PATH,
            Self::GoogleGenerativeAi => crate::adapter::google_generative_ai::WS_PATH,
            Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together => "",
        }
    }

    pub fn default_api_url(&self) -> Option<&'static str> {
        match self {
            Self::AquaVoice => None,
            Self::Cartesia => None,
            Self::Deepgram => None,
            Self::AssemblyAI => Some("https://api.assemblyai.com/v2"),
            Self::Soniox => None,
            Self::Fireworks => None,
            Self::OpenAI => None,
            Self::Gladia => Some("https://api.gladia.io/v2/live"),
            Self::ElevenLabs => Some("https://api.elevenlabs.io/v1"),
            Self::DashScope => None,
            Self::Mistral => None,
            Self::Pyannote => Some("https://api.pyannote.ai/v1"),
            Self::Cohere => Some("https://api.cohere.com/v2"),
            Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::GoogleGenerativeAi
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => None,
        }
    }

    pub fn default_api_base(&self) -> &'static str {
        match self {
            Self::AquaVoice => "https://api.aquavoice.com/v1",
            Self::Cartesia => "https://api.cartesia.ai",
            Self::Deepgram => "https://api.deepgram.com/v1",
            Self::AssemblyAI => "https://api.assemblyai.com/v2",
            Self::Soniox => "https://api.soniox.com",
            Self::Fireworks => "https://api.fireworks.ai",
            Self::OpenAI => "https://api.openai.com/v1",
            Self::Gladia => "https://api.gladia.io/v2",
            Self::ElevenLabs => "https://api.elevenlabs.io",
            Self::DashScope => "https://dashscope-intl.aliyuncs.com",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::Pyannote => "https://api.pyannote.ai",
            Self::Cohere => "https://api.cohere.com/v2",
            Self::AwsTranscribe => "https://transcribe.us-east-1.amazonaws.com",
            Self::AzureSpeech => "https://api.cognitive.microsoft.com",
            Self::GoogleCloud => "https://speech.googleapis.com/v1",
            Self::GoogleGenerativeAi => "https://generativelanguage.googleapis.com/v1beta",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::RevAi => "https://api.rev.ai/speechtotext/v1",
            Self::Speechmatics => "https://eu1.asr.api.speechmatics.com/v2",
            Self::Together => "https://api.together.xyz/v1",
            Self::Xai => "https://api.x.ai/v1",
            Self::SmallestAI => "https://api.smallest.ai",
        }
    }

    pub fn domain(&self) -> &'static str {
        match self {
            Self::AquaVoice => "aquavoice.com",
            Self::Cartesia => "cartesia.ai",
            Self::Deepgram => "deepgram.com",
            Self::AssemblyAI => "assemblyai.com",
            Self::Soniox => "soniox.com",
            Self::Fireworks => "fireworks.ai",
            Self::OpenAI => "openai.com",
            Self::Gladia => "gladia.io",
            Self::ElevenLabs => "elevenlabs.io",
            Self::DashScope => "aliyuncs.com",
            Self::Mistral => "mistral.ai",
            Self::Pyannote => "pyannote.ai",
            Self::Cohere => "cohere.com",
            Self::AwsTranscribe => "amazonaws.com",
            Self::AzureSpeech => "cognitive.microsoft.com",
            Self::GoogleCloud => "speech.googleapis.com",
            Self::GoogleGenerativeAi => "generativelanguage.googleapis.com",
            Self::Groq => "groq.com",
            Self::RevAi => "rev.ai",
            Self::Speechmatics => "speechmatics.com",
            Self::Together => "together.xyz",
            Self::Xai => "x.ai",
            Self::SmallestAI => "smallest.ai",
        }
    }

    pub fn is_host(&self, host: &str) -> bool {
        if *self == Self::AzureSpeech
            && (host == "cognitiveservices.azure.com"
                || host.ends_with(".cognitiveservices.azure.com"))
        {
            return true;
        }
        let domain = self.domain();
        host == domain || host.ends_with(&format!(".{}", domain))
    }

    pub fn matches_url(&self, base_url: &str) -> bool {
        url::Url::parse(base_url)
            .ok()
            .and_then(|u| u.host_str().map(|h| self.is_host(h)))
            .unwrap_or(false)
    }

    pub fn from_url(base_url: &str) -> Option<Self> {
        url::Url::parse(base_url)
            .ok()
            .and_then(|u| u.host_str().and_then(Self::from_host))
    }

    pub fn env_key_name(&self) -> &'static str {
        match self {
            Self::AquaVoice => "AQUAVOICE_API_KEY",
            Self::Cartesia => "CARTESIA_API_KEY",
            Self::Deepgram => "DEEPGRAM_API_KEY",
            Self::AssemblyAI => "ASSEMBLYAI_API_KEY",
            Self::Soniox => "SONIOX_API_KEY",
            Self::Fireworks => "FIREWORKS_API_KEY",
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Gladia => "GLADIA_API_KEY",
            Self::ElevenLabs => "ELEVENLABS_API_KEY",
            Self::DashScope => "DASHSCOPE_API_KEY",
            Self::Mistral => "MISTRAL_API_KEY",
            Self::Pyannote => "PYANNOTE_API_KEY",
            Self::Cohere => "COHERE_API_KEY",
            Self::AwsTranscribe => "AWS_TRANSCRIBE_API_KEY",
            Self::AzureSpeech => "AZURE_SPEECH_API_KEY",
            Self::GoogleCloud => "GOOGLE_CLOUD_ACCESS_TOKEN",
            Self::GoogleGenerativeAi => "GEMINI_API_KEY",
            Self::Groq => "GROQ_API_KEY",
            Self::RevAi => "REVAI_ACCESS_TOKEN",
            Self::Speechmatics => "SPEECHMATICS_API_KEY",
            Self::Together => "TOGETHER_API_KEY",
            Self::Xai => "XAI_API_KEY",
            Self::SmallestAI => "SMALLEST_API_KEY",
        }
    }

    pub fn default_live_model(&self) -> &'static str {
        match self {
            Self::AquaVoice => crate::adapter::aquavoice::DEFAULT_MODEL,
            Self::Cartesia => "ink-2",
            Self::Deepgram => "nova-3",
            Self::Soniox => "stt-rt-v5",
            Self::AssemblyAI => "universal-3-5-pro",
            Self::Fireworks => "whisper-v3-turbo",
            Self::OpenAI => "gpt-live-transcribe",
            Self::Gladia => "solaria-1",
            Self::ElevenLabs => "scribe_v2_realtime",
            Self::DashScope => "qwen3-asr-flash-realtime",
            Self::Mistral => "voxtral-mini-transcribe-realtime-2602",
            Self::Pyannote => "parakeet-tdt-0.6b-v3",
            Self::Cohere => crate::adapter::cohere::DEFAULT_MODEL,
            Self::AwsTranscribe => "amazon-transcribe",
            Self::AzureSpeech => "fast-transcription",
            Self::GoogleCloud => "latest_long",
            Self::GoogleGenerativeAi => "gemini-3.5-transcribe-live",
            Self::Groq => "whisper-large-v3-turbo",
            Self::RevAi => "machine",
            Self::Speechmatics => "enhanced",
            Self::Together => "openai/whisper-large-v3",
            Self::Xai => "xai-stt",
            Self::SmallestAI => crate::adapter::smallestai::DEFAULT_MODEL,
        }
    }

    pub fn default_live_sample_rate(&self) -> u32 {
        match self {
            Self::AquaVoice => 16000,
            Self::OpenAI => 24000,
            Self::ElevenLabs
            | Self::DashScope
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => 16000,
            _ => 16000,
        }
    }

    pub fn default_batch_model(&self) -> &'static str {
        match self {
            Self::AquaVoice => crate::adapter::aquavoice::DEFAULT_MODEL,
            Self::Cartesia => cartesia::DEFAULT_MODEL,
            Self::Deepgram => "nova-3",
            Self::Soniox => "stt-async-v5",
            Self::AssemblyAI => "universal-3-5-pro",
            Self::Fireworks => "whisper-v3-turbo",
            Self::OpenAI => "gpt-transcribe",
            Self::Gladia => "solaria-1",
            Self::ElevenLabs => "scribe_v2",
            Self::DashScope => "qwen3-asr-flash-filetrans",
            Self::Mistral => "voxtral-mini-2602",
            Self::Pyannote => "parakeet-tdt-0.6b-v3",
            Self::Cohere => crate::adapter::cohere::DEFAULT_MODEL,
            Self::AwsTranscribe => "amazon-transcribe",
            Self::AzureSpeech => "fast-transcription",
            Self::GoogleCloud => "latest_long",
            Self::GoogleGenerativeAi => "gemini-3.5-transcribe",
            Self::Groq => "whisper-large-v3-turbo",
            Self::RevAi => "machine",
            Self::Speechmatics => "enhanced",
            Self::Together => "openai/whisper-large-v3",
            Self::Xai => "xai-stt",
            Self::SmallestAI => crate::adapter::smallestai::DEFAULT_MODEL,
        }
    }

    pub fn default_query_params(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Deepgram => &[("model", "nova-3-general"), ("mip_opt_out", "false")],
            Self::OpenAI => &[("intent", "transcription")],
            Self::AquaVoice
            | Self::DashScope
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => &[],
            _ => &[],
        }
    }

    pub fn supports_native_multichannel(&self) -> bool {
        match self {
            Self::Deepgram | Self::Gladia | Self::Xai => true,
            Self::AquaVoice
            | Self::Cartesia
            | Self::Soniox
            | Self::AssemblyAI
            | Self::Fireworks
            | Self::OpenAI
            | Self::ElevenLabs
            | Self::DashScope
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::GoogleGenerativeAi
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::SmallestAI => false,
        }
    }

    pub fn preserves_batch_channel_identity(&self) -> bool {
        matches!(self, Self::Deepgram | Self::AssemblyAI | Self::GoogleCloud)
    }

    pub fn control_message_types(&self) -> &'static [&'static str] {
        match self {
            Self::AquaVoice => &[],
            Self::Cartesia => &[],
            Self::Deepgram => &["KeepAlive", "CloseStream", "Finalize"],
            Self::AssemblyAI => &["Terminate"],
            Self::Soniox => &["keepalive", "finalize"],
            Self::Fireworks => &[],
            Self::OpenAI => &[],
            Self::Gladia => &[],
            Self::ElevenLabs => &["commit"],
            Self::SmallestAI => &["finalize", "close_stream"],
            Self::DashScope
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::GoogleGenerativeAi
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai => &[],
        }
    }

    pub fn session_init_config(&self, sample_rate: u32, channels: u8) -> Option<serde_json::Value> {
        match self {
            Self::Gladia => Some(serde_json::json!({
                "encoding": "wav/pcm",
                "sample_rate": sample_rate,
                "bit_depth": 16,
                "channels": channels,
                "messages_config": {
                    "receive_partial_transcripts": true,
                    "receive_final_transcripts": true
                },
                "realtime_processing": {
                    "words_accurate_timestamps": true
                }
            })),
            Self::AquaVoice
            | Self::Cartesia
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => None,
            _ => None,
        }
    }

    pub fn translate_control_message(
        &self,
        msg: &owhisper_interface::ControlMessage,
    ) -> Option<String> {
        use crate::adapter::RealtimeSttAdapter;
        use anlg_ws_client::client::Message;
        use owhisper_interface::ControlMessage;

        fn extract_text(msg: Message) -> Option<String> {
            match msg {
                Message::Text(t) => Some(t.to_string()),
                _ => None,
            }
        }

        fn from_adapter(adapter: &impl RealtimeSttAdapter, msg: &ControlMessage) -> Option<String> {
            match msg {
                ControlMessage::KeepAlive => adapter.keep_alive_message().and_then(extract_text),
                ControlMessage::Finalize => extract_text(adapter.finalize_message()),
                ControlMessage::CloseStream => None,
            }
        }

        match self {
            Self::Deepgram => from_adapter(&crate::adapter::DeepgramAdapter, msg),
            Self::AssemblyAI => from_adapter(&crate::adapter::AssemblyAIAdapter, msg),
            Self::Soniox => from_adapter(&crate::adapter::SonioxAdapter, msg),
            Self::Fireworks => from_adapter(&crate::adapter::FireworksAdapter, msg),
            Self::Gladia => from_adapter(&crate::adapter::GladiaAdapter, msg),
            Self::ElevenLabs => from_adapter(&crate::adapter::ElevenLabsAdapter, msg),
            Self::DashScope => from_adapter(&crate::adapter::DashScopeAdapter, msg),
            Self::Mistral => from_adapter(&crate::adapter::MistralAdapter::default(), msg),
            Self::AquaVoice => None,
            Self::Cartesia => from_adapter(&crate::adapter::CartesiaAdapter, msg),
            Self::OpenAI => from_adapter(&crate::adapter::OpenAIAdapter::default(), msg),
            Self::Pyannote => None,
            Self::Cohere => None,
            Self::Xai => from_adapter(&crate::adapter::XaiAdapter::default(), msg),
            Self::SmallestAI => from_adapter(&crate::adapter::SmallestAIAdapter, msg),
            Self::GoogleGenerativeAi => {
                from_adapter(&crate::adapter::GoogleGenerativeAiAdapter, msg)
            }
            Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together => None,
        }
    }

    pub fn detect_error(&self, data: &[u8]) -> Option<ProviderError> {
        match self {
            Self::Deepgram => deepgram::error::detect_error(data),
            Self::Soniox => soniox::error::detect_error(data),
            Self::ElevenLabs => elevenlabs::error::detect_error(data),
            Self::AssemblyAI => assemblyai::error::detect_error(data),
            Self::AquaVoice
            | Self::Cartesia
            | Self::Fireworks
            | Self::OpenAI
            | Self::Gladia
            | Self::DashScope
            | Self::Mistral
            | Self::Pyannote
            | Self::Cohere
            | Self::AwsTranscribe
            | Self::AzureSpeech
            | Self::GoogleCloud
            | Self::GoogleGenerativeAi
            | Self::Groq
            | Self::RevAi
            | Self::Speechmatics
            | Self::Together
            | Self::Xai
            | Self::SmallestAI => None,
        }
    }

    pub fn detect_any_error(data: &[u8]) -> Option<ProviderError> {
        Self::ALL.iter().find_map(|p| p.detect_error(data))
    }
}
