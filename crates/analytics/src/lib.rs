use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

mod error;
mod outlit;

pub use error::*;

use outlit::OutlitClient;
use posthog_rs::{ClientOptions, Event};

pub use posthog_rs::FlagValue;

#[derive(Clone)]
pub struct DeviceFingerprint(pub String);

#[derive(Clone)]
pub struct AuthenticatedUserId(pub String);

struct PosthogState {
    client: posthog_rs::Client,
    local_eval: Option<LocalEvalState>,
}

struct LocalEvalState {
    evaluator: posthog_rs::LocalEvaluator,
    _poller: posthog_rs::AsyncFlagPoller,
}

struct LazyPosthogClient {
    api_key: String,
    personal_api_key: Option<String>,
    state: tokio::sync::OnceCell<PosthogState>,
}

impl LazyPosthogClient {
    fn new(api_key: String, personal_api_key: Option<String>) -> Self {
        Self {
            api_key,
            personal_api_key,
            state: tokio::sync::OnceCell::new(),
        }
    }

    async fn get(&self) -> &PosthogState {
        self.state
            .get_or_init(|| {
                let key = self.api_key.clone();
                let personal_key = self.personal_api_key.clone();
                async move {
                    let client = posthog_rs::client(ClientOptions::from(key.as_str())).await;

                    let local_eval = if let Some(personal_key) = personal_key {
                        let cache = posthog_rs::FlagCache::new();
                        let config = posthog_rs::LocalEvaluationConfig {
                            personal_api_key: personal_key,
                            project_api_key: key,
                            api_host: "https://us.i.posthog.com".to_string(),
                            poll_interval: Duration::from_secs(30),
                            request_timeout: Duration::from_secs(10),
                        };
                        let mut poller = posthog_rs::AsyncFlagPoller::new(config, cache.clone());
                        let _ = poller.load_flags().await;
                        poller.start().await;
                        let evaluator = posthog_rs::LocalEvaluator::new(cache);
                        Some(LocalEvalState {
                            evaluator,
                            _poller: poller,
                        })
                    } else {
                        None
                    };

                    PosthogState { client, local_eval }
                }
            })
            .await
    }
}

#[derive(Clone)]
pub struct AnalyticsClient {
    posthog: Option<Arc<LazyPosthogClient>>,
    outlit: Option<OutlitClient>,
}

#[derive(Default)]
pub struct AnalyticsClientBuilder {
    posthog_key: Option<String>,
    posthog_personal_key: Option<String>,
    outlit: Option<OutlitClient>,
}

impl AnalyticsClientBuilder {
    pub fn with_posthog(mut self, key: impl Into<String>) -> Self {
        self.posthog_key = Some(key.into());
        self
    }

    pub fn with_local_evaluation(mut self, personal_api_key: impl Into<String>) -> Self {
        self.posthog_personal_key = Some(personal_api_key.into());
        self
    }

    pub fn with_outlit(mut self, key: impl Into<String>) -> Self {
        self.outlit = OutlitClient::new(key);
        self
    }

    pub fn build(self) -> AnalyticsClient {
        let posthog = self
            .posthog_key
            .map(|key| Arc::new(LazyPosthogClient::new(key, self.posthog_personal_key)));
        AnalyticsClient {
            posthog,
            outlit: self.outlit,
        }
    }
}

impl AnalyticsClient {
    pub async fn event(
        &self,
        distinct_id: impl Into<String>,
        payload: AnalyticsPayload,
    ) -> Result<(), Error> {
        let distinct_id = distinct_id.into();

        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            let mut event = Event::new(&payload.event, &distinct_id);
            for (key, value) in &payload.props {
                let _ = event.insert_prop(key, value);
            }
            state.client.capture(event).await?;
        } else {
            tracing::info!("event: {:?}", payload);
        }

