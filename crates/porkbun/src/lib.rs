mod client;
mod dns;
mod error;

pub use client::*;
pub use dns::*;
pub use error::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_ping() {
        let client = PorkbunClientBuilder::default()
            .api_key("pk1_xxx")
            .secret_api_key("sk1_xxx")
            .build()
            .unwrap();

        let ip = client.ping().await.unwrap();
        assert!(!ip.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_dns_retrieve() {
        let client = PorkbunClientBuilder::default()
            .api_key("pk1_xxx")
            .secret_api_key("sk1_xxx")
            .build()
            .unwrap();

        let records = client.dns_retrieve("example.com", None).await.unwrap();
        assert!(!records.is_empty());
    }

    #[test]
    fn test_build_missing_api_key() {
        let result = PorkbunClientBuilder::default()
            .secret_api_key("sk1_xxx")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_build_missing_secret_api_key() {
        let result = PorkbunClientBuilder::default().api_key("pk1_xxx").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_build_defaults_api_base() {
        let client = PorkbunClientBuilder::default()
            .api_key("pk1_xxx")
            .secret_api_key("sk1_xxx")
            .build()
            .unwrap();
        assert_eq!(client.api_base.as_str(), "https://api.porkbun.com/");
    }
}
