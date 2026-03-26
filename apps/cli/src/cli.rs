use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};

/// Live transcription and audio tools
#[derive(Parser)]
#[command(
    name = "char",
    version,
    propagate_version = true,
    after_help = "Docs:  https://char.com/docs/cli\nBugs:  https://github.com/fastrepl/char/issues"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(clap::Args)]
pub struct GlobalArgs {
    #[arg(long, global = true, env = "CHAR_BASE_URL", hide_env_values = true, value_parser = parse_base_url)]
    pub base_url: Option<String>,

    #[arg(long, global = true, env = "CHAR_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(
        short = 'm',
        long,
        global = true,
        env = "CHAR_MODEL",
        hide_env_values = true
    )]
    pub model: Option<String>,

    #[arg(
        short = 'l',
        long,
        global = true,
        env = "CHAR_LANGUAGE",
        hide_env_values = true,
        default_value = "en"
    )]
    pub language: String,

    #[arg(long, global = true)]
    pub no_color: bool,

    #[arg(
        long,
        global = true,
        env = "CHAR_BASE",
        hide_env_values = true,
        value_name = "DIR"
    )]
    pub base: Option<std::path::PathBuf>,
}

fn parse_base_url(value: &str) -> Result<String, String> {
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
        #[command(subcommand)]
        command: crate::commands::model::Commands,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    #[cfg(feature = "standalone")]
    /// Record audio to an MP3 file
    Record {
        #[command(flatten)]
        args: crate::commands::record::Args,
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

    #[cfg(feature = "task")]
    /// Claude Code integration
    Claude {
        #[command(subcommand)]
        command: crate::commands::claude::Commands,
    },
    #[cfg(feature = "task")]
    /// Codex integration
    Codex {
        #[command(subcommand)]
        command: crate::commands::codex::Commands,
    },
    #[cfg(feature = "task")]
    /// OpenCode integration
    Opencode {
        #[command(subcommand)]
        command: crate::commands::opencode::Commands,
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

pub fn generate_completions(shell: clap_complete::Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "char", &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    #[cfg(feature = "standalone")]
    fn generate_docs_standalone() {
        let cmd = Cli::command();
        let json = cli_docs::generate_json(&cmd);

        let json_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cli-web/src/data/cli.json");
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
