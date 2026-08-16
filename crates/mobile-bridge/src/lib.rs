#![forbid(unsafe_code)]

mod db;
mod error;
mod listener;

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex};

use error::{
    BridgeError, cloudsync_error, cloudsync_runtime_error, execute_error, parse_params_json,
    reactive_error, serialization_error,
};
use listener::{ListenerSink, QueryEventListener};

uniffi::setup_scaffolding!();

fn block_on<F: Future>(runtime: &tokio::runtime::Runtime, future: F) -> F::Output {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| runtime.handle().block_on(future))
        }
        _ => runtime.handle().block_on(future),
    }
}

struct BridgeState {
    db: Arc<anlg_db_core::Db>,
    executor: anlg_db_execute::DbExecutor,
    live_query_runtime: Arc<anlg_db_reactive::LiveQueryRuntime<ListenerSink>>,
    runtime: Arc<tokio::runtime::Runtime>,
    subscription_ids: HashSet<String>,
    e2ee_sync_hook: Arc<anlg_db_sync::E2eeSyncHook>,
    replica_sync: anlg_db_sync::ReplicaSyncTask,
    witness_watch: anlg_db_sync::WitnessWatchTask,
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
        let runtime = Arc::new(runtime);
        let path = std::path::PathBuf::from(db_path);
        let cloudsync_enabled = cloudsync_open_mode.as_deref() == Some("enabled");
        let db =
            block_on(&runtime, db::open_app_db(&path, cloudsync_enabled)).map_err(|error| {
                BridgeError::OpenFailed {
                    reason: error.to_string(),
                }
            })?;
        let db = std::sync::Arc::new(db);
        let e2ee_sync_hook = Arc::new(anlg_db_sync::E2eeSyncHook::default());
        db.set_cloudsync_sync_hook(e2ee_sync_hook.clone());
        let executor = anlg_db_execute::DbExecutor::new(std::sync::Arc::clone(&db));
        let (live_query_runtime, replica_sync, witness_watch) = {
            let _guard = runtime.enter();
            (
                Arc::new(anlg_db_reactive::LiveQueryRuntime::new(Arc::clone(&db))),
                anlg_db_sync::spawn_replica_sync(Arc::clone(&db), Arc::clone(&e2ee_sync_hook)),
                anlg_db_sync::spawn_witness_watch(Arc::clone(&db), Arc::clone(&e2ee_sync_hook)),
            )
        };

