#![forbid(unsafe_code)]

mod db;
mod error;
mod listener;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use error::{
    BridgeError, cloudsync_error, cloudsync_runtime_error, parse_params_json, runtime_error,
    serialization_error,
};
use listener::{ListenerSink, QueryEventListener};

uniffi::setup_scaffolding!();

struct BridgeState {
    db_runtime: hypr_db_live_query::DbRuntime<ListenerSink>,
    runtime: tokio::runtime::Runtime,
    subscription_ids: HashSet<String>,
}

#[derive(uniffi::Object)]
pub struct MobileDbBridge {
    state: Mutex<Option<BridgeState>>,
}

#[uniffi::export]
impl MobileDbBridge {
    #[uniffi::constructor]
    pub fn open(db_path: String, cloudsync_open_mode: Option<String>) -> Result<Self, BridgeError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|error| BridgeError::OpenFailed {
                reason: error.to_string(),
            })?;
        let path = std::path::PathBuf::from(db_path);
        let cloudsync_open_mode = match cloudsync_open_mode.as_deref() {
            Some("enabled") => hypr_db_core2::CloudsyncOpenMode::Enabled,
            _ => hypr_db_core2::CloudsyncOpenMode::Disabled,
        };
        let db = runtime
            .block_on(db::open_app_db(&path, cloudsync_open_mode))
            .map_err(|error| BridgeError::OpenFailed {
                reason: error.to_string(),
            })?;
        let db_runtime = {
            let _guard = runtime.enter();
            hypr_db_live_query::DbRuntime::new(std::sync::Arc::new(db))
        };

        Ok(Self {
            state: Mutex::new(Some(BridgeState {
                db_runtime,
                runtime,
                subscription_ids: HashSet::new(),
            })),
        })
    }

    pub fn execute(&self, sql: String, params_json: String) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        self.with_state(|state| {
            let rows = state
                .runtime
                .block_on(state.db_runtime.execute(sql, params))
                .map_err(runtime_error)?;
            serde_json::to_string(&rows).map_err(serialization_error)
        })
    }

    pub fn execute_proxy(
        &self,
        sql: String,
        params_json: String,
        method: String,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        self.with_state(|state| {
            let rows = state
                .runtime
                .block_on(state.db_runtime.execute_proxy(sql, params, method))
                .map_err(runtime_error)?;
            serde_json::to_string(&rows).map_err(serialization_error)
        })
    }

    pub fn subscribe(
        &self,
        sql: String,
        params_json: String,
        listener: Arc<dyn QueryEventListener>,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        self.with_state(|state| {
            let registration = state
                .runtime
                .block_on(
                    state
                        .db_runtime
                        .subscribe(sql, params, ListenerSink::new(listener)),
                )
                .map_err(runtime_error)?;
            state.subscription_ids.insert(registration.id.clone());
            Ok(registration.id)
        })
    }

    pub fn unsubscribe(&self, subscription_id: String) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.unsubscribe(&subscription_id))
                .map_err(runtime_error)?;
            state.subscription_ids.remove(&subscription_id);
            Ok(())
        })
    }

    pub fn cloudsync_version(&self) -> Result<String, BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_version())
                .map_err(cloudsync_error)
        })
    }

    pub fn cloudsync_init(
        &self,
        table_name: String,
        crdt_algo: Option<String>,
        force: Option<bool>,
    ) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_init(
                    &table_name,
                    crdt_algo.as_deref(),
                    force,
                ))
                .map_err(cloudsync_error)
        })
    }

    pub fn cloudsync_network_init(&self, connection_string: String) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(
                    state
                        .db_runtime
                        .db()
                        .cloudsync_network_init(&connection_string),
                )
                .map_err(cloudsync_error)
        })
    }

    pub fn cloudsync_network_set_apikey(&self, api_key: String) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_network_set_apikey(&api_key))
                .map_err(cloudsync_error)
        })
    }

    pub fn cloudsync_network_set_token(&self, token: String) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_network_set_token(&token))
                .map_err(cloudsync_error)
        })
    }

    pub fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<i64, BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(
                    state
                        .db_runtime
                        .db()
                        .cloudsync_network_sync(wait_ms, max_retries),
                )
                .map_err(cloudsync_error)
        })
    }

    pub fn configure_cloudsync(&self, config_json: String) -> Result<(), BridgeError> {
        let config: hypr_db_core2::CloudsyncRuntimeConfig = serde_json::from_str(&config_json)
            .map_err(|error| BridgeError::InvalidCloudsyncConfigJson {
                reason: error.to_string(),
            })?;
        self.with_state(|state| {
            state
                .db_runtime
                .db()
                .cloudsync_configure(config)
                .map_err(cloudsync_runtime_error)
        })
    }

    pub fn start_cloudsync(&self) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_start())
                .map_err(cloudsync_runtime_error)
        })
    }

    pub fn stop_cloudsync(&self) -> Result<(), BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_stop())
                .map_err(cloudsync_runtime_error)
        })
    }

    pub fn cloudsync_status(&self) -> Result<String, BridgeError> {
        self.with_state(|state| {
            let status = state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_status())
                .map_err(cloudsync_runtime_error)?;
            serde_json::to_string(&status).map_err(serialization_error)
        })
    }

    pub fn cloudsync_sync_now(&self) -> Result<i64, BridgeError> {
        self.with_state(|state| {
            state
                .runtime
                .block_on(state.db_runtime.db().cloudsync_trigger_sync())
                .map_err(cloudsync_runtime_error)
        })
    }

    pub fn close(&self) -> Result<(), BridgeError> {
        let mut guard = self.state.lock().unwrap();
        let Some(mut state) = guard.take() else {
            return Ok(());
        };

        let subscription_ids: Vec<String> = state.subscription_ids.drain().collect();
        let pool = state.db_runtime.db().pool().clone();
        state.runtime.block_on(async {
            for subscription_id in subscription_ids {
                let _ = state.db_runtime.unsubscribe(&subscription_id).await;
            }
            let _ = state.db_runtime.db().cloudsync_stop().await;
        });
        drop(state.db_runtime);
        state.runtime.block_on(pool.close());

        Ok(())
    }
}

