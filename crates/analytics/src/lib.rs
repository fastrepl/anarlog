use std::collections::HashMap;
use std::sync::Arc;

mod error;

pub use error::*;

use posthog_rs::{ClientOptions, Event};
use sha2::{Digest, Sha256};

/// Distinct id format shipped in desktop 1.4.21–1.4.26. Kept only so installs can
/// alias that id back onto the device fingerprint.
pub fn legacy_pseudonymous_device_id(fingerprint: &str) -> String {
    pseudonymous_id("device", fingerprint)
}

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
        .filter_map(|(key, value)| {
            let value = if key == "serving_revision" {
                value
                    .as_str()
                    .filter(|revision| {
                        revision.len() == 40
                            && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
                    .map(|revision| serde_json::Value::String(revision.to_string()))
            } else {
                sanitized_value(value)
            };
            value.map(|value| (key.clone(), value))
        })
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
    if value.len() <= 64 && is_safe_analytics_string(value.strip_prefix('$').unwrap_or(value)) {
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
    event_properties: HashMap<String, serde_json::Value>,
}

#[derive(Default)]
pub struct AnalyticsClientBuilder {
    posthog_key: Option<String>,
    event_properties: HashMap<String, serde_json::Value>,
}

impl AnalyticsClientBuilder {
    pub fn with_event_property(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.event_properties.insert(key.into(), value.into());
        self
    }

    pub fn with_posthog(mut self, key: impl Into<String>) -> Self {
        self.posthog_key = Some(key.into());
        self
    }

    pub fn build(self) -> AnalyticsClient {
        let posthog = self
            .posthog_key
            .map(|key| Arc::new(LazyPosthogClient::new(key)));
        AnalyticsClient {
            posthog,
            event_properties: sanitized_properties(&self.event_properties),
        }
    }
}

impl AnalyticsClient {
    async fn capture(&self, state: &PosthogState, mut event: Event) -> Result<(), Error> {
        for (key, value) in &self.event_properties {
            let _ = event.insert_prop(key, value);
        }
        state.client.capture(event).await?;
        Ok(())
    }

    pub async fn event(
        &self,
        distinct_id: impl Into<String>,
        payload: AnalyticsPayload,
    ) -> Result<(), Error> {
        let distinct_id = distinct_id.into();

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new(safe_event_name(&payload.event), &distinct_id);
            for (key, value) in sanitized_properties(&payload.props) {
                let _ = event.insert_prop(key, value);
            }
            if let Some(groups) = &payload.groups {
                for (group_type, group_key) in groups {
                    event.add_group(safe_event_name(group_type), group_key);
                }
            }
            self.capture(state, event).await?;
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
        let distinct_id = distinct_id.into();

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
            self.capture(state, event).await?;
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
        let user_id = user_id.into();
        let anon_distinct_id = anon_distinct_id.into();

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new("$identify", &user_id);
            let _ = event.insert_prop("$anon_distinct_id", &anon_distinct_id);
            if let Some(group) = &payload.group {
                event.add_group(safe_event_name(&group.r#type), &group.key);
            }

            let set_props = sanitized_properties(&payload.set);
            if !set_props.is_empty() {
                let _ = event.insert_prop("$set", &set_props);
            }
            let set_once = sanitized_properties(&payload.set_once);
            if !set_once.is_empty() {
                let _ = event.insert_prop("$set_once", &set_once);
            }
            self.capture(state, event).await?;

            if let Some(group) = payload.group {
                let group_type = safe_event_name(&group.r#type);
                let mut event = Event::new("$groupidentify", &user_id);
                let _ = event.insert_prop("$group_type", group_type);
                let _ = event.insert_prop("$group_key", &group.key);
                let group_properties = sanitized_properties(&group.properties);
                let _ = event.insert_prop("$group_set", &group_properties);
                self.capture(state, event).await?;
            }
        } else {
            tracing::info!("analytics_backend_unavailable");
        }

        Ok(())
    }

    /// Merges the person behind `other_distinct_id` into the person behind `distinct_id`,
    /// even when both are already identified. Only for ids known to belong to the same device.
    pub async fn merge_distinct_ids(
        &self,
        distinct_id: impl Into<String>,
        other_distinct_id: impl Into<String>,
    ) -> Result<(), Error> {
        let distinct_id = distinct_id.into();
        let other_distinct_id = other_distinct_id.into();

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new("$merge_dangerously", &distinct_id);
            let _ = event.insert_prop("alias", &other_distinct_id);
            self.capture(state, event).await?;
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

    #[tokio::test]
    async fn emitted_events_keep_runtime_attribution_without_private_or_person_properties() {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let revision = "2a8459e98f5d91537adc10550449c3e4454008e6";
        let mut client = AnalyticsClientBuilder::default()
            .with_event_property("serving_revision", revision)
            .with_event_property("app_version", "0.0.100")
            .with_event_property("service", "ai")
            .with_event_property("email", "person@example.com")
            .build();
        let lazy = LazyPosthogClient::new("test-key".into());
        lazy.state
            .get_or_init(|| async {
                PosthogState {
                    client: posthog_rs::client(("test-key", server.uri().as_str())).await,
                }
            })
            .await;
        client.posthog = Some(Arc::new(lazy));

        client
            .event(
                "test-user",
                AnalyticsPayload::builder("$ai_generation")
                    .with("serving_revision", "untrusted")
                    .with("service", "untrusted")
                    .with("$ai_model", "openai/gpt-6.1-sol")
                    .with("prompt", "private-note")
                    .build(),
            )
            .await
            .unwrap();
        let mut properties = PropertiesPayload::builder()
            .set("platform", "desktop")
            .set("email", "person@example.com")
            .set_once("channel", "stable")
            .build();
        client
            .set_properties("test-user", properties.clone())
            .await
            .unwrap();
        properties.group = Some(AnalyticsGroup {
            r#type: "workspace".into(),
            key: "test-group".into(),
            properties: HashMap::from([
                ("plan".into(), serde_json::json!("pro")),
                ("name".into(), serde_json::json!("Private workspace")),
            ]),
        });
        client
            .identify("test-user", "test-anon", properties)
            .await
            .unwrap();
        client
            .merge_distinct_ids("test-user", "test-other")
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let events: Vec<serde_json::Value> = requests
            .iter()
            .map(|request| serde_json::from_slice(&request.body).unwrap())
            .collect();
        assert_eq!(
            events
                .iter()
                .map(|event| event["event"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "$ai_generation",
                "$set",
                "$identify",
                "$groupidentify",
                "$merge_dangerously"
            ]
        );
        for event in &events {
            let properties = &event["properties"];
            assert_eq!(properties["serving_revision"], revision);
            assert_eq!(properties["app_version"], "0.0.100");
            assert_eq!(properties["service"], "ai");
            assert!(properties.get("email").is_none());
            assert!(properties.get("prompt").is_none());
            for key in ["$set", "$set_once", "$group_set"] {
                if let Some(person) = properties.get(key) {
                    assert!(person.get("serving_revision").is_none());
                    assert!(person.get("app_version").is_none());
                    assert!(person.get("service").is_none());
                    assert!(person.get("email").is_none());
                    assert!(person.get("name").is_none());
                }
            }
        }
        assert_eq!(events[0]["properties"]["$ai_model"], "openai/gpt-6.1-sol");
        assert_eq!(
            events[1]["properties"]["$set"],
            serde_json::json!({"platform": "desktop"})
        );
        assert_eq!(
            events[2]["properties"]["$set_once"],
            serde_json::json!({"channel": "stable"})
        );
        assert_eq!(
            events[3]["properties"]["$group_set"],
            serde_json::json!({"plan": "pro"})
        );
        assert_eq!(events[4]["properties"]["alias"], "test-other");
    }

    #[test]
    fn legacy_device_id_is_stable() {
        let id = legacy_pseudonymous_device_id("3f2a9c1d8b7e6f50");
        assert_eq!(id, legacy_pseudonymous_device_id("3f2a9c1d8b7e6f50"));
        assert!(id.starts_with("anon_"));
        assert_eq!(id.len(), 69);
        assert!(!id.contains("3f2a9c1d8b7e6f50"));
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
}
