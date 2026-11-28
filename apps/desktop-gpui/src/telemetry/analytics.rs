use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use anlg_analytics::{
    AnalyticsClient, AnalyticsClientBuilder, AnalyticsGroup, AnalyticsPayload, PropertiesPayload,
    legacy_pseudonymous_device_id,
};
use anlg_desktop_identity::{Channel, store_json::StoreJson};
use serde_json::{Map, Value, json};
use tokio::sync::Semaphore;

use crate::product::settings::Snapshot;

const MAX_CONCURRENT_EVENTS: usize = 32;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_DURATION: Duration = Duration::from_secs(24 * 60 * 60);
const ANALYTICS_STORE_SCOPE: &str = "analytics";
const LEGACY_ALIAS_STORE_KEY: &str = "LegacyDistinctIdAliased";
const FIRST_OPENED_STORE_KEY: &str = "AppFirstOpened";

struct SessionTracker {
    id: uuid::Uuid,
    started_at: SystemTime,
    last_activity_at: SystemTime,
}

impl SessionTracker {
    fn new(now: SystemTime) -> Self {
        Self {
            id: uuid::Uuid::now_v7(),
            started_at: now,
            last_activity_at: now,
        }
    }

    fn session_id(&mut self, now: SystemTime) -> String {
        let idle_timed_out = now
            .duration_since(self.last_activity_at)
            .is_ok_and(|elapsed| elapsed >= IDLE_TIMEOUT);
        let max_duration_reached = now
            .duration_since(self.started_at)
            .is_ok_and(|elapsed| elapsed >= MAX_DURATION);
        if now < self.started_at
            || now < self.last_activity_at
            || idle_timed_out
            || max_duration_reached
        {
            *self = Self::new(now);
        } else {
            self.last_activity_at = now;
        }
        self.id.to_string()
    }
}

struct InFlightTracker {
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl InFlightTracker {
    fn new() -> Self {
        Self {
            state: Arc::new((Mutex::new(0), Condvar::new())),
        }
    }

    fn acquire(&self) -> InFlightGuard {
        let (lock, _) = &*self.state;
        let mut count = lock.lock().unwrap_or_else(|poison| poison.into_inner());
        *count += 1;
        InFlightGuard {
            state: self.state.clone(),
        }
    }

    fn wait_idle(&self, timeout: Duration) -> bool {
        let (lock, cvar) = &*self.state;
        let count = lock.lock().unwrap_or_else(|poison| poison.into_inner());
        let (remaining, result) = cvar
            .wait_timeout_while(count, timeout, |c| *c > 0)
            .unwrap_or_else(|poison| poison.into_inner());
        *remaining == 0 && !result.timed_out()
    }
}

struct InFlightGuard {
    state: Arc<(Mutex<usize>, Condvar)>,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let (lock, cvar) = &*self.state;
        let mut count = lock.lock().unwrap_or_else(|poison| poison.into_inner());
        *count = count.saturating_sub(1);
        if *count == 0 {
            cvar.notify_all();
        }
    }
}

#[derive(Clone)]
struct PendingIdentity {
    user_id: String,
    plan: String,
    trial_end: Option<i64>,
    created_at: Option<String>,
}

struct AnalyticsCore {
    client: AnalyticsClient,
    permits: Arc<Semaphore>,
    in_flight: InFlightTracker,
    shipping_store: Option<Arc<StoreJson>>,
    legacy_aliased: Arc<AtomicBool>,
    legacy_alias_claimed: Arc<AtomicBool>,
    groups: Mutex<HashMap<String, String>>,
    session: Mutex<SessionTracker>,
    last_identify: Mutex<Option<String>>,
    pending_identity: Mutex<Option<PendingIdentity>>,
    last_settings: Mutex<Option<Map<String, Value>>>,
    pending_settings: Mutex<Option<Map<String, Value>>>,
    runtime: tokio::runtime::Handle,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
}

struct AnalyticsHolder {
    _runtime: tokio::runtime::Runtime,
    core: AnalyticsCore,
}

static ANALYTICS: OnceLock<AnalyticsHolder> = OnceLock::new();