impl MobileDbBridge {
    fn with_state<T>(
        &self,
        f: impl FnOnce(&mut BridgeState) -> Result<T, BridgeError>,
    ) -> Result<T, BridgeError> {
        let mut guard = self.state.lock().unwrap();
        let state = guard.as_mut().ok_or(BridgeError::Closed)?;
        f(state)
    }
}

impl Drop for MobileDbBridge {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(any(
    all(test, target_os = "macos", target_arch = "aarch64"),
    all(test, target_os = "macos", target_arch = "x86_64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(
        test,
        target_os = "linux",
        target_env = "musl",
        target_arch = "aarch64"
    ),
    all(test, target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(test, target_os = "windows", target_arch = "x86_64"),
))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[derive(Clone, Debug, PartialEq)]
    enum TestEvent {
        Result(Vec<serde_json::Value>),
        Error(String),
    }

    #[derive(Clone)]
    struct TestListener {
        events: Arc<Mutex<Vec<TestEvent>>>,
    }

    impl QueryEventListener for TestListener {
        fn on_result(&self, rows_json: String) {
            let rows: Vec<serde_json::Value> =
                serde_json::from_str(&rows_json).expect("rows json should parse");
            self.events.lock().unwrap().push(TestEvent::Result(rows));
        }

        fn on_error(&self, message: String) {
            self.events.lock().unwrap().push(TestEvent::Error(message));
        }
    }

    impl TestListener {
        fn capture() -> (Arc<Self>, Arc<Mutex<Vec<TestEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Arc::new(Self {
                    events: Arc::clone(&events),
                }),
                events,
            )
        }
    }

    fn next_event(events: &Arc<Mutex<Vec<TestEvent>>>, index: usize) -> TestEvent {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        loop {
            if let Some(event) = events.lock().unwrap().get(index).cloned() {
                return event;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for event {index}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_stable_event_count(
        events: &Arc<Mutex<Vec<TestEvent>>>,
        stable_for: Duration,
    ) -> usize {
        let mut last_len = events.lock().unwrap().len();
        loop {
            std::thread::sleep(stable_for);
            let len = events.lock().unwrap().len();
            if len == last_len {
                return len;
            }
            last_len = len;
        }
    }

    fn new_bridge(open_mode: Option<&str>) -> (tempfile::TempDir, MobileDbBridge) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("app.db");
        let bridge = MobileDbBridge::open(
            db_path.to_string_lossy().into_owned(),
            open_mode.map(str::to_string),
        )
        .unwrap();
        (dir, bridge)
    }

    #[test]
    fn execute_roundtrips_rows() {
        let (_dir, bridge) = new_bridge(None);

        bridge
            .execute(
                "INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)".to_string(),
                r#"["note-1","2026-04-13","{}","user-1"]"#.to_string(),
            )
            .unwrap();

        let rows_json = bridge
            .execute(
                "SELECT id, date, user_id FROM daily_notes ORDER BY id".to_string(),
                "[]".to_string(),
            )
            .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&rows_json).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "note-1");
        assert_eq!(rows[0]["date"], "2026-04-13");
        assert_eq!(rows[0]["user_id"], "user-1");
    }

