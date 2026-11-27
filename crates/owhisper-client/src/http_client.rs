use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_tracing::TracingMiddleware;

pub fn create_client() -> ClientWithMiddleware {
    ClientBuilder::new(
        reqwest::Client::builder()
            .user_agent("Anarlog")
            .build()
            .expect("valid Anarlog user agent"),
    )
    .with(TracingMiddleware::default())
    .build()
}