        if let Some(outlit) = &self.outlit {
            outlit.event(&distinct_id, &payload).await;
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
            let mut set_props = payload.set.clone();
            if let Some(ref email) = payload.email {
                set_props.insert("email".to_string(), serde_json::json!(email));
            }
            if !set_props.is_empty() {
                let _ = event.insert_prop("$set", &set_props);
            }
            if !payload.set_once.is_empty() {
                let _ = event.insert_prop("$set_once", &payload.set_once);
            }
            state.client.capture(event).await?;
        } else {
            tracing::info!("set_properties: {:?}", payload);
        }

        if let Some(outlit) = &self.outlit {
            outlit.identify(&distinct_id, &payload).await;
        }

        Ok(())
    }

    pub async fn is_feature_enabled(
        &self,
        flag_key: &str,
        distinct_id: &str,
    ) -> Result<bool, Error> {
        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;

            if let Some(local) = &state.local_eval {
                match local
                    .evaluator
                    .evaluate_flag(flag_key, distinct_id, &HashMap::new())
                {
                    Ok(Some(FlagValue::Boolean(v))) => return Ok(v),
                    Ok(Some(FlagValue::String(_))) => return Ok(true),
                    Ok(None) | Err(_) => {}
                }
            }

            Ok(state
                .client
                .is_feature_enabled(flag_key, distinct_id, None, None, None)
                .await
                .unwrap_or(false))
        } else {
            tracing::info!("is_feature_enabled: {} (no client)", flag_key);
            Ok(false)
        }
    }

    pub async fn get_feature_flag(
        &self,
        flag_key: &str,
        distinct_id: &str,
        person_properties: Option<HashMap<String, serde_json::Value>>,
        group_properties: Option<HashMap<String, HashMap<String, serde_json::Value>>>,
    ) -> Result<Option<FlagValue>, Error> {
        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;

            if let Some(local) = &state.local_eval {
                let props = person_properties.as_ref().cloned().unwrap_or_default();
                if let Ok(Some(value)) =
                    local.evaluator.evaluate_flag(flag_key, distinct_id, &props)
                {
                    return Ok(Some(value));
                }
            }

            Ok(state
                .client
                .get_feature_flag(
                    flag_key,
                    distinct_id,
                    None,
                    person_properties,
                    group_properties,
                )
                .await?)
        } else {
            tracing::info!("get_feature_flag: {} (no client)", flag_key);
            Ok(None)
        }
    }

    pub async fn get_feature_flag_payload(
        &self,
        flag_key: &str,
        distinct_id: &str,
    ) -> Result<Option<serde_json::Value>, Error> {
        if let Some(lazy) = &self.posthog {
            let state = lazy.get().await;
            Ok(state
                .client
                .get_feature_flag_payload(flag_key, distinct_id)
                .await?)
        } else {
            tracing::info!("get_feature_flag_payload: {} (no client)", flag_key);
            Ok(None)
        }
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

            let mut set_props = payload.set.clone();
            if let Some(ref email) = payload.email {
                set_props.insert("email".to_string(), serde_json::json!(email));
            }
            if !set_props.is_empty() {
                let _ = event.insert_prop("$set", &set_props);
            }
            if !payload.set_once.is_empty() {
                let _ = event.insert_prop("$set_once", &payload.set_once);
            }
            state.client.capture(event).await?;
        } else {
            tracing::info!(
                "identify: user_id={}, anon_distinct_id={}, payload={:?}",
                user_id,
                anon_distinct_id,
                payload
            );
        }

        if let Some(outlit) = &self.outlit {
            outlit.identify(&user_id, &payload).await;
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
    #[serde(flatten)]
    pub props: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct PropertiesPayload {
    #[serde(default)]
    pub set: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub set_once: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
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
        }
    }
}

#[derive(Clone)]
pub struct AnalyticsPayloadBuilder {
    event: Option<String>,
    props: HashMap<String, serde_json::Value>,
}

impl AnalyticsPayload {
    pub fn builder(event: impl Into<String>) -> AnalyticsPayloadBuilder {
        AnalyticsPayloadBuilder {
            event: Some(event.into()),
            props: HashMap::new(),
        }
    }
}

impl AnalyticsPayloadBuilder {
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
            props: self.props,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
