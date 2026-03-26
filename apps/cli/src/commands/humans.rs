pub use hypr_db_app::human_cli::Commands;

use sqlx::SqlitePool;

use crate::error::CliResult;

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> CliResult<()> {
    Ok(hypr_db_app::human_cli::run(pool, command).await?)
}
