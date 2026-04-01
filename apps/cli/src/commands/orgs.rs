pub use hypr_db_app::organization_cli::Commands;

use crate::app::AppContext;
use crate::error::CliResult;

pub async fn run(ctx: &AppContext, command: Option<Commands>) -> CliResult<()> {
    let pool = ctx.pool().await?;
    Ok(hypr_db_app::organization_cli::run(&pool, command).await?)
}
