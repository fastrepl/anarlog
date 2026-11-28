use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;

pub fn create_client() -> ClientWithMiddleware {
    create_client_with_headers(&[])
}

pub fn create_client_with_headers(extra_headers: &[(String, String)]) -> ClientWithMiddleware {
    let mut builder = reqwest::Client::builder();
    if !extra_headers.is_empty() {
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in extra_headers {
            if let (Ok(name), Ok(value)) = (
                reqwest::header::HeaderName::from_bytes(name.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(name, value);
            }
        }
        if !headers.is_empty() {
            builder = builder.default_headers(headers);
        }
    }
    ClientBuilder::new(builder.build().unwrap_or_else(|_| reqwest::Client::new()))
        .with(TracingMiddleware::default())
        .build()
}