pub(super) fn init(channel: Option<Channel>, shipping_store: Option<Arc<StoreJson>>) {
    if channel.is_none() || ANALYTICS.get().is_some() {
        return;
    }
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .thread_name("analytics")
        .build()
    else {
        tracing::warn!("could not start analytics worker runtime");
        return;
    };
    let mut builder = AnalyticsClientBuilder::default();
    if let Some(key) = option_env!("POSTHOG_API_KEY").filter(|key| key.starts_with("phc_")) {
        builder = builder.with_posthog(key);
    }
    let core = AnalyticsCore::new(
        builder.build(),
        runtime.handle().clone(),
        shipping_store,
        Box::new(super::telemetry_enabled),
    );
    let _ = ANALYTICS.set(AnalyticsHolder {
        _runtime: runtime,
        core,
    });
}

async fn ensure_legacy_alias_in_task(
    client: &AnalyticsClient,
    store: Option<&Arc<StoreJson>>,
    machine_id: &str,
    aliased: &AtomicBool,
    claimed: &AtomicBool,
) {
    if aliased.load(Ordering::Acquire) {
        return;
    }
    let Some(store) = store else {
        return;
    };
    let store_read = store.clone();
    let already = tokio::task::spawn_blocking(move || {
        store_read
            .get(ANALYTICS_STORE_SCOPE, LEGACY_ALIAS_STORE_KEY)
            .ok()
            .flatten()
            == Some(Value::Bool(true))
    })
    .await
    .unwrap_or(false);
    if already {
        aliased.store(true, Ordering::Release);
        return;
    }
    if claimed
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let legacy_id = legacy_pseudonymous_device_id(machine_id);
    if let Err(error) = client.merge_distinct_ids(machine_id, legacy_id).await {
        claimed.store(false, Ordering::Release);
        tracing::debug!(%error, "legacy analytics id merge delivery failed");
        return;
    }
    aliased.store(true, Ordering::Release);
    let store_write = store.clone();
    let persisted = tokio::task::spawn_blocking(move || {
        store_write.set(
            ANALYTICS_STORE_SCOPE,
            LEGACY_ALIAS_STORE_KEY,
            Value::Bool(true),
        )
    })
    .await;
    if let Ok(Err(error)) = persisted {
        tracing::debug!(%error, "failed to persist legacy analytics alias marker");
    }
}

