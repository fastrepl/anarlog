mod client;
mod error;
mod model;
mod types;

pub use client::*;
pub use error::*;
pub use model::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = Client::new("http://localhost:50060/v1");
        let status = client.status().await;
        println!("{:?}", status);
        client
            .init(InitRequest::new("").with_model(AmModel::ParakeetV2, "/tmp"))
            .await
            .unwrap();
        assert!(true);
    }
}
