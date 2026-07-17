macro_rules! common_derives {
    ($item:item) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        $item
    };
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.len() <= 3 {
            f.write_str("[REDACTED]")
        } else {
            write!(f, "{}...[REDACTED]", &self.0[..3])
        }
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct ServerStatus {
        pub status: ServerStatusType,
        pub model: String,
        pub version: String,
        pub model_state: ModelState,
        pub verbose: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub download_progress: Option<DownloadProgress>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct DownloadProgress {
        pub progress_percentage: f64,
        pub is_downloading: bool,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum ServerStatusType {
        Ready,
        Initializing,
        Uninitialized,
        Unloaded,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum ModelState {
        Unloading,
        Unloaded,
        Loading,
        Loaded,
        Prewarming,
        Prewarmed,
        Downloading,
        Downloaded,
    }
}

common_derives! {
    #[serde(rename_all = "camelCase")]
    pub struct InitRequest {
        pub api_key: Secret,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub model_token: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub download_base: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub model_repo: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub model_folder: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub tokenizer_folder: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fast_load: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fast_load_encoder_compute_units: Option<ComputeUnits>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub fast_load_decoder_compute_units: Option<ComputeUnits>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub model_vad: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub verbose: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub custom_vocabulary: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub custom_vocabulary_model_folder: Option<String>,
    }
}

common_derives! {
    #[serde(rename_all = "lowercase")]
    pub enum ComputeUnits {
        Cpu,
        #[serde(rename = "cpuandgpu")]
        CpuAndGpu,
        #[serde(rename = "cpuandneuralengine")]
        CpuAndNeuralEngine,
        All,
    }
}

common_derives! {
    #[serde(tag = "status")]
    #[serde(rename_all = "snake_case")]
    pub enum InitResponse {
        Initializing {
            message: String,
            model: String,
            verbose: bool,
        },
        Error {
            message: String,
        },
        Timeout {
            message: String,
        },
        AlreadyInitialized {
            message: String,
        },
        NotInitialized {
            message: String,
        },
    }
}

impl InitResponse {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            InitResponse::Initializing { .. } | InitResponse::AlreadyInitialized { .. }
        )
    }

    pub fn message(&self) -> &str {
        match self {
            InitResponse::Initializing { message, .. }
            | InitResponse::Error { message }
            | InitResponse::Timeout { message }
            | InitResponse::AlreadyInitialized { message }
            | InitResponse::NotInitialized { message } => message,
        }
    }

    pub fn status_str(&self) -> &'static str {
        match self {
            InitResponse::Initializing { .. } => "initializing",
            InitResponse::Error { .. } => "error",
            InitResponse::Timeout { .. } => "timeout",
            InitResponse::AlreadyInitialized { .. } => "already_initialized",
            InitResponse::NotInitialized { .. } => "not_initialized",
        }
    }
}

common_derives! {
    pub struct ResetResponse {
        pub status: ResetStatus,
        pub message: String,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum ResetStatus {
        Reset,
    }
}

common_derives! {
    pub struct UnloadResponse {
        pub status: UnloadStatus,
        pub message: String,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "lowercase")]
    pub enum UnloadStatus {
        Unloaded,
    }
}

common_derives! {
    pub struct ShutdownResponse {
        pub status: ShutdownStatus,
        pub message: String,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum ShutdownStatus {
        ShuttingDown,
    }
}

common_derives! {
    pub struct ErrorResponse {
        pub status: ErrorStatus,
        pub message: String,
    }
}

common_derives! {
    #[derive(Eq, PartialEq)]
    #[serde(rename_all = "snake_case")]
    pub enum ErrorStatus {
        Error,
        Timeout,
        AlreadyInitialized,
        NotInitialized,
    }
}

impl std::fmt::Display for ErrorStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorStatus::Error => write!(f, "error"),
            ErrorStatus::Timeout => write!(f, "timeout"),
            ErrorStatus::AlreadyInitialized => write!(f, "already_initialized"),
            ErrorStatus::NotInitialized => write!(f, "not_initialized"),
        }
    }
}

common_derives! {
    pub struct GenericResponse {
        pub status: String,
        pub message: String,
    }
}
