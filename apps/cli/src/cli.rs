use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use anlg_agent_access::{DEFAULT_TRANSCRIPT_LIMIT, MAX_TRANSCRIPT_LIMIT};

#[derive(Debug, Parser)]
#[command(
    name = "anarlog",
    version = crate::VERSION,
    about = "Access Anarlog from the command line"
)]
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

impl Args {
    pub fn analytics_command_name(&self) -> &'static str {
        match &self.command {
            Command::Auth { command } => match command {
                AuthCommand::Login => "auth_login",
                AuthCommand::Status => "auth_status",
                AuthCommand::Logout => "auth_logout",
            },
            Command::Doctor => "doctor",
            Command::Meetings { command, .. } => match command {
                MeetingCommand::List { .. } => "meetings_list",
                MeetingCommand::Get { .. } => "meetings_get",
                MeetingCommand::Note { .. } => "meetings_note",
                MeetingCommand::Transcript { .. } => "meetings_transcript",
                MeetingCommand::History { .. } => "meetings_history",
                MeetingCommand::Export { .. } => "meetings_export",
            },
            Command::Proposals { command } => match command {
                ProposalCommand::Create { .. } => "proposals_create",
                ProposalCommand::List { .. } => "proposals_list",
                ProposalCommand::Show { .. } => "proposals_show",
                ProposalCommand::Decline { .. } => "proposals_decline",
            },
            Command::Mcp => "mcp",
        }
    }

    pub fn needs_write(&self) -> bool {
        matches!(
            self.command,
            Command::Proposals {
                command: ProposalCommand::Create { .. } | ProposalCommand::Decline { .. },
            } | Command::Mcp
        )
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in to an Anarlog account from a browser
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    /// Check the local CLI and database connection without changing data
    Doctor,
    /// Browse and export meetings
    Meetings {
        /// Choose local data, hosted Cloud snapshots, or Cloud only when no local database exists
        #[arg(long, env = "ANARLOG_SOURCE", value_enum, default_value_t = MeetingSource::Local)]
        source: MeetingSource,
        #[command(subcommand)]
        command: MeetingCommand,
    },
    /// Propose meeting edits for desktop review
    Proposals {
        #[command(subcommand)]
        command: ProposalCommand,
    },
    /// Run the Anarlog MCP server over stdio
    Mcp,
}

