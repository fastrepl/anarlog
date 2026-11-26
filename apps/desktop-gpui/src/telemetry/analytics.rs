use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use anlg_analytics::{
    AnalyticsClient, AnalyticsClientBuilder, AnalyticsGroup, AnalyticsPayload, PropertiesPayload,
};
use anlg_desktop_identity::Channel;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

const MAX_CONCURRENT_EVENTS: usize = 32;
const IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

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

struct AnalyticsCore {
    client: AnalyticsClient,
    permits: Arc<Semaphore>,
    groups: Mutex<HashMap<String, String>>,
    session: Mutex<SessionTracker>,
    last_identify: Mutex<Option<String>>,
    pending_identity: Mutex<Option<(String, String, Option<i64>)>>,
    runtime: tokio::runtime::Handle,
    enabled: Box<dyn Fn() -> bool + Send + Sync>,
}

struct AnalyticsHolder {
    _runtime: tokio::runtime::Runtime,
    core: AnalyticsCore,
}

static ANALYTICS: OnceLock<AnalyticsHolder> = OnceLock::new();

pub(super) fn init(channel: Option<Channel>) {
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
        Box::new(super::telemetry_enabled),
    );
    let _ = ANALYTICS.set(AnalyticsHolder {
        _runtime: runtime,
        core,
    });
}

impl AnalyticsCore {
    fn new(
        client: AnalyticsClient,
        runtime: tokio::runtime::Handle,
        enabled: Box<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            client,
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT_EVENTS)),
            groups: Mutex::new(HashMap::new()),
            session: Mutex::new(SessionTracker::new(SystemTime::now())),
            last_identify: Mutex::new(None),
            pending_identity: Mutex::new(None),
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
        let mut builder = AnalyticsPayload::builder(event);
        for (key, value) in props {
            builder = builder.with(*key, value.clone());
        }
        let mut payload = builder.build();
        self.enrich(&mut payload);
        let client = self.client.clone();
        let machine_id = anlg_host::fingerprint();
        self.runtime.spawn(async move {
            let _permit = permit;
            if let Err(error) = client.event(machine_id, payload).await {
                tracing::debug!(%error, "analytics event delivery failed");
            }
        });
        true
    }

    fn identify(&self, user_id: &str, plan: &str, trial_end: Option<i64>) -> bool {
        if user_id.is_empty() {
            return false;
        }
        *self
            .pending_identity
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) =
            Some((user_id.to_owned(), plan.to_owned(), trial_end));
        if !(self.enabled)() {
            return false;
        }
        let app_version = crate::platform::package::VERSION;
        let signature = format!("{user_id}:{plan}:{trial_end:?}:{app_version}");
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
            let user_id = user_id.to_owned();
            let machine_id = anlg_host::fingerprint();
            self.runtime.spawn(async move {
                let _permit = permit;
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
        let Some((user_id, plan, trial_end)) = pending else {
            return false;
        };
        self.identify(&user_id, &plan, trial_end)
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
}

pub(super) fn track(event: impl Into<String>, props: &[(&str, Value)]) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.track(event.into(), props);
    }
}

pub(super) fn identify(user_id: &str, plan: &str, trial_end: Option<i64>) {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.identify(user_id, plan, trial_end);
    }
}

pub(super) fn sync_identity() {
    if let Some(holder) = ANALYTICS.get() {
        let _ = holder.core.sync_identity();
    }
}

pub(super) fn clear_identity() {
    if let Some(holder) = ANALYTICS.get() {
        holder.core.clear_identity();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

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
        let client = AnalyticsClientBuilder::default().build();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let enabled = Arc::new(AtomicBool::new(false));
        let toggle = enabled.clone();
        let core = AnalyticsCore::new(
            client,
            runtime.handle().clone(),
            Box::new(move || toggle.load(Ordering::SeqCst)),
        );
        assert!(!core.track("app_opened".into(), &[]));
        assert!(!core.identify("user-1", "pro", None));
        assert!(core.groups.lock().unwrap().is_empty());
        assert!(core.last_identify.lock().unwrap().is_none());
        assert_eq!(core.permits.available_permits(), MAX_CONCURRENT_EVENTS);

        enabled.store(true, Ordering::SeqCst);
        assert!(core.sync_identity());
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

        core.clear_identity();
        assert!(!core.sync_identity());
        assert!(core.groups.lock().unwrap().is_empty());
        assert!(core.last_identify.lock().unwrap().is_none());
    }
}
