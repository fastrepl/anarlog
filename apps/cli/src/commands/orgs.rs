pub use hypr_db_app::organization_cli::Commands;

use sqlx::SqlitePool;

use crate::error::CliResult;

pub async fn run(pool: &SqlitePool, command: Option<Commands>) -> CliResult<()> {
    Ok(hypr_db_app::organization_cli::run(pool, command).await?)
}