#[derive(Debug, Subcommand)]
pub enum ProposalCommand {
    /// Stage a summary or memo replacement for desktop review
    Create {
        #[arg(long = "meeting")]
        meeting_id: String,
        #[arg(long, value_enum)]
        kind: ProposalKind,
        #[arg(long = "target")]
        target_id: Option<String>,
        #[arg(long, required_unless_present = "content_file")]
        content: Option<String>,
        #[arg(long, value_name = "FILE", required_unless_present = "content")]
        content_file: Option<PathBuf>,
    },
    /// List staged meeting proposals
    List {
        #[arg(long = "meeting")]
        meeting_id: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=200), help = "Maximum results (1-200)")]
        limit: u32,
        #[arg(long, default_value_t = 0, help = "Number of results to skip")]
        offset: u32,
    },
    /// Show one proposal and its unified diff
    Show { id: String },
    /// Decline a pending proposal without changing the meeting
    Decline { id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ProposalKind {
    Summary,
    Memo,
}

impl ProposalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Memo => "memo",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum AuthCommand {
    /// Sign in through a browser on this or another device
    Login,
    /// Show the locally stored account session
    Status,
    /// Remove the locally stored account session
    Logout,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum MeetingSource {
    #[default]
    Local,
    Cloud,
    Auto,
}

#[derive(Debug, Subcommand)]
pub enum MeetingCommand {
    /// List meetings, optionally filtered by text or recurring series
    List {
        #[arg(short, long)]
        query: Option<String>,
        #[arg(long)]
        series_id: Option<String>,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=200), help = "Maximum results (1-200)")]
        limit: u32,
        #[arg(long, default_value_t = 0, help = "Number of results to skip")]
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
    /// Show a bounded page of a meeting transcript
    Transcript {
        id: String,
        #[arg(long, default_value_t = DEFAULT_TRANSCRIPT_LIMIT, value_parser = clap::value_parser!(u32).range(1..=MAX_TRANSCRIPT_LIMIT as i64), help = "Maximum transcript words (1-500)")]
        limit: u32,
        #[arg(long, default_value_t = 0, help = "Word offset")]
        offset: u32,
    },
    /// List meetings from the same recurring series
    History {
        id: String,
        #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=200), help = "Maximum meetings (1-200)")]
        limit: u32,
        #[arg(long, default_value_t = 0, help = "Number of meetings to skip")]
        offset: u32,
    },
    /// Export a meeting to Markdown or JSON
    Export {
        id: String,
        #[arg(long, value_enum, default_value_t = ExportFormat::Markdown)]
        format: ExportFormat,
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        #[arg(long, requires = "output", help = "Replace an existing output file")]
        force: bool,
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
    use clap::CommandFactory;

    #[test]
    fn parses_meeting_list_filters() {
        let args = Args::parse_from([
            "anarlog", "--json", "meetings", "list", "--query", "planning", "--limit", "10",
        ]);

        assert!(args.json);
        let Command::Meetings { command, .. } = args.command else {
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
        let help = Args::command().render_long_help().to_string();
        assert!(help.contains("auth"));
        assert!(help.contains("meetings"));
        assert!(help.contains("mcp"));
        assert!(help.contains("doctor"));
        assert!(help.contains("proposals"));

        let Command::Meetings { command, .. } = Args::parse_from([
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

    #[test]
    fn parses_auth_commands() {
        assert!(matches!(
            Args::parse_from(["anarlog", "auth", "login"]).command,
            Command::Auth {
                command: AuthCommand::Login
            }
        ));
        assert!(matches!(
            Args::parse_from(["anarlog", "auth", "status"]).command,
            Command::Auth {
                command: AuthCommand::Status
            }
        ));
        assert!(matches!(
            Args::parse_from(["anarlog", "auth", "logout"]).command,
            Command::Auth {
                command: AuthCommand::Logout
            }
        ));
    }

    #[test]
    fn parses_transcript_and_history_pagination() {
        let Command::Meetings { command, .. } = Args::parse_from([
            "anarlog",
            "meetings",
            "transcript",
            "meeting-1",
            "--offset",
            "25",
            "--limit",
            "100",
        ])
        .command
        else {
            panic!("expected meetings command");
        };
        assert!(matches!(
            command,
            MeetingCommand::Transcript {
                offset: 25,
                limit: 100,
                ..
            }
        ));

        let Command::Meetings { command, .. } = Args::parse_from([
            "anarlog",
            "meetings",
            "history",
            "meeting-1",
            "--offset",
            "10",
        ])
        .command
        else {
            panic!("expected meetings command");
        };
        assert!(matches!(
            command,
            MeetingCommand::History { offset: 10, .. }
        ));
    }

    #[test]
    fn export_force_requires_an_output_path() {
        assert!(
            Args::try_parse_from(["anarlog", "meetings", "export", "meeting-1", "--force"])
                .is_err()
        );
    }

    #[test]
    fn parses_cloud_and_auto_meeting_sources() {
        let Command::Meetings { source, .. } =
            Args::parse_from(["anarlog", "meetings", "--source", "cloud", "list"]).command
        else {
            panic!("expected meetings command");
        };
        assert_eq!(source, MeetingSource::Cloud);

        let Command::Meetings { source, .. } =
            Args::parse_from(["anarlog", "meetings", "--source", "auto", "list"]).command
        else {
            panic!("expected meetings command");
        };
        assert_eq!(source, MeetingSource::Auto);
    }

    #[test]
    fn public_docs_and_skill_cover_the_command_contract() {
        let docs = include_str!("../../../docs/reference/cli.mdx");
        let skill = concat!(
            include_str!("../../../skills/anarlog/references/cli.md"),
            include_str!("../../../skills/anarlog/references/setup.md"),
        );
        let command = Args::command();
        let mut paths = Vec::new();
        collect_leaf_commands(&command, "", &mut paths);

        for path in paths {
            assert!(docs.contains(&path), "CLI docs are missing `{path}`");
            assert!(skill.contains(&path), "Anarlog skill is missing `{path}`");
        }
        assert_options_are_documented(&command, docs);
    }

    #[test]
    fn cli_contract_matches_snapshot() {
        let contract: serde_json::Value =
            serde_json::from_str(&cli_docs::generate_json(&Args::command())).unwrap();
        insta::assert_json_snapshot!("cli_contract", canonicalize_json(contract));
    }

    #[test]
    fn version_uses_the_build_version() {
        assert_eq!(Args::command().get_version(), Some(crate::VERSION));
    }

    fn canonicalize_json(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(object) => serde_json::Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| (key, canonicalize_json(value)))
                    .collect::<std::collections::BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            serde_json::Value::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(canonicalize_json).collect())
            }
            value => value,
        }
    }

    fn collect_leaf_commands(command: &clap::Command, prefix: &str, paths: &mut Vec<String>) {
        for subcommand in command
            .get_subcommands()
            .filter(|subcommand| subcommand.get_name() != "help")
        {
            let path = if prefix.is_empty() {
                subcommand.get_name().to_string()
            } else {
                format!("{prefix} {}", subcommand.get_name())
            };
            if subcommand
                .get_subcommands()
                .any(|child| child.get_name() != "help")
            {
                collect_leaf_commands(subcommand, &path, paths);
            } else {
                paths.push(path);
            }
        }
    }

    fn assert_options_are_documented(command: &clap::Command, docs: &str) {
        for argument in command.get_arguments() {
            if let Some(long) = argument.get_long() {
                assert!(
                    docs.contains(&format!("--{long}")),
                    "CLI docs are missing `--{long}`"
                );
            }
        }
        for subcommand in command.get_subcommands() {
            assert_options_are_documented(subcommand, docs);
        }
    }
}
