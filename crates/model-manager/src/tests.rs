use std::{path::Path, path::PathBuf, sync::Arc, time::Duration};

use crate::{ModelLoader, ModelManager, ModelStatus, TryGetResult};

struct MockModel;

impl ModelLoader for MockModel {
    type Error = String;

    fn load(_path: &Path) -> Result<Self, Self::Error> {
        Ok(MockModel)
    }
}

fn temp_model_path() -> PathBuf {
    let dir = std::env::temp_dir().join("cactus-model-manager-tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.bin", uuid::Uuid::new_v4()));
    std::fs::write(&path, b"").unwrap();
    path
}

fn build_manager(
    timeout: Duration,
    check_interval: Duration,
    models: &[(&str, PathBuf)],
) -> ModelManager<MockModel> {
    let mut builder = ModelManager::<MockModel>::builder()
        .inactivity_timeout(timeout)
        .check_interval(check_interval);
    for (name, path) in models {
        builder = builder.register(*name, path.clone());
    }
    builder.build()
}

async fn wait_for_try_get_ready(mgr: &ModelManager<MockModel>) {
    for _ in 0..50 {
        if matches!(mgr.try_get(None).await, TryGetResult::Ready(_)) {
            return;
        }
        tokio::task::yield_now().await;
    }

    panic!("model did not become ready via try_get");
}

async fn wait_for_status_ready(mgr: &ModelManager<MockModel>) {
    for _ in 0..50 {
        if matches!(mgr.status(None).await, ModelStatus::Ready(_)) {
            return;
        }
        tokio::task::yield_now().await;
    }

    panic!("model did not become ready via status");
}

#[tokio::test(start_paused = true)]
async fn idle_model_gets_evicted() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );

    let m1 = mgr.get(Some("a")).await.unwrap();
    let m2 = mgr.get(Some("a")).await.unwrap();
    assert!(Arc::ptr_eq(&m1, &m2));

    tokio::time::advance(Duration::from_millis(120)).await;
    tokio::task::yield_now().await;

    let m3 = mgr.get(Some("a")).await.unwrap();
    assert!(!Arc::ptr_eq(&m1, &m3));
}

#[tokio::test(start_paused = true)]
async fn activity_prevents_eviction() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );

    let m1 = mgr.get(Some("a")).await.unwrap();

    for _ in 0..5 {
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;

        let m = mgr.get(Some("a")).await.unwrap();
        assert!(Arc::ptr_eq(&m1, &m));
    }
}

#[tokio::test(start_paused = true)]
async fn access_near_timeout_resets_timer() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );

    let m1 = mgr.get(Some("a")).await.unwrap();

    tokio::time::advance(Duration::from_millis(90)).await;
    tokio::task::yield_now().await;

    let m2 = mgr.get(Some("a")).await.unwrap();
    assert!(Arc::ptr_eq(&m1, &m2));

    tokio::time::advance(Duration::from_millis(50)).await;
    tokio::task::yield_now().await;

    let m3 = mgr.get(Some("a")).await.unwrap();
    assert!(Arc::ptr_eq(&m1, &m3));
}

#[tokio::test(start_paused = true)]
async fn try_get_returns_loading_then_ready() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );
    mgr.set_default("a").await;

    // First try_get should start loading
    let result = mgr.try_get(None).await;
    assert!(matches!(result, TryGetResult::Loading));

    wait_for_try_get_ready(&mgr).await;
}

#[tokio::test(start_paused = true)]
async fn try_get_not_registered() {
    let mgr = build_manager(Duration::from_millis(100), Duration::from_millis(10), &[]);

    let result = mgr.try_get(Some("nonexistent")).await;
    assert!(matches!(result, TryGetResult::NotRegistered));
}

#[tokio::test(start_paused = true)]
async fn try_get_after_eviction_reloads() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );
    mgr.set_default("a").await;

    // Load via blocking get
    let _m1 = mgr.get(Some("a")).await.unwrap();

    // Evict
    tokio::time::advance(Duration::from_millis(120)).await;
    tokio::task::yield_now().await;

    // try_get should trigger reload
    let result = mgr.try_get(None).await;
    assert!(matches!(result, TryGetResult::Loading));
}

#[tokio::test(start_paused = true)]
async fn status_is_idle_before_first_request() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );
    mgr.set_default("a").await;

    let result = mgr.status(None).await;
    assert!(matches!(result, ModelStatus::Idle));
}

#[tokio::test(start_paused = true)]
async fn status_reports_loading_without_starting_load() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );
    mgr.set_default("a").await;

    assert!(matches!(mgr.status(None).await, ModelStatus::Idle));
    assert!(matches!(mgr.status(None).await, ModelStatus::Idle));

    let result = mgr.try_get(None).await;
    assert!(matches!(result, TryGetResult::Loading));
    assert!(matches!(mgr.status(None).await, ModelStatus::Loading));
}

#[tokio::test(start_paused = true)]
async fn status_promotes_completed_load_without_refreshing_activity() {
    let path = temp_model_path();
    let mgr = build_manager(
        Duration::from_millis(100),
        Duration::from_millis(10),
        &[("a", path)],
    );
    mgr.set_default("a").await;

    let result = mgr.try_get(None).await;
    assert!(matches!(result, TryGetResult::Loading));

    wait_for_status_ready(&mgr).await;

    tokio::time::advance(Duration::from_millis(90)).await;
    tokio::task::yield_now().await;
    assert!(matches!(mgr.status(None).await, ModelStatus::Ready(_)));

    tokio::time::advance(Duration::from_millis(20)).await;
    tokio::task::yield_now().await;
    assert!(matches!(mgr.status(None).await, ModelStatus::Idle));
}
