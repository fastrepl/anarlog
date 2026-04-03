include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

pub const DEFAULT_BASE_URL: &str = "https://api.pyannote.ai";

#[derive(Clone, Debug)]
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
}

impl Client {
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }
}

impl ClientBuilder {
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn build(self) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        let mut headers = reqwest::header::HeaderMap::new();

        let auth = format!("Bearer {}", self.api_key);
        let mut auth_value = reqwest::header::HeaderValue::from_str(&auth)?;
        auth_value.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Client::new_with_client(&self.base_url, client))
    }
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    let mut spec: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/openapi-filtered.gen.json"
    )))
    .expect("invalid pyannote openapi json");

    spec["openapi"] = serde_json::Value::String("3.1.0".to_string());

    serde_json::from_value(spec).expect("invalid pyannote openapi")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::{Client, DEFAULT_BASE_URL, openapi};

    #[test]
    fn builder_defaults_to_pyannote_api() {
        let client = Client::builder("test-key").build().unwrap();
        assert_eq!(client.baseurl, DEFAULT_BASE_URL);
    }

    #[tokio::test]
    async fn builder_applies_authorization_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/test"))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"status": "OK", "message": "connection successful"})),
            )
            .mount(&server)
            .await;

        let response = Client::builder("test-key")
            .base_url(server.uri())
            .build()
            .unwrap()
            .test_key()
            .await
            .unwrap()
            .into_inner();

        assert_eq!(response.status, "OK");
    }

    #[test]
    fn openapi_contains_v1_paths() {
        let doc = openapi();
        assert!(doc.paths.paths.contains_key("/v1/diarize"));
        assert!(doc.paths.paths.contains_key("/v1/jobs/{jobId}"));
    }
}
