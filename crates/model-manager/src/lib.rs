mod builder;
mod error;
mod loader;
mod manager;

pub use builder::ModelManagerBuilder;
pub use error::Error;
pub use loader::ModelLoader;
pub use manager::ModelManager;

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc, time::Duration};

    use super::*;

    struct MockModel;

    #[derive(Debug, thiserror::Error)]
    #[error("mock error")]
    struct MockError;

    impl ModelLoader for MockModel {
        type Error = MockError;

        fn load(_path: &Path) -> Result<Self, Self::Error> {
            Ok(MockModel)
        }
    }

    fn temp_model_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("model-manager-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}.bin", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"").unwrap();
        path
    }

    fn build_manager(
        timeout: Duration,
        check_interval: Duration,
        models: &[(&str, std::path::PathBuf)],
    ) -> ModelManager<MockModel> {
        let mut builder = ModelManager::<MockModel>::builder()
            .inactivity_timeout(timeout)
            .check_interval(check_interval);
        for (name, path) in models {
            builder = builder.register(*name, path.clone());
        }
        builder.build()
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
    async fn access_after_timeout_before_monitor_tick_reloads() {
        let path = temp_model_path();
        let mgr = build_manager(
            Duration::from_millis(100),
            Duration::from_secs(60),
            &[("a", path)],
        );

        let m1 = mgr.get(Some("a")).await.unwrap();

        tokio::time::advance(Duration::from_millis(120)).await;
        tokio::task::yield_now().await;

        let m2 = mgr.get(Some("a")).await.unwrap();
        assert!(!Arc::ptr_eq(&m1, &m2));
    }
}
