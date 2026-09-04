use std::collections::HashMap;
use std::sync::Arc;

mod error;

pub use error::*;

use posthog_rs::{ClientOptions, Event};
use sha2::{Digest, Sha256};

fn pseudonymous_id(scope: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"anarlog-analytics-v1\0");
    hasher.update(scope.as_bytes());
    hasher.update(b"\0");
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    let mut result = String::with_capacity(5 + digest.len() * 2);
    result.push_str("anon_");
    for byte in digest {
        result.push(char::from_digit(u32::from(byte >> 4), 16).unwrap());
        result.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap());
    }
    result
}

fn is_sensitive_property_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "email"
            | "address"
            | "condition"
            | "contact"
            | "customer_id"
            | "diagnosis"
            | "error"
            | "health"
            | "medical"
            | "name"
            | "user_id"
            | "account_id"
            | "team_id"
            | "session_id"
            | "file_id"
            | "note_id"
            | "meeting_id"
            | "transcript"
            | "title"
            | "prompt"
            | "content"
            | "body"
            | "text"
            | "message"
            | "participant"
            | "patient"
            | "request"
            | "response"
            | "speaker"
            | "ip"
            | "ip_address"
            | "url"
            | "path"
            | "query"
    ) || key.ends_with("_id")
        || key.ends_with("_email")
        || key.ends_with("_url")
        || key.ends_with("_path")
}

fn sanitized_properties(
    properties: &HashMap<String, serde_json::Value>,
) -> HashMap<String, serde_json::Value> {
    properties
        .iter()
        .filter(|(key, _)| !is_sensitive_property_key(key))
        .filter_map(|(key, value)| sanitized_value(value).map(|value| (key.clone(), value)))
        .collect()
}

fn sanitized_value(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            Some(value.clone())
        }
        serde_json::Value::String(value) => {
            is_safe_analytics_string(value).then(|| serde_json::Value::String(value.clone()))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .map(sanitized_value)
            .collect::<Option<Vec<_>>>()
            .map(serde_json::Value::Array),
        serde_json::Value::Object(values) => Some(serde_json::Value::Object(
            values
                .iter()
                .filter(|(key, _)| !is_sensitive_property_key(key))
                .filter_map(|(key, value)| sanitized_value(value).map(|value| (key.clone(), value)))
                .collect(),
        )),
    }
}

fn is_safe_analytics_string(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b'-' | b'.' | b':' | b'/' | b'{' | b'}' | b'<' | b'>'
                )
        })
        && !value.starts_with('/')
        && !value.contains("..")
        && !(value.len() >= 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
}

fn safe_event_name(value: &str) -> &str {
    if value.len() <= 64 && is_safe_analytics_string(value) {
        value
    } else {
        "analytics_event"
    }
}

#[derive(Clone)]
pub struct DeviceFingerprint(pub String);

#[derive(Clone)]
pub struct AuthenticatedUserId(pub String);

struct PosthogState {
    client: posthog_rs::Client,
}

struct LazyPosthogClient {
    api_key: String,
    state: tokio::sync::OnceCell<PosthogState>,
}

impl LazyPosthogClient {
    fn new(api_key: String) -> Self {
        Self {
            api_key,
            state: tokio::sync::OnceCell::new(),
        }
    }

    async fn get(&self) -> &PosthogState {
        self.state
            .get_or_init(|| {
                let key = self.api_key.clone();
                async move {
                    let client = posthog_rs::client(ClientOptions::from(key.as_str())).await;
                    PosthogState { client }
                }
            })
            .await
    }
}

#[derive(Clone)]
pub struct AnalyticsClient {
    posthog: Option<Arc<LazyPosthogClient>>,
}

#[derive(Default)]
pub struct AnalyticsClientBuilder {
    posthog_key: Option<String>,
}

impl AnalyticsClientBuilder {
    pub fn with_posthog(mut self, key: impl Into<String>) -> Self {
        self.posthog_key = Some(key.into());
        self
    }

    pub fn build(self) -> AnalyticsClient {
        let posthog = self
            .posthog_key
            .map(|key| Arc::new(LazyPosthogClient::new(key)));
        AnalyticsClient { posthog }
    }
}

impl AnalyticsClient {
    pub async fn event(
        &self,
        distinct_id: impl Into<String>,
        payload: AnalyticsPayload,
    ) -> Result<(), Error> {
        let distinct_id = pseudonymous_id("device", &distinct_id.into());

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new(safe_event_name(&payload.event), &distinct_id);
            for (key, value) in sanitized_properties(&payload.props) {
                let _ = event.insert_prop(key, value);
            }
            if let Some(groups) = &payload.groups {
                for (group_type, group_key) in groups {
                    let group_type = safe_event_name(group_type);
                    event.add_group(
                        group_type,
                        &pseudonymous_id(&format!("group-{group_type}"), group_key),
                    );
                }
            }
            state.client.capture(event).await?;
        } else {
            tracing::info!(
                event.name = safe_event_name(&payload.event),
                "analytics_backend_unavailable"
            );
        }

