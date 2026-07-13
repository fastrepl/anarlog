use sqlx::{Executor, Sqlite};

use super::CloudsyncAuth;
use crate::Db;

impl Db {
    pub fn cloudsync_enabled(&self) -> bool {
        self.cloudsync_enabled
    }

    pub fn has_cloudsync(&self) -> bool {
        self.cloudsync_path.is_some()
    }

    pub fn cloudsync_path(&self) -> Option<&std::path::Path> {
        self.cloudsync_path.as_deref()
    }

    pub async fn cloudsync_version(&self) -> Result<String, hypr_cloudsync::Error> {
        hypr_cloudsync::version(&self.pool).await
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        init_flags: Option<i64>,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::init(&self.pool, table_name, crdt_algo, init_flags).await
    }

    pub async fn cloudsync_network_init(
        &self,
        connection_string: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_init(&self.pool, connection_string).await
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_set_apikey(&self.pool, api_key).await
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_set_token(&self.pool, token).await
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        cloudsync_begin_alter_on(&self.pool, table_name).await
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        cloudsync_commit_alter_on(&self.pool, table_name).await
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::cleanup(&self.pool, table_name).await
    }

    pub async fn cloudsync_terminate(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::terminate(&self.pool).await
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_cleanup(&self.pool).await
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, hypr_cloudsync::Error> {
        hypr_cloudsync::network_has_unsent_changes(&self.pool).await
    }

    pub async fn cloudsync_network_send_changes(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _guard = sync_lock.lock().await;
        hypr_cloudsync::network_send_changes(&self.pool, wait_ms, max_retries).await
    }

    pub async fn cloudsync_network_check_changes(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _guard = sync_lock.lock().await;
        hypr_cloudsync::network_check_changes(&self.pool, wait_ms, max_retries).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_reset_sync_version(&self.pool).await
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), hypr_cloudsync::Error> {
        hypr_cloudsync::network_logout(&self.pool).await
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let sync_lock = self.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let _guard = sync_lock.lock().await;
        hypr_cloudsync::network_sync(&self.pool, wait_ms, max_retries).await
    }

    pub(crate) async fn apply_cloudsync_auth(
        &self,
        auth: &CloudsyncAuth,
    ) -> Result<(), hypr_cloudsync::Error> {
        match auth {
            CloudsyncAuth::None => Ok(()),
            CloudsyncAuth::ApiKey { api_key } => self.cloudsync_network_set_apikey(api_key).await,
            CloudsyncAuth::Token { token } => self.cloudsync_network_set_token(token).await,
        }
    }
}

pub async fn cloudsync_begin_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::begin_alter(executor, table_name).await
}

pub async fn cloudsync_commit_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::commit_alter(executor, table_name).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn network_sync_waits_for_single_flight_lock() {
        let db = Arc::new(Db::connect_memory_plain().await.unwrap());
        let sync_lock = db.cloudsync_runtime.lock().unwrap().sync_lock.clone();
        let guard = sync_lock.lock().await;
        let task_db = Arc::clone(&db);
        let mut task =
            tokio::spawn(async move { task_db.cloudsync_network_sync(None, None).await });

        assert!(
            tokio::time::timeout(Duration::from_millis(25), &mut task)
                .await
                .is_err()
        );

        drop(guard);
        assert!(
            tokio::time::timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
    }
}
