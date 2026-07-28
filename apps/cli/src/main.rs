use std::process::ExitCode;
use std::time::Instant;

use anarlog_cli::Args;
use clap::Parser;
use clap::error::ErrorKind;

mod analytics;

#[tokio::main]
async fn main() -> ExitCode {
    let json = std::env::args_os().any(|arg| arg == "--json");
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let exit_code = error.exit_code();
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) || !json
            {
                let _ = error.print();
            } else {
                eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "schema_version": anarlog_cli::JSON_SCHEMA_VERSION,
                        "error": {
                            "code": "invalid_arguments",
                            "message": error.to_string(),
                            "exit_code": exit_code,
                        }
                    }))
                    .expect("argument error response is always serializable")
                );
            }
            return ExitCode::from(exit_code as u8);
        }
    };

    let command = args.analytics_command_name();
    let started_at = Instant::now();
    let result = anarlog_cli::run(args).await;
    let outcome = match &result {
        Ok(0) => "succeeded",
        Ok(_) | Err(_) => "failed",
    };
    analytics::capture_command_completed(command, outcome, started_at.elapsed());

    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            if json {
                eprintln!("{}", error.to_json());
            } else {
                eprintln!("error: {error}");
            }
            ExitCode::from(error.exit_code())
        }
    }
}
