pub mod exp;
mod transcribe;

pub use crate::cli::DebugCommands;
use crate::error::CliResult;

pub async fn run(command: DebugCommands) -> CliResult<()> {
    match command {
        DebugCommands::Transcribe { args } => transcribe::run(args).await,
        DebugCommands::Exp => exp::run().await,
    }
}
