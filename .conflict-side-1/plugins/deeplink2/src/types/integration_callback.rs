use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct IntegrationCallbackSearch {
    pub integration_id: String,
    pub status: String,
    pub disconnected_connection_id: Option<String>,
    pub return_to: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disconnected_connection_id() {
        let search: IntegrationCallbackSearch = serde_qs::from_str(
            "integration_id=google-calendar&status=success&disconnected_connection_id=conn-personal",
        )
        .unwrap();

        assert_eq!(
            search.disconnected_connection_id.as_deref(),
            Some("conn-personal")
        );
    }
}
