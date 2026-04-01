use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};

/// Live transcription and audio tools
#[derive(Parser)]
#[command(
    name = "char",
    version = option_env!("APP_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
    propagate_version = true,
    after_help = "Docs:        https://cli.char.com\nDiscussions: https://github.com/fastrepl/char/discussions/4788\nBugs:        https://github.com/fastrepl/char/issues"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(long)]
    pub no_color: bool,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

pub(crate) fn parse_base_url(value: &str) -> Result<String, String> {
    let parsed = url::Url::parse(value).map_err(|e| format!("invalid URL '{value}': {e}"))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(format!(
            "invalid URL '{value}': scheme must be http or https"
        ));
    }
    Ok(value.to_string())
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
}

#[derive(Subcommand, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Commands {
    /// Transcribe an audio file
    Transcribe {
        #[command(flatten)]
        args: crate::commands::transcribe::Args,
    },
    #[cfg(feature = "standalone")]
    /// Manage local models
    Models {
        #[command(flatten)]
        args: crate::commands::model::Args,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    #[cfg(feature = "standalone")]
    /// Play an audio file
    Play {
        #[command(flatten)]
        args: crate::commands::play::Args,
    },
    #[cfg(feature = "standalone")]
    /// Record audio to an MP3 file
    Record {
        #[command(flatten)]
        args: crate::commands::record::Args,
    },
    #[cfg(feature = "standalone")]
    /// Install char skill for AI coding agents
    Skill {
        #[command(subcommand)]
        command: crate::commands::skill::Commands,
    },
    #[cfg(feature = "standalone")]
    /// Open the desktop app or download page
    Desktop,
    #[cfg(feature = "standalone")]
    /// Report a bug on GitHub
    #[command(hide = true)]
    Bug,
    #[cfg(feature = "standalone")]
    /// Open char.com
    #[command(hide = true)]
    Hello,
    #[cfg(feature = "standalone")]
    /// Update char to the latest version
    Update,
    #[cfg(all(feature = "standalone", target_os = "macos"))]
    #[command(hide = true)]
    ShortcutDaemon,

    #[cfg(feature = "todo")]
    /// Manage todos and automations
    Todo {
        #[command(subcommand)]
        command: Option<crate::commands::todo::Commands>,
    },
    #[cfg(feature = "desktop")]
    /// Browse past meetings
    Meetings {
        #[command(subcommand)]
        command: crate::commands::meetings::Commands,
    },
    #[cfg(feature = "desktop")]
    /// Browse humans (contacts)
    Humans {
        #[command(subcommand)]
        command: Option<crate::commands::humans::Commands>,
    },
    #[cfg(feature = "desktop")]
    /// Browse organizations
    Orgs {
        #[command(subcommand)]
        command: Option<crate::commands::orgs::Commands>,
    },
    #[cfg(feature = "desktop")]
    /// Export data in various formats
    Export {
        #[command(subcommand)]
        command: crate::commands::export::Commands,
    },
}

impl Commands {
    pub fn base_override(&self) -> Option<&std::path::Path> {
        match self {
            Commands::Transcribe { args } => args.base.as_deref(),
            #[cfg(feature = "standalone")]
            Commands::Models { args } => args.base.as_deref(),
            #[cfg(feature = "standalone")]
            Commands::Play { args } => args.base.as_deref(),
            #[cfg(feature = "standalone")]
            Commands::Record { args } => args.base.as_deref(),
            _ => None,
        }
    }
}