    #[test]
    fn execute_proxy_roundtrips_positional_rows() {
        let (_dir, bridge) = new_bridge(None);

        bridge
            .execute(
                "INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)".to_string(),
                r#"["note-1","2026-04-13","{}","user-1"]"#.to_string(),
            )
            .unwrap();

        let result_json = bridge
            .execute_proxy(
                "SELECT id, date, user_id FROM daily_notes ORDER BY id".to_string(),
                "[]".to_string(),
                "all".to_string(),
            )
            .unwrap();
        let result: hypr_db_live_query::ProxyQueryResult =
            serde_json::from_str(&result_json).unwrap();

        assert_eq!(
            result.rows,
            vec![serde_json::json!(["note-1", "2026-04-13", "user-1"])]
        );
    }

    #[test]
    fn subscribe_reruns_after_write() {
        let (_dir, bridge) = new_bridge(None);
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                "[]".to_string(),
                listener,
            )
            .unwrap();

        let initial = next_event(&events, 0);
        assert_eq!(initial, TestEvent::Result(Vec::new()));

        bridge
            .execute(
                "INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)".to_string(),
                r#"["note-live","2026-04-13","{}","user-1"]"#.to_string(),
            )
            .unwrap();

        let refresh = next_event(&events, 1);
        let TestEvent::Result(rows) = refresh else {
            panic!("expected result event");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "note-live");

        bridge.unsubscribe(subscription_id).unwrap();
    }

    #[test]
    fn unsubscribe_stops_future_events() {
        let (_dir, bridge) = new_bridge(None);
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, date FROM daily_notes ORDER BY id".to_string(),
                "[]".to_string(),
                listener,
            )
            .unwrap();

        next_event(&events, 0);
        bridge.unsubscribe(subscription_id).unwrap();

        bridge
            .execute(
                "INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)".to_string(),
                r#"["note-after-unsub","2026-04-13","{}","user-1"]"#.to_string(),
            )
            .unwrap();

        let count = wait_for_stable_event_count(&events, Duration::from_millis(100));
        assert_eq!(count, 1);
    }

    #[test]
    fn close_rejects_future_calls() {
        let (_dir, bridge) = new_bridge(None);

        bridge.close().unwrap();

        assert!(matches!(
            bridge.execute("SELECT 1".to_string(), "[]".to_string()),
            Err(BridgeError::Closed)
        ));
    }

    #[test]
    fn cloudsync_manager_roundtrips_when_disabled() {
        let (_dir, bridge) = new_bridge(None);

        bridge
            .configure_cloudsync(
                r#"{
                    "connection_string":"sqlitecloud://demo.invalid/app.db?apikey=demo",
                    "auth":{"type":"none"},
                    "tables":[{"table_name":"templates","crdt_algo":null,"force_init":null,"enabled":false}],
                    "sync_interval_ms":30000,
                    "wait_ms":1000,
                    "max_retries":1
                }"#
                .to_string(),
            )
            .unwrap();
        bridge.start_cloudsync().unwrap();

        let status: serde_json::Value =
            serde_json::from_str(&bridge.cloudsync_status().unwrap()).unwrap();
        assert_eq!(status["open_mode"], "disabled");
        assert_eq!(status["configured"], true);
        assert_eq!(status["running"], false);
        assert_eq!(status["network_initialized"], false);

        assert_eq!(bridge.cloudsync_sync_now().unwrap(), 0);
        bridge.stop_cloudsync().unwrap();
    }

    #[test]
    fn cloudsync_methods_delegate() {
        let (_dir, bridge) = new_bridge(Some("enabled"));

        let version = bridge.cloudsync_version().unwrap();
        assert!(!version.is_empty());

        bridge
            .execute(
                "CREATE TABLE IF NOT EXISTS mobile_sync_test (
                    id TEXT PRIMARY KEY NOT NULL,
                    value TEXT NOT NULL DEFAULT ''
                )"
                .to_string(),
                "[]".to_string(),
            )
            .unwrap();

        let error = bridge
            .cloudsync_init("missing_mobile_sync_test".to_string(), None, None)
            .unwrap_err();
        assert!(matches!(error, BridgeError::CloudsyncFailed { .. }));
    }

    #[test]
    fn invalid_params_shape_is_rejected() {
        let (_dir, bridge) = new_bridge(None);

        let error = bridge
            .execute("SELECT 1".to_string(), "{}".to_string())
            .unwrap_err();

        assert!(matches!(error, BridgeError::ParamsMustBeArray));
    }
}
