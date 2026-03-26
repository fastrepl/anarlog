mod chat;
mod helpers;
mod humans;
mod meeting;
mod meetings;
mod notes;
mod orgs;
mod transcript;

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};
use sqlx::SqlitePool;

use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Export a single meeting with all associated data
    Meeting {
        #[arg(long)]
        id: String,
        #[arg(short = 'f', long, value_enum, default_value = "json")]
        format: DocFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export meeting list
    Meetings {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: TableFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export transcript for a meeting
    Transcript {
        #[arg(long)]
        meeting: String,
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: TranscriptFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export notes for a meeting
    Notes {
        #[arg(long)]
        meeting: String,
        #[arg(short = 'f', long, value_enum, default_value = "markdown")]
        format: DocFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export chat messages for a meeting
    Chat {
        #[arg(long)]
        meeting: String,
        #[arg(short = 'f', long, value_enum, default_value = "markdown")]
        format: DocFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export all contacts
    Humans {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: TableFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Export all organizations
    Orgs {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: TableFormat,
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DocFormat {
    Json,
    Markdown,
    Text,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TableFormat {
    Json,
    Csv,
    Text,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TranscriptFormat {
    Json,
    Text,
    Srt,
    Vtt,
}

pub async fn run(pool: &SqlitePool, command: Commands) -> CliResult<()> {
    match command {
        Commands::Meeting { id, format, output } => {
            meeting::meeting(pool, &id, format, output.as_deref()).await
        }
        Commands::Meetings { format, output } => {
            meetings::meetings(pool, format, output.as_deref()).await
        }
        Commands::Transcript {
            meeting,
            format,
            output,
        } => transcript::transcript(pool, &meeting, format, output.as_deref()).await,
        Commands::Notes {
            meeting,
            format,
            output,
        } => notes::notes(pool, &meeting, format, output.as_deref()).await,
        Commands::Chat {
            meeting,
            format,
            output,
        } => chat::chat(pool, &meeting, format, output.as_deref()).await,
        Commands::Humans { format, output } => {
            humans::humans(pool, format, output.as_deref()).await
        }
        Commands::Orgs { format, output } => orgs::orgs(pool, format, output.as_deref()).await,
    }
}
