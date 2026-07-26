mod client;
mod error;
mod types;

pub use client::{OpenStatusClient, OpenStatusClientBuilder};
pub use error::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn get_client() -> OpenStatusClient {
        OpenStatusClient::builder()
            .api_key(std::env::var("OPENSTATUS_API_KEY").unwrap_or_else(|_| "test-key".to_string()))
            .build()
    }

    #[test]
    fn test_client_builder() {
        let client = OpenStatusClient::builder()
            .api_key("test-api-key")
            .api_base("https://custom.api.com/v1")
            .build();

        assert_eq!(client.api_base().as_str(), "https://custom.api.com/v1");
    }

    #[test]
    fn test_status_report_status_serialization() {
        let status = StatusReportStatus::Investigating;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"investigating\"");

        let status: StatusReportStatus = serde_json::from_str("\"resolved\"").unwrap();
        assert_eq!(status, StatusReportStatus::Resolved);
    }

    #[test]
    fn test_create_status_report_request_serialization() {
        let req = CreateStatusReportRequest {
            title: "Test Report".to_string(),
            status: StatusReportStatus::Investigating,
            page_id: 123,
            message: "We are investigating an issue".to_string(),
            monitor_ids: Some(vec![1, 2, 3]),
            date: None,
        };

        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["title"], "Test Report");
        assert_eq!(json["status"], "investigating");
        assert_eq!(json["pageId"], 123);
        assert_eq!(json["message"], "We are investigating an issue");
        assert_eq!(json["monitorIds"], serde_json::json!([1, 2, 3]));
        assert!(json.get("date").is_none());
    }

    #[ignore]
    #[tokio::test]
    async fn test_list_incidents() {
        let client = get_client();
        let result = client.list_incidents().await;
        println!("incidents: {:?}", result);
    }

    #[ignore]
    #[tokio::test]
    async fn test_list_status_reports() {
        let client = get_client();
        let result = client.list_status_reports().await;
        println!("status_reports: {:?}", result);
    }
}