        Ok(())
    }

    pub async fn set_properties(
        &self,
        distinct_id: impl Into<String>,
        payload: PropertiesPayload,
    ) -> Result<(), Error> {
        let distinct_id = pseudonymous_id("device", &distinct_id.into());

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new("$set", &distinct_id);
            let set_props = sanitized_properties(&payload.set);
            if !set_props.is_empty() {
                let _ = event.insert_prop("$set", &set_props);
            }
            let set_once = sanitized_properties(&payload.set_once);
            if !set_once.is_empty() {
                let _ = event.insert_prop("$set_once", &set_once);
            }
            state.client.capture(event).await?;
        } else {
            tracing::info!("analytics_backend_unavailable");
        }

        Ok(())
    }

    pub async fn identify(
        &self,
        user_id: impl Into<String>,
        anon_distinct_id: impl Into<String>,
        payload: PropertiesPayload,
    ) -> Result<(), Error> {
        let user_id = pseudonymous_id("user", &user_id.into());
        let anon_distinct_id = pseudonymous_id("device", &anon_distinct_id.into());

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new("$identify", &user_id);
            let _ = event.insert_prop("$anon_distinct_id", &anon_distinct_id);
            if let Some(group) = &payload.group {
                let group_type = safe_event_name(&group.r#type);
                event.add_group(
                    group_type,
                    &pseudonymous_id(&format!("group-{group_type}"), &group.key),
                );
            }

            let set_props = sanitized_properties(&payload.set);
            if !set_props.is_empty() {
                let _ = event.insert_prop("$set", &set_props);
            }
            let set_once = sanitized_properties(&payload.set_once);
            if !set_once.is_empty() {
                let _ = event.insert_prop("$set_once", &set_once);
            }
            state.client.capture(event).await?;

            if let Some(group) = payload.group {
                let group_type = safe_event_name(&group.r#type);
                let mut event = Event::new("$groupidentify", &user_id);
                let _ = event.insert_prop("$group_type", group_type);
                let group_key = pseudonymous_id(&format!("group-{group_type}"), &group.key);
                let _ = event.insert_prop("$group_key", &group_key);
                let group_properties = sanitized_properties(&group.properties);
                let _ = event.insert_prop("$group_set", &group_properties);
                state.client.capture(event).await?;
            }
        } else {
            tracing::info!("analytics_backend_unavailable");
        }

        Ok(())
    }
}

pub trait ToAnalyticsPayload {
    fn to_analytics_payload(&self) -> AnalyticsPayload;

    fn to_analytics_properties(&self) -> Option<PropertiesPayload> {
        None
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AnalyticsPayload {
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub props: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct AnalyticsGroup {
    pub r#type: String,
    pub key: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PropertiesPayload {
    #[serde(default)]
    pub set: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub set_once: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<AnalyticsGroup>,
}

#[derive(Default)]
pub struct PropertiesPayloadBuilder {
    set: HashMap<String, serde_json::Value>,
    set_once: HashMap<String, serde_json::Value>,
}

impl PropertiesPayload {
    pub fn builder() -> PropertiesPayloadBuilder {
        PropertiesPayloadBuilder::default()
    }
}

impl PropertiesPayloadBuilder {
    pub fn set(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.set.insert(key.into(), value.into());
        self
    }

    pub fn set_once(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.set_once.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> PropertiesPayload {
        PropertiesPayload {
            set: self.set,
            set_once: self.set_once,
            email: None,
            user_id: None,
            group: None,
        }
    }
}

#[derive(Clone)]
pub struct AnalyticsPayloadBuilder {
    event: Option<String>,
    groups: HashMap<String, String>,
    props: HashMap<String, serde_json::Value>,
}

impl AnalyticsPayload {
    pub fn builder(event: impl Into<String>) -> AnalyticsPayloadBuilder {
        AnalyticsPayloadBuilder {
            event: Some(event.into()),
            groups: HashMap::new(),
            props: HashMap::new(),
        }
    }
}

impl AnalyticsPayloadBuilder {
    pub fn group(mut self, group_type: impl Into<String>, group_key: impl Into<String>) -> Self {
        self.groups.insert(group_type.into(), group_key.into());
        self
    }

    pub fn with(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.props.insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> AnalyticsPayload {
        if self.event.is_none() {
            panic!("'Event' is not specified");
        }

        AnalyticsPayload {
            event: self.event.unwrap(),
            groups: (!self.groups.is_empty()).then_some(self.groups),
            props: self.props,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_payload_builder_attaches_groups() {
        let payload = AnalyticsPayload::builder("test_event")
            .group("account", "account_123")
            .build();

        assert_eq!(
            payload.groups.unwrap().get("account"),
            Some(&"account_123".to_string())
        );
    }

    #[test]
    fn identifiers_are_stable_and_domain_separated() {
        let first = pseudonymous_id("user", "person@example.com");
        assert_eq!(first, pseudonymous_id("user", "person@example.com"));
        assert_ne!(first, pseudonymous_id("anonymous", "person@example.com"));
        assert!(!first.contains("person@example.com"));
    }

    #[test]
    fn sensitive_properties_are_removed_at_the_sink() {
        let properties = HashMap::from([
            ("email".to_string(), serde_json::json!("person@example.com")),
            ("recording_id".to_string(), serde_json::json!("recording-1")),
            (
                "request_url".to_string(),
                serde_json::json!("https://example.com/private"),
            ),
            (
                "arbitrary_copy".to_string(),
                serde_json::json!("Jane Doe has diabetes"),
            ),
            (
                "$set".to_string(),
                serde_json::json!({
                    "email": "person@example.com",
                    "platform": "desktop"
                }),
            ),
        ]);

        assert_eq!(
            sanitized_properties(&properties),
            HashMap::from([(
                "$set".to_string(),
                serde_json::json!({ "platform": "desktop" })
            )])
        );
    }

    #[ignore]
    #[tokio::test]
    async fn test_analytics() {
        let client = AnalyticsClientBuilder::default().build();
        let payload = AnalyticsPayload::builder("test_event")
            .with("key1", "value1")
            .with("key2", 2)
            .build();

        client.event("machine_id_123", payload).await.unwrap();
    }
}
