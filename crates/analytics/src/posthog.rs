use crate::{AnalyticsPayload, Error, PropertiesPayload};

use posthog::Event;
use posthog_core::event::InnerEvent;

#[derive(Clone)]
pub struct PosthogClient {
    client: reqwest::Client,
    api_key: String,
}

impl PosthogClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
        }
    }

    pub async fn event(&self, distinct_id: &str, payload: &AnalyticsPayload) -> Result<(), Error> {
        let mut e = Event::new(&payload.event, &distinct_id.to_string());
        e.set_timestamp(chrono::Utc::now().naive_utc());

        for (key, value) in &payload.props {
            let _ = e.insert_prop(key, value.clone());
        }

        let inner_event = InnerEvent::new(e, self.api_key.clone());

        self.client
            .post("https://us.i.posthog.com/i/v0/e/")
            .json(&inner_event)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn set_properties(
        &self,
        distinct_id: &str,
        payload: &PropertiesPayload,
    ) -> Result<(), Error> {
        let mut e = Event::new("$set", distinct_id);
        e.set_timestamp(chrono::Utc::now().naive_utc());

        if !payload.set.is_empty() {
            let _ = e.insert_prop("$set", serde_json::json!(payload.set));
        }

        if !payload.set_once.is_empty() {
            let _ = e.insert_prop("$set_once", serde_json::json!(payload.set_once));
        }

        if let Some(email) = &payload.email {
            let _ = e.insert_prop("$email", serde_json::json!(email));
        }

        let inner_event = InnerEvent::new(e, self.api_key.clone());

        self.client
            .post("https://us.i.posthog.com/i/v0/e/")
            .json(&inner_event)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}
