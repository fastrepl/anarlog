mod install;

use clap::Subcommand;

use crate::app::AppContext;
use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Install char skill for AI coding agents
    Install {
        /// Skip interactive selection: claude, codex, github-copilot
        #[arg(short, long)]
        format: Option<String>,
    },
}

pub async fn run(_ctx: &AppContext, command: Commands) -> CliResult<()> {
    match command {
        Commands::Install { format } => install::run(format.as_deref()),
    }
}
