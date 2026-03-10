mod commands;
mod error;
mod event;
mod frame;
mod terminal;
mod textarea_input;
mod theme;

use clap::{Parser, Subcommand};

use crate::commands::OutputFormat;
use crate::commands::batch::Provider as BatchProvider;
use crate::commands::model::ModelCommands;
use crate::error::{CliError, CliResult};

#[derive(Parser)]
#[command(name = "char", about = "Live transcription TUI and utilities", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long, env = "CHAR_BASE_URL", value_parser = parse_base_url)]
    base_url: Option<String>,

    #[arg(long, env = "CHAR_API_KEY", default_value = "")]
    api_key: String,

    #[arg(long, env = "CHAR_MODEL", default_value = "")]
    model: String,

    #[arg(long, env = "CHAR_LANGUAGE", default_value = "en")]
    language: String,

    #[arg(long, env = "CHAR_RECORD")]
    record: bool,
}

fn parse_base_url(value: &str) -> Result<String, String> {
    let parsed =
        url::Url::parse(value).map_err(|e| format!("invalid --base-url '{value}': {e}"))?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!(
            "invalid --base-url '{value}': scheme must be http or https"
        ));
    }

    Ok(value.to_string())
}

fn required_base_url(base_url: Option<String>) -> CliResult<String> {
    base_url.ok_or_else(|| CliError::required_argument("--base-url (or CHAR_BASE_URL)"))
}

#[derive(Subcommand)]
enum Commands {
    Listen,
    Auth,
    Desktop,
    #[command(about = "Transcribe an audio file (batch mode)")]
    Batch {
        #[arg(long, value_name = "PATH", visible_alias = "file")]
        input: std::path::PathBuf,
        #[arg(long, value_enum)]
        provider: BatchProvider,
        #[arg(long, short = 'k', value_name = "KEYWORD")]
        keyword: Vec<String>,
        #[arg(long, value_name = "PATH")]
        output: Option<std::path::PathBuf>,
        #[arg(long, value_enum, default_value = "pretty")]
        format: OutputFormat,
        #[arg(long, hide = true, conflicts_with = "format")]
        json: bool,
        #[arg(long, short = 'q')]
        quiet: bool,
    },
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(error) = run(cli).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> CliResult<()> {
    let Cli {
        command,
        base_url,
        api_key,
        model,
        language,
        record,
    } = cli;

    match command {
        Some(Commands::Auth) => commands::auth::run(),
        Some(Commands::Desktop) => commands::desktop::run().map(|_| ()),
        Some(Commands::Listen) => commands::listen::run(commands::listen::Args {
            base_url,
            api_key,
            model,
            language,
            record,
        })
        .await
        .map(|_| ()),
        Some(Commands::Batch {
            input,
            provider,
            keyword,
            output,
            json,
            format,
            quiet,
        }) => {
            let base_url = if matches!(provider, BatchProvider::Cactus) {
                base_url
            } else {
                Some(required_base_url(base_url)?)
            };

            commands::batch::run(commands::batch::Args {
                input,
                provider,
                base_url,
                api_key,
                model: if model.is_empty() { None } else { Some(model) },
                language,
                keywords: keyword,
                output,
                format: if json { OutputFormat::Json } else { format },
                quiet,
            })
            .await
        }
        Some(Commands::Model { command }) => commands::model::run(command).await,
        None => match commands::entry::run(commands::entry::Args {
            status_message: None,
        })
        .await
        {
            commands::entry::EntryAction::Listen => commands::listen::run(commands::listen::Args {
                base_url,
                api_key,
                model,
                language,
                record,
            })
            .await
            .map(|_| ()),
            commands::entry::EntryAction::Quit => Ok(()),
        },
    }
}