impl AnalyticsCore {
    fn new(
        client: AnalyticsClient,
        runtime: tokio::runtime::Handle,
        shipping_store: Option<Arc<StoreJson>>,
        enabled: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            client,
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_EVENTS)),
            in_flight: InFlightTracker::new(),
            shipping_store,
            legacy_aliased: Arc::new(AtomicBool::new(false)),
            legacy_alias_claimed: Arc::new(AtomicBool::new(false)),
            groups: Mutex::new(HashMap::new()),
            session: Mutex::new(SessionTracker::new(SystemTime::now())),
            last_identify: Mutex::new(None),
            pending_identity: Mutex::new(None),
            last_settings: Mutex::new(None),
            pending_settings: Mutex::new(None),
            runtime,
            enabled,
        }
    }

    fn enrich(&self, payload: &mut AnalyticsPayload) {
        let app_version = crate::platform::package::VERSION;
        let app_id = crate::platform::package::AppIds::current().app_user_model_id;
        let session_id = self
            .session
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .session_id(SystemTime::now());
        let groups = self
            .groups
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();

        payload
            .props
            .insert("$session_id".into(), session_id.into());
        for (group_type, group_key) in groups {
            payload
                .groups
                .get_or_insert_default()
                .entry(group_type)
                .or_insert(group_key);
        }
        payload
            .props
            .entry("app_version".into())
            .or_insert(app_version.into());
        payload
            .props
            .entry("surface".into())
            .or_insert("desktop".into());
        payload
            .props
            .entry("app_shell".into())
            .or_insert("gpui".into());
        payload
            .props
            .entry("analytics_schema_version".into())
            .or_insert(1.into());
        payload
            .props
            .entry("app_identifier".into())
            .or_insert(app_id.into());
        payload
            .props
            .entry("bundle_id".into())
            .or_insert(app_id.into());
        payload
            .props
            .entry("git_hash".into())
            .or_insert(env!("ANARLOG_GIT_SHA").into());
        payload
            .props
            .entry("os".into())
            .or_insert(std::env::consts::OS.into());
        payload
            .props
            .entry("arch".into())
            .or_insert(std::env::consts::ARCH.into());
        payload.props.entry("$set".into()).or_insert_with(|| {
            json!({
                "app_version": app_version,
                "app_shell": "gpui",
                "platform": std::env::consts::OS,
                "os_version": anlg_host::long_os_version(),
            })
        });
    }

    fn track(&self, event: String, props: &[(&str, Value)]) -> bool {
        if !(self.enabled)() {
            return false;
        }
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return false;
        };
        let guard = self.in_flight.acquire();
        let mut builder = AnalyticsPayload::builder(event);
        for (key, value) in props {
            builder = builder.with(*key, value.clone());
        }
        let mut payload = builder.build();
        self.enrich(&mut payload);
        let client = self.client.clone();
        let store = self.shipping_store.clone();
        let aliased = self.legacy_aliased.clone();
        let claimed = self.legacy_alias_claimed.clone();
        let machine_id = anlg_host::fingerprint();
        self.runtime.spawn(async move {
            let _permit = permit;
            let _guard = guard;
            ensure_legacy_alias_in_task(&client, store.as_ref(), &machine_id, &aliased, &claimed)
                .await;
            if let Err(error) = client.event(machine_id, payload).await {
                tracing::debug!(%error, "analytics event delivery failed");
            }
        });
        true
    }

    fn set_properties(&self, set: Map<String, Value>, set_once: Map<String, Value>) -> bool {
        if !(self.enabled)() {
            return false;
        }
        if set.is_empty() && set_once.is_empty() {
            return false;
        }
        let Ok(permit) = self.permits.clone().try_acquire_owned() else {
            return false;
        };
        let guard = self.in_flight.acquire();
        let mut builder = PropertiesPayload::builder();
        for (key, value) in set {
            builder = builder.set(key, value);
        }
        for (key, value) in set_once {
            builder = builder.set_once(key, value);
        }
        let payload = builder.build();
        let client = self.client.clone();
        let store = self.shipping_store.clone();
        let aliased = self.legacy_aliased.clone();
        let claimed = self.legacy_alias_claimed.clone();
        let machine_id = anlg_host::fingerprint();
        self.runtime.spawn(async move {
            let _permit = permit;
            let _guard = guard;
            ensure_legacy_alias_in_task(&client, store.as_ref(), &machine_id, &aliased, &claimed)
                .await;
            if let Err(error) = client.set_properties(machine_id, payload).await {
                tracing::debug!(%error, "analytics properties delivery failed");
            }
        });
        true
    }

    fn identify(
        &self,
        user_id: &str,
        plan: &str,
        trial_end: Option<i64>,
        created_at: Option<&str>,
    ) -> bool {
        if user_id.is_empty() {
            return false;
        }
        let created_at = created_at
            .map(str::trim)
            .filter(|ts| !ts.is_empty())
            .map(str::to_owned);
        *self
            .pending_identity
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(PendingIdentity {
            user_id: user_id.to_owned(),
            plan: plan.to_owned(),
            trial_end,
            created_at: created_at.clone(),
        });
        if !(self.enabled)() {
            return false;
        }
        let app_version = crate::platform::package::VERSION;
        let signature = format!("{user_id}:{plan}:{trial_end:?}:{created_at:?}:{app_version}");
        {
            let mut last = self
                .last_identify
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if last.as_deref() == Some(signature.as_str()) {
                return false;
            }
            let Ok(permit) = self.permits.clone().try_acquire_owned() else {
                return false;
            };
            let guard = self.in_flight.acquire();
            *last = Some(signature);
            self.groups
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .insert("account".into(), user_id.into());

            let trial_end_date = trial_end
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
            let mut set_props = PropertiesPayload::builder()
                .set("is_signed_up", true)
                .set("app_version", app_version)
                .set("app_shell", "gpui")
                .set("platform", std::env::consts::OS)
                .set("os_version", anlg_host::long_os_version())
                .set("plan", plan);
            let mut group_props = HashMap::from([("plan".to_owned(), json!(plan))]);
            if let Some(date) = created_at {
                set_props = set_props.set("account_created_date", date.clone());
                group_props.insert("created_at".into(), json!(date));
            }
            if let Some(date) = trial_end_date {
                set_props = set_props.set("trial_end_date", date.clone());
                group_props.insert("trial_end_date".into(), json!(date));
            }
            let mut payload = set_props.build();
            payload.group = Some(AnalyticsGroup {
                r#type: "account".into(),
                key: user_id.into(),
                properties: group_props,
            });
            let client = self.client.clone();
            let store = self.shipping_store.clone();
            let aliased = self.legacy_aliased.clone();
            let claimed = self.legacy_alias_claimed.clone();
            let user_id = user_id.to_owned();
            let machine_id = anlg_host::fingerprint();
            self.runtime.spawn(async move {
                let _permit = permit;
                let _guard = guard;
                ensure_legacy_alias_in_task(
                    &client,
                    store.as_ref(),
                    &machine_id,
                    &aliased,
                    &claimed,
                )
                .await;
                if let Err(error) = client.identify(user_id, machine_id, payload).await {
                    tracing::debug!(%error, "analytics identify delivery failed");
                }
            });
            true
        }
    }

    fn sync_identity(&self) -> bool {
        let pending = self
            .pending_identity
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        let Some(pending) = pending else {
            return false;
        };
        self.identify(
            &pending.user_id,
            &pending.plan,
            pending.trial_end,
            pending.created_at.as_deref(),
        )
    }

    fn sync_settings_properties(&self, snapshot: &Snapshot) -> bool {
        let map = settings_properties_map(snapshot);
        *self
            .pending_settings
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(map.clone());
        self.send_settings_properties(map)
    }

    fn sync_settings_properties_if_pending(&self) -> bool {
        let Some(map) = self
            .pending_settings
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
        else {
            return false;
        };
        self.send_settings_properties(map)
    }

    fn send_settings_properties(&self, map: Map<String, Value>) -> bool {
        if !(self.enabled)() {
            return false;
        }
        let mut last = self
            .last_settings
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if last.as_ref() == Some(&map) {
            return false;
        }
        if !self.set_properties(map.clone(), Map::new()) {
            return false;
        }
        *last = Some(map);
        true
    }

    fn track_first_open_if_needed(&self, shipping_store: Option<&Arc<StoreJson>>) -> bool {
        if !(self.enabled)() {
            return false;
        }
        let Some(store) = shipping_store else {
            return false;
        };
        if store
            .get(ANALYTICS_STORE_SCOPE, FIRST_OPENED_STORE_KEY)
            .ok()
            .flatten()
            == Some(Value::Bool(true))
        {
            return false;
        }
        let already_aliased = store
            .get(ANALYTICS_STORE_SCOPE, LEGACY_ALIAS_STORE_KEY)
            .ok()
            .flatten()
            == Some(Value::Bool(true));
        let sent = if already_aliased {
            false
        } else {
            self.track(
                "app_first_opened".into(),
                &[("first_open_marker", json!("local_install"))],
            )
        };
        if let Err(error) = store.set(
            ANALYTICS_STORE_SCOPE,
            FIRST_OPENED_STORE_KEY,
            Value::Bool(true),
        ) {
            tracing::debug!(%error, "failed to persist app_first_opened marker");
        }
        sent
    }

    fn clear_identity(&self) {
        self.groups
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clear();
        *self
            .last_identify
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
        *self
            .pending_identity
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
    }

    fn flush(&self, timeout: Duration) -> bool {
        self.in_flight.wait_idle(timeout)
    }
}

