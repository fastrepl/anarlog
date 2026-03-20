use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};

use crate::llm::LlmProvider;

/// Live transcription and audio tools
#[derive(Parser)]
#[command(name = "char", version, propagate_version = true)]
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
    #[arg(long, global = true, env = "CHAR_BASE_URL", value_parser = parse_base_url)]
    pub base_url: Option<String>,

    #[arg(long, global = true, env = "CHAR_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(short = 'm', long, global = true, env = "CHAR_MODEL")]
    pub model: Option<String>,

    #[arg(
        short = 'l',
        long,
        global = true,
        env = "CHAR_LANGUAGE",
        default_value = "en"
    )]
    pub language: String,

    #[arg(long, global = true, env = "CHAR_RECORD")]
    pub record: bool,

    #[arg(long, global = true)]
    pub no_color: bool,

    #[arg(long, global = true, env = "CHAR_BASE", value_name = "DIR")]
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

#[derive(Subcommand, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum Commands {
    /// Interactive chat with an LLM
    Chat {
        #[command(subcommand)]
        command: Option<crate::commands::chat::Commands>,
        /// Send a single prompt without entering the TUI (use `-` to read from stdin)
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<LlmProvider>,
    },
    /// Configure an STT or LLM provider
    Connect {
        #[arg(long, value_enum)]
        r#type: Option<crate::commands::connect::ConnectionType>,

        #[arg(long, value_enum)]
        provider: Option<crate::commands::connect::ConnectProvider>,
    },
    /// Browse past meetings
    Meetings {
        #[command(subcommand)]
        command: Option<crate::commands::meetings::Commands>,
    },
    /// Browse humans (contacts)
    Humans {
        #[command(subcommand)]
        command: Option<crate::commands::humans::Commands>,
    },
    /// Browse organizations
    Orgs {
        #[command(subcommand)]
        command: Option<crate::commands::orgs::Commands>,
    },
    /// Configure providers and settings
    Configure {
        #[arg(long, value_enum)]
        tab: Option<crate::commands::configure::ConfigureTab>,
    },
    /// Authenticate with char.com
    Auth,
    /// Open the desktop app or download page
    Desktop,
    /// Report a bug on GitHub
    Bug,
    /// Open char.com
    Hello,
    /// Transcribe an audio file (no meeting created)
    Transcribe {
        #[command(flatten)]
        args: crate::commands::transcribe::Args,
    },
    /// Manage local models
    Models {
        #[command(subcommand)]
        command: crate::commands::model::Commands,
    },
    /// Debug and diagnostic tools
    #[cfg(feature = "dev")]
    Debug {
        #[command(subcommand)]
        command: crate::commands::debug::Commands,
    },
    /// Export data in various formats
    Export {
        #[command(subcommand)]
        command: crate::commands::export::Commands,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

// --- Shared types (used across multiple modules) ---

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Provider {
    Deepgram,
    Soniox,
    Assemblyai,
    Fireworks,
    Openai,
    Gladia,
    Elevenlabs,
    Mistral,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Text,
    Json,
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
    fn generate_docs() {
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
