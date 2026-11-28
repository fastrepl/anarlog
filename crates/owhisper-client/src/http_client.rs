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
            builder = builder
                .default_headers(headers)
                .redirect(reqwest::redirect::Policy::custom(|attempt| {
                    if attempt.previous().len() >= 10 {
                        return attempt.error("too many redirects");
                    }
                    let next = attempt.url();
                    let same_proxy_origin = attempt.previous().first().is_some_and(|origin| {
                        origin.host() == next.host()
                            && origin.port_or_known_default() == next.port_or_known_default()
                            && (next.scheme() == origin.scheme()
                                || (origin.scheme() == "http" && next.scheme() == "https"))
                    });
                    if same_proxy_origin && crate::adapter::is_anarlog_proxy(next.as_str()) {
                        attempt.follow()
                    } else {
                        attempt.error("redirect to non-proxy destination blocked")
                    }
                }));
        }
    }
    ClientBuilder::new(builder.build().unwrap_or_else(|_| reqwest::Client::new()))
        .with(TracingMiddleware::default())
        .build()
}
