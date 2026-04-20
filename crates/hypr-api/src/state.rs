use std::sync::Arc;

use hypr_db_core::Db;
use hypr_db_execute::DbExecutor;
use hypr_db_reactive::LiveQueryRuntime;

use crate::Result;
use crate::live::JsonRowsSink;

#[derive(Clone)]
pub struct AppState {
    db: Arc<Db>,
    executor: DbExecutor,
    live_query_runtime: Arc<LiveQueryRuntime<JsonRowsSink>>,
}

impl AppState {
    pub async fn open() -> Result<Self> {
        let db = Db::connect_memory_plain().await?;
        hypr_db_migrate::migrate(&db, hypr_db_app2::schema()).await?;

        let db = Arc::new(db);

        Ok(Self {
            executor: DbExecutor::new(Arc::clone(&db)),
            live_query_runtime: Arc::new(LiveQueryRuntime::new(Arc::clone(&db))),
            db,
        })
    }

    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }

    pub fn executor(&self) -> &DbExecutor {
        &self.executor
    }

    pub fn live_query_runtime(&self) -> &Arc<LiveQueryRuntime<JsonRowsSink>> {
        &self.live_query_runtime
    }
}
