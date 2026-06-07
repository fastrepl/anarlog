use std::path::Path;

use crate::Error;

pub struct LlmServer {
    _private: (),
}

impl LlmServer {
    pub async fn start_with_model_path(
        _name: String,
        _file_path: impl AsRef<Path>,
    ) -> Result<Self, Error> {
        Err(Error::Other(
            "Local LLM is not supported on this platform".to_string(),
        ))
    }

    pub fn url(&self) -> &str {
        unreachable!()
    }

    pub fn exit_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        unreachable!()
    }

    pub async fn stop(self) {}
}