fn settings_properties_map(snapshot: &Snapshot) -> Map<String, Value> {
    let spoken_languages = match snapshot.get("spoken_languages").map(|s| &s.value) {
        Some(Value::String(text)) => serde_json::from_str::<Vec<String>>(text).unwrap_or_default(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    };
    let nullable_text = |key: &str| -> Value {
        snapshot
            .get(key)
            .and_then(|s| s.value.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_owned()))
            .unwrap_or(Value::Null)
    };
    let mut map = Map::new();
    map.insert(
        "spoken_languages".into(),
        Value::Array(spoken_languages.into_iter().map(Value::String).collect()),
    );
    map.insert(
        "current_stt_provider".into(),
        nullable_text("current_stt_provider"),
    );
    map.insert(
        "current_stt_model".into(),
        nullable_text("current_stt_model"),
    );
    map.insert(
        "current_llm_provider".into(),
        nullable_text("current_llm_provider"),
    );
    map.insert(
        "current_llm_model".into(),
        nullable_text("current_llm_model"),
    );
    map
}

pub(super) fn track(event: impl Into<String>, props: &[(&str, Value)]) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.track(event.into(), props);
    }
}

pub(super) fn set_properties(set: Map<String, Value>, set_once: Map<String, Value>) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.set_properties(set, set_once);
    }
}

