use hypr_api_env::PyannoteEnv;

#[derive(Clone)]
pub struct PyannoteConfig {
    pub api_key: String,
    pub api_base: String,
}

impl PyannoteConfig {
    pub fn new(env: &PyannoteEnv) -> Self {
        Self {
            api_key: env.pyannote_api_key.clone(),
            api_base: env.pyannote_api_base.clone(),
        }
    }

    pub fn client(
        &self,
    ) -> Result<hypr_pyannote_cloud::Client, Box<dyn std::error::Error + Send + Sync>> {
        hypr_pyannote_cloud::Client::builder(&self.api_key)
            .base_url(&self.api_base)
            .build()
    }
}