        Ok(Self {
            state: Mutex::new(Some(BridgeState {
                db,
                executor,
                live_query_runtime,
                runtime,
                subscription_ids: HashSet::new(),
                e2ee_sync_hook,
                replica_sync,
                witness_watch,
            })),
        })
    }

    pub fn execute(&self, sql: String, params_json: String) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let (runtime, executor) =
            self.with_state(|state| Ok((Arc::clone(&state.runtime), state.executor.clone())))?;
        let rows = block_on(&runtime, executor.execute(sql, params)).map_err(execute_error)?;
        serde_json::to_string(&rows).map_err(serialization_error)
    }

    pub fn execute_proxy(
        &self,
        sql: String,
        params_json: String,
        method: String,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let method = method
            .parse::<anlg_db_execute::ProxyQueryMethod>()
            .map_err(execute_error)?;
        let (runtime, executor) =
            self.with_state(|state| Ok((Arc::clone(&state.runtime), state.executor.clone())))?;
        let rows = block_on(&runtime, executor.execute_proxy(sql, params, method))
            .map_err(execute_error)?;
        serde_json::to_string(&rows).map_err(serialization_error)
    }

    pub fn execute_transaction(&self, statements_json: String) -> Result<String, BridgeError> {
        let statements: Vec<anlg_db_execute::TransactionStatement> =
            serde_json::from_str(&statements_json).map_err(|error| {
                BridgeError::InvalidTransactionStatementsJson {
                    reason: error.to_string(),
                }
            })?;
        let (runtime, executor) =
            self.with_state(|state| Ok((Arc::clone(&state.runtime), state.executor.clone())))?;
        let rows_affected =
            block_on(&runtime, executor.execute_transaction(statements)).map_err(execute_error)?;
        serde_json::to_string(&rows_affected).map_err(serialization_error)
    }

    pub fn subscribe(
        &self,
        sql: String,
        params_json: String,
        listener: Arc<dyn QueryEventListener>,
    ) -> Result<String, BridgeError> {
        let params = parse_params_json(&params_json)?;
        let sql_for_log = sql.clone();
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        let registration = block_on(
            &runtime,
            live_query_runtime.subscribe(sql, params, ListenerSink::new(listener)),
        )
        .map_err(reactive_error)?;

        if let anlg_db_reactive::DependencyAnalysis::NonReactive { reason } = &registration.analysis
        {
            eprintln!(
                "[mobile-bridge] live query subscription is non-reactive for SQL {:?}: {}",
                sql_for_log, reason
            );
        }

        let subscription_id = registration.id.clone();
        if self
            .with_state(|state| {
                state.subscription_ids.insert(subscription_id.clone());
                Ok(())
            })
            .is_err()
        {
            let _ = block_on(&runtime, live_query_runtime.unsubscribe(&registration.id));
            return Err(BridgeError::Closed);
        }

        Ok(registration.id)
    }

    pub fn unsubscribe(&self, subscription_id: String) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(&runtime, live_query_runtime.unsubscribe(&subscription_id))
            .map_err(reactive_error)?;
        self.with_state(|state| {
            state.subscription_ids.remove(&subscription_id);
            Ok(())
        })
    }

    pub fn cloudsync_version(&self) -> Result<String, BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(&runtime, live_query_runtime.db().cloudsync_version()).map_err(cloudsync_error)
    }

    pub fn cloudsync_init(
        &self,
        table_name: String,
        crdt_algo: Option<String>,
        init_flags: Option<i64>,
    ) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(
            &runtime,
            live_query_runtime
                .db()
                .cloudsync_init(&table_name, crdt_algo.as_deref(), init_flags),
        )
        .map_err(cloudsync_error)
    }

    pub fn cloudsync_network_init(&self, connection_string: String) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(
            &runtime,
            live_query_runtime
                .db()
                .cloudsync_network_init(&connection_string),
        )
        .map_err(cloudsync_error)
    }

    pub fn cloudsync_network_set_apikey(&self, api_key: String) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(
            &runtime,
            live_query_runtime
                .db()
                .cloudsync_network_set_apikey(&api_key),
        )
        .map_err(cloudsync_error)
    }

    pub fn cloudsync_network_set_token(&self, token: String) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(
            &runtime,
            live_query_runtime.db().cloudsync_network_set_token(&token),
        )
        .map_err(cloudsync_error)
    }

    pub fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<String, BridgeError> {
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        let result = block_on(
            &runtime,
            live_query_runtime
                .db()
                .cloudsync_network_sync(wait_ms, max_retries),
        )
        .map_err(cloudsync_error)?;
        serde_json::to_string(&result).map_err(serialization_error)
    }

    pub fn configure_cloudsync(&self, config_json: String) -> Result<(), BridgeError> {
        let config: anlg_db_core::CloudsyncRuntimeConfig = serde_json::from_str(&config_json)
            .map_err(|error| BridgeError::InvalidCloudsyncConfigJson {
                reason: error.to_string(),
            })?;
        let (runtime, live_query_runtime) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
            ))
        })?;
        block_on(
            &runtime,
            live_query_runtime.db().cloudsync_configure(config),
        )
        .map_err(cloudsync_runtime_error)
    }

    pub fn generate_e2ee_recovery_key(&self) -> Result<String, BridgeError> {
        self.with_state(|_| Ok(()))?;
        let recovery_key = anlg_e2ee::RecoveryKey::generate().map_err(cloudsync_error)?;
        Ok(recovery_key.expose_code().to_string())
    }

    pub fn inspect_e2ee_recovery_key(
        &self,
        recovery_key_code: String,
    ) -> Result<String, BridgeError> {
        self.with_state(|_| Ok(()))?;
        let recovery_key =
            anlg_e2ee::RecoveryKey::parse(&recovery_key_code).map_err(cloudsync_error)?;
        let member_public_key = recovery_key
            .member_identity_key()
            .map_err(cloudsync_error)?
            .public_key();
        serde_json::to_string(&serde_json::json!({
            "keyId": recovery_key.key_id(),
            "memberPublicKey": member_public_key,
        }))
        .map_err(serialization_error)
    }

    pub fn configure_e2ee_replica(
        &self,
        workspace_id: String,
        witness_endpoint: String,
        witness_access_token: String,
        recovery_key_code: String,
    ) -> Result<String, BridgeError> {
        let recovery_key =
            anlg_e2ee::RecoveryKey::parse(&recovery_key_code).map_err(cloudsync_error)?;
        let (runtime, db, e2ee_sync_hook) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.db),
                Arc::clone(&state.e2ee_sync_hook),
            ))
        })?;
        let result = block_on(&runtime, async {
            e2ee_sync_hook.clear();
            if db.cloudsync_enabled() {
                db.cloudsync_stop().await.map_err(cloudsync_runtime_error)?;
            }
            e2ee_sync_hook
                .set_personal_workspace(&workspace_id, &recovery_key)
                .map_err(cloudsync_error)?;
            let claimed = match anlg_db_app::cloudsync_workspace_is_claimed_by(
                db.pool(),
                &workspace_id,
            )
            .await
            {
                Ok(claimed) => claimed,
                Err(error) if anlg_db_sync::is_permanent_cloudsync_workspace_rejection(&error) => {
                    e2ee_sync_hook.clear();
                    return Ok("account_mismatch".to_string());
                }
                Err(error) => return Err(cloudsync_error(error)),
            };
            if !claimed {
                match anlg_db_app::claim_cloudsync_workspace(db.pool(), &workspace_id).await {
                    Ok(()) => {}
                    Err(error)
                        if anlg_db_sync::is_permanent_cloudsync_workspace_rejection(&error) =>
                    {
                        e2ee_sync_hook.clear();
                        return Ok("account_mismatch".to_string());
                    }
                    Err(error) => return Err(cloudsync_error(error)),
                }
            }
            let witness = anlg_db_sync::E2eeWitnessClient::new(
                anlg_db_sync::E2eeWitnessConfig {
                    endpoint: witness_endpoint,
                    access_token: witness_access_token,
                },
                &workspace_id,
            )
            .map_err(cloudsync_error)?;
            let key = e2ee_sync_hook
                .workspace_key(&workspace_id)
                .ok_or_else(|| cloudsync_error("E2EE replica identity is not configured"))?;
            let cancellation = anlg_db_sync::E2eeWitnessCancellation::default();
            e2ee_sync_hook
                .prepare_local_snapshot(db.pool(), &cancellation)
                .await
                .map_err(cloudsync_error)?;
            witness
                .initialize_cancellable(db.pool(), &key, &cancellation)
                .await
                .map_err(cloudsync_error)?;
            e2ee_sync_hook.set_replica_witness(witness);
            Ok("configured".to_string())
        });
        if result.is_err() {
            e2ee_sync_hook.clear();
        }
        result
    }

    pub fn start_cloudsync(&self) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime, e2ee_sync_hook) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
                Arc::clone(&state.e2ee_sync_hook),
            ))
        })?;
        if e2ee_sync_hook.replica_transport_configured() {
            e2ee_sync_hook.request_reconciliation();
            e2ee_sync_hook.request_replica_sync();
            return Ok(());
        }
        block_on(&runtime, live_query_runtime.db().cloudsync_start())
            .map_err(cloudsync_runtime_error)
    }

    pub fn stop_cloudsync(&self) -> Result<(), BridgeError> {
        let (runtime, live_query_runtime, e2ee_sync_hook) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
                Arc::clone(&state.e2ee_sync_hook),
            ))
        })?;
        if e2ee_sync_hook.replica_transport_configured() {
            e2ee_sync_hook.clear();
            return Ok(());
        }
        block_on(&runtime, live_query_runtime.db().cloudsync_stop())
            .map_err(cloudsync_runtime_error)
    }

    pub fn cloudsync_status(&self) -> Result<String, BridgeError> {
        let (runtime, live_query_runtime, e2ee_sync_hook) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
                Arc::clone(&state.e2ee_sync_hook),
            ))
        })?;
        if e2ee_sync_hook.replica_transport_configured() {
            let keys = e2ee_sync_hook.snapshot();
            let local_work_pending = block_on(
                &runtime,
                anlg_db_app::has_pending_e2ee_dirty_rows_deferring_active_captures(
                    live_query_runtime.db().pool(),
                    &keys,
                ),
            )
            .map_err(cloudsync_error)?;
            let replica = e2ee_sync_hook.replica_status();
            return serde_json::to_string(&serde_json::json!({
                "cloudsync_enabled": true,
                "extension_loaded": live_query_runtime.db().cloudsync_enabled(),
                "configured": true,
                "running": true,
                "network_initialized": true,
                "activity_paused": e2ee_sync_hook.activity_paused(),
                "deferred_for_capture": false,
                "last_sync": null,
                "last_sync_at_ms": replica.last_sync_at_ms,
                "has_unsent_changes": local_work_pending || replica.syncing,
                "last_error": replica.last_error,
                "last_error_kind": (replica.consecutive_failures > 0).then_some("transient"),
                "consecutive_failures": replica.consecutive_failures,
                "recovery_pending": false,
                "recovery_delayed": false,
                "recovery_phase": null,
                "activity_log": [],
            }))
            .map_err(serialization_error);
        }
        let status = block_on(&runtime, live_query_runtime.db().cloudsync_status())
            .map_err(cloudsync_runtime_error)?;
        serde_json::to_string(&status).map_err(serialization_error)
    }

    pub fn cloudsync_sync_now(&self) -> Result<String, BridgeError> {
        let (runtime, live_query_runtime, e2ee_sync_hook) = self.with_state(|state| {
            Ok((
                Arc::clone(&state.runtime),
                Arc::clone(&state.live_query_runtime),
                Arc::clone(&state.e2ee_sync_hook),
            ))
        })?;
        if e2ee_sync_hook.replica_transport_configured() {
            e2ee_sync_hook.request_replica_sync();
            return Ok("{}".to_string());
        }
        let result = block_on(&runtime, live_query_runtime.db().cloudsync_trigger_sync())
            .map_err(cloudsync_runtime_error)?;
        serde_json::to_string(&result).map_err(serialization_error)
    }

    pub fn close(&self) -> Result<(), BridgeError> {
        let mut guard = self.state.lock().unwrap();
        let Some(mut state) = guard.take() else {
            return Ok(());
        };
        drop(guard);

        let subscription_ids: Vec<String> = state.subscription_ids.drain().collect();
        let pool = state.live_query_runtime.db().pool().clone();
        block_on(&state.runtime, async {
            for subscription_id in subscription_ids {
                let _ = state.live_query_runtime.unsubscribe(&subscription_id).await;
            }
            let _ = state.live_query_runtime.db().cloudsync_stop().await;
        });
        state.e2ee_sync_hook.clear();
        drop(state.witness_watch);
        drop(state.replica_sync);
        drop(state.live_query_runtime);
        drop(state.executor);
        drop(state.db);
        block_on(&state.runtime, pool.close());

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
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
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

    const REENTRANT_SUBSCRIBE_CHILD_ENV: &str = "MOBILE_BRIDGE_REENTRANT_SUBSCRIBE_CHILD";

    fn cloudsync_config_json() -> String {
        r#"{
            "connection_string":"sqlitecloud://demo.invalid/app.db?apikey=demo",
            "auth":{"type":"none"},
            "tables":[{"table_name":"templates","crdt_algo":null,"init_flags":null,"enabled":false}],
            "sync_interval_ms":30000,
            "wait_ms":1000,
            "max_retries":1
        }"#
        .to_string()
    }

    #[test]
    fn execute_roundtrips_rows() {
        let (_dir, bridge) = new_bridge(None);

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-1","Weekly"]"#.to_string(),
            )
            .unwrap();

        let rows_json = bridge
            .execute(
                "SELECT id, title FROM templates WHERE id = ?".to_string(),
                r#"["template-1"]"#.to_string(),
            )
            .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&rows_json).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "template-1");
        assert_eq!(rows[0]["title"], "Weekly");
    }

    #[test]
    fn execute_proxy_roundtrips_positional_rows() {
        let (_dir, bridge) = new_bridge(None);

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-1","Weekly"]"#.to_string(),
            )
            .unwrap();

        let result_json = bridge
            .execute_proxy(
                "SELECT id, title FROM templates WHERE id = ?".to_string(),
                r#"["template-1"]"#.to_string(),
                "all".to_string(),
            )
            .unwrap();
        let result: anlg_db_execute::ProxyQueryResult = serde_json::from_str(&result_json).unwrap();

        assert_eq!(
            result.rows,
            vec![serde_json::json!(["template-1", "Weekly"])]
        );
    }

    #[test]
    fn execute_transaction_roundtrips_affected_rows_and_rolls_back() {
        let (_dir, bridge) = new_bridge(None);

        let affected_json = bridge
            .execute_transaction(
                r#"[
                    {"sql":"INSERT INTO templates (id, title) VALUES (?, ?)","params":["template-1","Weekly"],"expectedRowsAffected":1},
                    {"sql":"UPDATE templates SET title = ? WHERE id = ?","params":["Retro","template-1"],"expectedRowsAffected":1}
                ]"#
                    .to_string(),
            )
            .unwrap();
        assert_eq!(affected_json, "[1,1]");

        let error = bridge
            .execute_transaction(
                r#"[
                    {"sql":"INSERT INTO templates (id, title) VALUES (?, ?)","params":["rolled-back","Draft"]},
                    {"sql":"UPDATE templates SET title = ? WHERE id = ?","params":["Missing","missing"],"expectedRowsAffected":1}
                ]"#
                    .to_string(),
            )
            .unwrap_err();
        assert!(matches!(error, BridgeError::QueryFailed { .. }));

        let rows = bridge
            .execute(
                "SELECT id FROM templates WHERE id = ?".to_string(),
                r#"["rolled-back"]"#.to_string(),
            )
            .unwrap();
        assert_eq!(rows, "[]");
    }

    #[test]
    fn subscribe_reruns_after_write() {
        let (_dir, bridge) = new_bridge(None);
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, title FROM templates WHERE id = ?".to_string(),
                r#"["template-live"]"#.to_string(),
                listener,
            )
            .unwrap();

        let initial = next_event(&events, 0);
        assert_eq!(initial, TestEvent::Result(Vec::new()));

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-live","Retro"]"#.to_string(),
            )
            .unwrap();

        let refresh = next_event(&events, 1);
        let TestEvent::Result(rows) = refresh else {
            panic!("expected result event");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "template-live");

        bridge.unsubscribe(subscription_id).unwrap();
    }

    #[test]
    fn subscribe_listener_can_reenter_bridge_without_deadlock() {
        if std::env::var_os(REENTRANT_SUBSCRIBE_CHILD_ENV).is_some() {
            #[derive(Clone)]
            struct ReentrantListener {
                bridge: std::sync::Weak<MobileDbBridge>,
                callback_completed: Arc<AtomicBool>,
            }

            impl QueryEventListener for ReentrantListener {
                fn on_result(&self, _rows_json: String) {
                    let bridge = self
                        .bridge
                        .upgrade()
                        .expect("bridge should be alive during callback");
                    bridge.configure_cloudsync(cloudsync_config_json()).unwrap();
                    self.callback_completed.store(true, Ordering::SeqCst);
                }

                fn on_error(&self, message: String) {
                    panic!("unexpected error callback: {message}");
                }
            }

            let (_dir, bridge) = new_bridge(None);
            let bridge = Arc::new(bridge);
            let callback_completed = Arc::new(AtomicBool::new(false));
            let listener = Arc::new(ReentrantListener {
                bridge: Arc::downgrade(&bridge),
                callback_completed: Arc::clone(&callback_completed),
            });

            let subscription_id = bridge
                .subscribe(
                    "SELECT id, title FROM templates ORDER BY id".to_string(),
                    "[]".to_string(),
                    listener,
                )
                .unwrap();
            assert!(callback_completed.load(Ordering::SeqCst));
            bridge.unsubscribe(subscription_id).unwrap();
            return;
        }

        let current_exe = std::env::current_exe().unwrap();
        let mut child = Command::new(current_exe)
            .arg("--exact")
            .arg("tests::subscribe_listener_can_reenter_bridge_without_deadlock")
            .arg("--nocapture")
            .env(REENTRANT_SUBSCRIBE_CHILD_ENV, "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);

        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "child test failed with status {status}");
                break;
            }

            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("child test timed out; subscribe listener re-entry likely deadlocked");
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn unsubscribe_stops_future_events() {
        let (_dir, bridge) = new_bridge(None);
        let (listener, events) = TestListener::capture();

        let subscription_id = bridge
            .subscribe(
                "SELECT id, title FROM templates ORDER BY id".to_string(),
                "[]".to_string(),
                listener,
            )
            .unwrap();

        next_event(&events, 0);
        bridge.unsubscribe(subscription_id).unwrap();

        bridge
            .execute(
                "INSERT INTO templates (id, title) VALUES (?, ?)".to_string(),
                r#"["template-after-unsub","Standup"]"#.to_string(),
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

        bridge.configure_cloudsync(cloudsync_config_json()).unwrap();
        bridge.start_cloudsync().unwrap();

        let status: serde_json::Value =
            serde_json::from_str(&bridge.cloudsync_status().unwrap()).unwrap();
        assert_eq!(status["cloudsync_enabled"], false);
        assert_eq!(status["configured"], true);
        assert_eq!(status["running"], false);
        assert_eq!(status["network_initialized"], false);

        assert_eq!(bridge.cloudsync_sync_now().unwrap(), "{}");
        bridge.stop_cloudsync().unwrap();
    }

    #[test]
    fn recovery_key_identity_roundtrips() {
        let (_dir, bridge) = new_bridge(None);

        let recovery_key = bridge.generate_e2ee_recovery_key().unwrap();
        let identity: serde_json::Value = serde_json::from_str(
            &bridge
                .inspect_e2ee_recovery_key(recovery_key.clone())
                .unwrap(),
        )
        .unwrap();
        let parsed = anlg_e2ee::RecoveryKey::parse(&recovery_key).unwrap();

        assert_eq!(identity["keyId"], parsed.key_id());
        assert_eq!(
            identity["memberPublicKey"],
            parsed.member_identity_key().unwrap().public_key()
        );
    }

    #[test]
    fn inspect_e2ee_recovery_key_rejects_invalid_input() {
        let (_dir, bridge) = new_bridge(None);

        let error = bridge
            .inspect_e2ee_recovery_key("invalid-recovery-key".to_string())
            .unwrap_err();

        assert!(matches!(error, BridgeError::CloudsyncFailed { .. }));
    }

    #[test]
    fn configure_e2ee_replica_rejects_invalid_recovery_key() {
        let (_dir, bridge) = new_bridge(None);

        let error = bridge
            .configure_e2ee_replica(
                "user-a".to_string(),
                "https://api.example.com/sync/e2ee/witness/user-a".to_string(),
                "access-token".to_string(),
                "invalid-recovery-key".to_string(),
            )
            .unwrap_err();

        assert!(matches!(error, BridgeError::CloudsyncFailed { .. }));
    }

    #[test]
    fn configure_e2ee_replica_refuses_another_bound_account() {
        let (_dir, bridge) = new_bridge(None);
        bridge
            .execute(
                "UPDATE app_settings SET value_json = ? WHERE id = 'cloudsync_workspace_binding'"
                    .to_string(),
                r#"["{\"workspace_id\":\"user-a\",\"account_user_id\":\"user-a\"}"]"#.to_string(),
            )
            .unwrap();
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();

        let result = bridge
            .configure_e2ee_replica(
                "user-b".to_string(),
                "https://api.example.com/sync/e2ee/witness/user-b".to_string(),
                "access-token".to_string(),
                recovery_key.expose_code().to_string(),
            )
            .unwrap();

        assert_eq!(result, "account_mismatch");
    }

    #[test]
    fn configure_e2ee_replica_reports_an_invalid_binding_as_account_mismatch() {
        let (_dir, bridge) = new_bridge(None);
        bridge
            .execute(
                "UPDATE app_settings SET value_json = ? WHERE id = 'cloudsync_workspace_binding'"
                    .to_string(),
                r#"["not-json"]"#.to_string(),
            )
            .unwrap();
        let recovery_key = anlg_e2ee::RecoveryKey::generate().unwrap();

        let result = bridge
            .configure_e2ee_replica(
                "user-a".to_string(),
                "https://api.example.com/sync/e2ee/witness/user-a".to_string(),
                "access-token".to_string(),
                recovery_key.expose_code().to_string(),
            )
            .unwrap();

        assert_eq!(result, "account_mismatch");
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