pub fn generate_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "char", &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_help(command: &mut clap::Command) -> String {
        let mut bytes = Vec::new();
        command.write_long_help(&mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn root_help_only_shows_truly_global_options() {
        let mut command = Cli::command();
        let help = render_help(&mut command);

        assert!(help.contains("--no-color"));
        assert!(!help.contains("--base-url"));
        assert!(!help.contains("--api-key"));
        assert!(!help.contains("--model <MODEL>"));
        assert!(!help.contains("--language <LANGUAGE>"));
        assert!(!help.contains("--base <DIR>"));
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn transcribe_help_keeps_stt_config() {
        let mut command = Cli::command();
        let mut transcribe = command.find_subcommand_mut("transcribe").unwrap().clone();
        let help = render_help(&mut transcribe);

        assert!(help.contains("--base-url"));
        assert!(help.contains("--api-key"));
        assert!(help.contains("--model <MODEL>"));
        assert!(help.contains("--language <LANGUAGE>"));
        assert!(help.contains("--base <DIR>"));
    }

    #[test]
    #[cfg(all(feature = "standalone", target_os = "macos"))]
    fn transcribe_accepts_whispercpp_provider() {
        let input = tempfile::NamedTempFile::new().unwrap();
        Cli::try_parse_from([
            "char",
            "transcribe",
            "--input",
            input.path().to_str().unwrap(),
            "--provider",
            "whispercpp",
        ])
        .unwrap();
    }

    #[test]
    #[cfg(all(
        feature = "standalone",
        target_os = "macos",
        any(target_arch = "arm", target_arch = "aarch64")
    ))]
    fn transcribe_accepts_cactus_provider_on_apple_silicon() {
        let input = tempfile::NamedTempFile::new().unwrap();
        Cli::try_parse_from([
            "char",
            "transcribe",
            "--input",
            input.path().to_str().unwrap(),
            "--provider",
            "cactus",
        ])
        .unwrap();
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn models_help_only_shows_base_override() {
        let mut command = Cli::command();
        let mut models = command.find_subcommand_mut("models").unwrap().clone();
        let help = render_help(&mut models);

        assert!(help.contains("--base <DIR>"));
        assert!(help.contains("list"));
        assert!(help.contains("download"));
        assert!(help.contains("delete"));
        assert!(!help.contains("paths"));
        assert!(!help.contains("cactus"));
        assert!(!help.contains("--supported"));
        assert!(!help.contains("--base-url"));
        assert!(!help.contains("--api-key"));
        assert!(!help.contains("--model <MODEL>"));
        assert!(!help.contains("--language <LANGUAGE>"));
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn record_rejects_stt_only_flags() {
        match Cli::try_parse_from(["char", "record", "--api-key", "secret"]) {
            Ok(_) => panic!("record unexpectedly accepted --api-key"),
            Err(error) => assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument),
        }
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn model_delete_uses_long_only_force() {
        Cli::try_parse_from(["char", "models", "delete", "tiny", "--force"]).unwrap();

        match Cli::try_parse_from(["char", "models", "delete", "tiny", "-f"]) {
            Ok(_) => panic!("models delete unexpectedly accepted -f"),
            Err(error) => assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument),
        }
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn models_list_accepts_json_format() {
        let mut command = Cli::command();
        let mut list = command
            .find_subcommand_mut("models")
            .unwrap()
            .find_subcommand_mut("list")
            .unwrap()
            .clone();
        let help = render_help(&mut list);

        assert!(help.contains("json"));
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn generate_docs_standalone() {
        let cmd = Cli::command();
        let json = cli_docs::generate_json(&cmd);

        let json_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cli-web/src/data/cli.gen.json");
        std::fs::create_dir_all(json_path.parent().unwrap()).unwrap();
        std::fs::write(&json_path, json).unwrap();
    }

    #[test]
    #[cfg(feature = "desktop")]
    fn generate_docs_desktop() {
        let cmd = Cli::command();
        let md = cli_docs::generate(&cmd);

        let frontmatter = "\
---
title: \"CLI Reference\"
section: \"CLI\"
description: \"Command-line reference for the char CLI\"
---\n\n";

        let mdx_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../web/content/docs/cli/index.mdx");
        std::fs::create_dir_all(mdx_path.parent().unwrap()).unwrap();
        std::fs::write(&mdx_path, format!("{frontmatter}{md}")).unwrap();
    }
}
