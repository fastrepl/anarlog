use sqlx::pool::PoolConnection;
use sqlx::{Executor, Sqlite};
use tokio::sync::MutexGuard;

use super::CloudsyncAuth;
use crate::Db;

impl Db {
    async fn lock_cloudsync_connection(
        &self,
    ) -> Result<MutexGuard<'_, Option<PoolConnection<Sqlite>>>, hypr_cloudsync::Error> {
        let mut connection = self.cloudsync_connection.lock().await;
        if connection.is_none() {
            *connection = Some(self.pool.acquire().await?);
        }
        Ok(connection)
    }

    fn release_single_pool_connection(
        &self,
        connection: &mut MutexGuard<'_, Option<PoolConnection<Sqlite>>>,
    ) {
        if self.pool.options().get_max_connections() == 1 {
            connection.take();
        }
    }

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
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_init(
        &self,
        table_name: &str,
        crdt_algo: Option<&str>,
        init_flags: Option<i64>,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::init(
            &mut **connection.as_mut().unwrap(),
            table_name,
            crdt_algo,
            init_flags,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_init(
        &self,
        connection_string: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_init(&mut **connection.as_mut().unwrap(), connection_string)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_apikey(
        &self,
        api_key: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_apikey(&mut **connection.as_mut().unwrap(), api_key).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_set_token(
        &self,
        token: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_set_token(&mut **connection.as_mut().unwrap(), token).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_begin_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_begin_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_commit_alter(
        &self,
        table_name: &str,
    ) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            cloudsync_commit_alter_on(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_cleanup(&self, table_name: &str) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::cleanup(&mut **connection.as_mut().unwrap(), table_name).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_terminate(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::terminate(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_cleanup(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_cleanup(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_has_unsent_changes(
        &self,
    ) -> Result<bool, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_has_unsent_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_send_changes(
        &self,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_send_changes(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_receive_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_receive_changes(
            &mut **connection.as_mut().unwrap(),
            max_chunks,
        )
        .await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_check_changes(
        &self,
        max_chunks: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        self.cloudsync_network_receive_changes(max_chunks).await
    }

    pub async fn cloudsync_network_reset_sync_version(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_reset_sync_version(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_logout(&self) -> Result<(), hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result = hypr_cloudsync::network_logout(&mut **connection.as_mut().unwrap()).await;
        self.release_single_pool_connection(&mut connection);
        result
    }

    pub async fn cloudsync_network_sync(
        &self,
        wait_ms: Option<i64>,
        max_retries: Option<i64>,
    ) -> Result<hypr_cloudsync::NetworkResult, hypr_cloudsync::Error> {
        let mut connection = self.lock_cloudsync_connection().await?;
        let result =
            hypr_cloudsync::network_sync(&mut **connection.as_mut().unwrap(), wait_ms, max_retries)
                .await;
        self.release_single_pool_connection(&mut connection);
        result
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

pub async fn cloudsync_is_enabled_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<bool, hypr_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    hypr_cloudsync::is_enabled(executor, table_name).await
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
    async fn network_calls_reuse_one_checked_out_connection() {
        let db = Arc::new(Db::connect_memory_plain().await.unwrap());
        {
            let mut connection = db.lock_cloudsync_connection().await.unwrap();
            sqlx::query("CREATE TEMP TABLE cloudsync_connection_marker (value INTEGER)")
                .execute(&mut **connection.as_mut().unwrap())
                .await
                .unwrap();
        }

        let mut connection = db.lock_cloudsync_connection().await.unwrap();
        let marker_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name = 'cloudsync_connection_marker'",
        )
        .fetch_one(&mut **connection.as_mut().unwrap())
        .await
        .unwrap();

        assert_eq!(marker_exists, 1);
    }

    #[tokio::test]
    async fn network_sync_waits_for_checked_out_connection() {
        let db = Arc::new(Db::connect_memory_plain().await.unwrap());
        let guard = db.lock_cloudsync_connection().await.unwrap();
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
