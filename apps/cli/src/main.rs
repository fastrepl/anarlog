use std::process::ExitCode;

use anarlog_cli::Args;
use clap::Parser;
use clap::error::ErrorKind;

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

    match anarlog_cli::run(args).await {
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
