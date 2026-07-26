#[derive(Debug, Clone)]
pub struct ProviderError {
    pub http_code: u16,
    pub message: String,
    pub provider_code: Option<String>,
}

impl ProviderError {
    pub fn new(http_code: u16, message: impl Into<String>) -> Self {
        Self {
            http_code,
            message: message.into(),
            provider_code: None,
        }
    }

    pub fn with_provider_code(mut self, code: impl Into<String>) -> Self {
        self.provider_code = Some(code.into());
        self
    }

    pub fn to_ws_close_code(&self) -> u16 {
        match self.http_code {
            400 => 4400,
            401 => 4401,
            402 => 4402,
            403 => 4403,
            404 => 4404,
            429 => 4429,
            500..=599 => 4500,
            _ => 4000,
        }
    }
}
