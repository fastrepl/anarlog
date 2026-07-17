pub use hypr_api_env::OpenRouterEnv as Env;

pub struct ApiKey(pub String);

impl From<&Env> for ApiKey {
    fn from(env: &Env) -> Self {
        Self(env.openrouter_api_key.clone())
    }
}

impl From<&str> for ApiKey {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for ApiKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}