pub(super) fn identify(
    user_id: &str,
    plan: &str,
    trial_end: Option<i64>,
    created_at: Option<&str>,
) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.identify(user_id, plan, trial_end, created_at);
    }
}

pub(super) fn sync_identity() {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.sync_identity();
    }
}

pub(super) fn sync_settings_properties(snapshot: &Snapshot) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.sync_settings_properties(snapshot);
    }
}

pub(super) fn sync_settings_properties_if_pending() {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.sync_settings_properties_if_pending();
    }
}

pub(super) fn track_first_open_if_needed(shipping_store: Option<&Arc<StoreJson>>) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.track_first_open_if_needed(shipping_store);
    }
}

pub(super) fn clear_identity() {
    if let Some(holder) = ANALYTICS.get() {
        holder.core.clear_identity();
    }
}

pub(super) fn flush(timeout: Duration) -> bool {
    ANALYTICS
        .get()
        .is_none_or(|holder| holder.core.flush(timeout))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn session_tracker_keeps_active_session_and_rotates_on_timeout() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let mut tracker = SessionTracker::new(now);
        let first = tracker.session_id(now);
        let active = now + IDLE_TIMEOUT - Duration::from_millis(1);
        assert_eq!(tracker.session_id(active), first);
        assert_ne!(tracker.session_id(active + IDLE_TIMEOUT), first);
    }

    #[test]
    fn disabled_analytics_does_not_dispatch() {
        let rt = runtime();
        let server = rt.block_on(MockServer::start());
        rt.block_on(async {
            Mock::given(method("POST"))
                .and(path("/i/v0/e/"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;
        });
        let client = AnalyticsClientBuilder::default()
            .with_posthog_endpoint("phc_test", server.uri())
            .build();
        let enabled = Arc::new(AtomicBool::new(false));
        let toggle = enabled.clone();
        let core = AnalyticsCore::new(
            client,
            rt.handle().clone(),
            None,
            Box::new(move || toggle.load(Ordering::SeqCst)),
        );
        assert!(!core.track("app_opened".into(), &[]));
        assert!(!core.identify("user-1", "pro", None, Some("2026-01-01T00:00:00Z")));
        assert!(core.groups.lock().unwrap().is_empty());
        assert!(core.last_identify.lock().unwrap().is_none());
        assert_eq!(core.permits.available_permits(), MAX_CONCURRENT_EVENTS);
        assert_eq!(rt.block_on(server.received_requests()).unwrap().len(), 0);

        enabled.store(true, Ordering::SeqCst);
        assert!(core.sync_identity());
        assert!(core.flush(Duration::from_secs(5)));
        assert_eq!(
            core.groups
                .lock()
                .unwrap()
                .get("account")
                .map(String::as_str),
            Some("user-1")
        );
        assert!(core.last_identify.lock().unwrap().is_some());
        assert!(!core.sync_identity());

        let requests = rt.block_on(server.received_requests()).unwrap();
        assert_eq!(requests.len(), 2);
        let identify_body: Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(identify_body["event"], "$identify");
        assert_eq!(
            identify_body["properties"]["$set"]["account_created_date"],
            "2026-01-01T00:00:00Z"
        );
        let group_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
        assert_eq!(group_body["event"], "$groupidentify");
        assert_eq!(
            group_body["properties"]["$group_set"]["created_at"],
            "2026-01-01T00:00:00Z"
        );

        core.clear_identity();
        assert!(!core.sync_identity());
        assert!(core.groups.lock().unwrap().is_empty());
        assert!(core.last_identify.lock().unwrap().is_none());
    }

    #[test]
    fn track_set_properties_and_legacy_alias_deliver_and_flush_cleanly() {
        let rt = runtime();
        let server = rt.block_on(MockServer::start());
        rt.block_on(async {
            Mock::given(method("POST"))
                .and(path("/i/v0/e/"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;
        });

        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StoreJson::at(dir.path().join("store.json")));
        let client = AnalyticsClientBuilder::default()
            .with_posthog_endpoint("phc_test", server.uri())
            .build();
        let core = AnalyticsCore::new(
            client,
            rt.handle().clone(),
            Some(store.clone()),
            Box::new(|| true),
        );

        assert!(core.track_first_open_if_needed(Some(&store)));
        assert!(!core.track_first_open_if_needed(Some(&store)));
        assert!(core.flush(Duration::from_secs(5)));
        assert_eq!(
            store
                .get(ANALYTICS_STORE_SCOPE, LEGACY_ALIAS_STORE_KEY)
                .unwrap(),
            Some(Value::Bool(true))
        );
        assert_eq!(
            store
                .get(ANALYTICS_STORE_SCOPE, FIRST_OPENED_STORE_KEY)
                .unwrap(),
            Some(Value::Bool(true))
        );

        let snapshot = crate::product::settings::decode(&[json!({
            "id": "current_stt_provider",
            "value_json": "\"anarlog\"",
            "oversized": 0,
            "source_rank": 0,
        })])
        .unwrap();
        assert!(core.sync_settings_properties(&snapshot));
        assert!(!core.sync_settings_properties(&snapshot));
        assert!(core.flush(Duration::from_secs(5)));

        let requests = rt.block_on(server.received_requests()).unwrap();
        let event_names: Vec<String> = requests
            .iter()
            .map(|req| {
                let body: Value = serde_json::from_slice(&req.body).unwrap();
                body["event"].as_str().unwrap_or_default().to_owned()
            })
            .collect();
        assert_eq!(
            event_names,
            ["$merge_dangerously", "app_first_opened", "$set"]
        );

        let first_open_body: Value = serde_json::from_slice(&requests[1].body).unwrap();
        assert_eq!(
            first_open_body["properties"]["first_open_marker"],
            "local_install"
        );
        assert_eq!(first_open_body["properties"]["app_shell"], "gpui");
        assert!(first_open_body["properties"]["$session_id"].is_string());

        let set_body: Value = serde_json::from_slice(&requests[2].body).unwrap();
        assert_eq!(
            set_body["properties"]["$set"]["current_stt_provider"],
            "anarlog"
        );
    }

    #[test]
    fn failed_legacy_alias_can_retry_until_persisted() {
        let rt = runtime();
        let server = rt.block_on(MockServer::start());
        rt.block_on(async {
            Mock::given(method("POST"))
                .and(path("/i/v0/e/"))
                .respond_with(ResponseTemplate::new(500))
                .up_to_n_times(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/i/v0/e/"))
                .respond_with(ResponseTemplate::new(200))
                .mount(&server)
                .await;
        });

        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(StoreJson::at(dir.path().join("store.json")));
        let client = AnalyticsClientBuilder::default()
            .with_posthog_endpoint("phc_test", server.uri())
            .build();
        let core = AnalyticsCore::new(
            client,
            rt.handle().clone(),
            Some(store.clone()),
            Box::new(|| true),
        );

        assert!(core.track("app_started".into(), &[]));
        assert!(core.flush(Duration::from_secs(5)));
        assert_eq!(
            store
                .get(ANALYTICS_STORE_SCOPE, LEGACY_ALIAS_STORE_KEY)
                .unwrap(),
            None
        );

        assert!(core.track("app_opened".into(), &[]));
        assert!(core.flush(Duration::from_secs(5)));
        assert_eq!(
            store
                .get(ANALYTICS_STORE_SCOPE, LEGACY_ALIAS_STORE_KEY)
                .unwrap(),
            Some(Value::Bool(true))
        );
    }
}
