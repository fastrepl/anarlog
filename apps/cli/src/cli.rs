use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "anarlog", version, about = "Query local Anarlog meeting data")]
pub struct Args {
    #[arg(
        long,
        global = true,
        env = "ANARLOG_BASE",
        hide_env_values = true,
        value_name = "DIR"
    )]
    pub base: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        env = "ANARLOG_DB_PATH",
        hide_env_values = true,
        value_name = "FILE"
    )]
    pub db_path: Option<PathBuf>,

    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Browse and export meetings
    Meetings {
        #[command(subcommand)]
        command: MeetingCommand,
    },
    /// Run the read-only Anarlog MCP server over stdio
    Mcp,
}

#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    /// List meetings, optionally filtered by text or recurring series
    List {
        #[arg(short, long)]
        query: Option<String>,
        #[arg(long)]
        series_id: Option<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=200))]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Show meeting metadata, notes, summaries, people, and action items
    Get { id: String },
    /// Show the note or generated summaries for a meeting
    Note {
        id: String,
        #[arg(long, value_enum, default_value_t = DocumentKind::Note)]
        kind: DocumentKind,
    },
    /// Show a meeting transcript
    Transcript { id: String },
    /// List other meetings from the same recurring series
    History {
        id: String,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=200))]
        limit: u32,
    },
    /// Export a meeting to Markdown or JSON
    Export {
        id: String,
        #[arg(long, value_enum, default_value_t = ExportFormat::Markdown)]
        format: ExportFormat,
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum DocumentKind {
    #[default]
    Note,
    Summary,
    All,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum ExportFormat {
    #[default]
    Markdown,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_meeting_list_filters() {
        let args = Args::parse_from([
            "anarlog", "--json", "meetings", "list", "--query", "planning", "--limit", "10",
        ]);

        assert!(args.json);
        let Command::Meetings { command } = args.command else {
            panic!("expected meetings command");
        };
        let MeetingCommand::List { query, limit, .. } = command else {
            panic!("expected list command");
        };
        assert_eq!(query.as_deref(), Some("planning"));
        assert_eq!(limit, 10);
    }

    #[test]
    fn help_exposes_mcp_and_export() {
        use clap::CommandFactory;

        let help = Args::command().render_long_help().to_string();
        assert!(help.contains("meetings"));
        assert!(help.contains("mcp"));

        let Command::Meetings { command } = Args::parse_from([
            "anarlog",
            "meetings",
            "export",
            "meeting-1",
            "--format",
            "json",
        ])
        .command
        else {
            panic!("expected meetings command");
        };
        assert!(matches!(
            command,
            MeetingCommand::Export {
                format: ExportFormat::Json,
                ..
            }
        ));
    }
}
