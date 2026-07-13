use std::process::ExitCode;

use anarlog_cli::Args;
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    match anarlog_cli::run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
