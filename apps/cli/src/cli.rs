use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_verbosity_flag::{InfoLevel, Verbosity};

use crate::llm::LlmProvider;

/// Live transcription and audio tools
#[derive(Parser)]
#[command(
    name = "char",
    version,
    propagate_version = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

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
        #[arg(long)]
        session: Option<String>,
        /// Send a single prompt without entering the TUI (use `-` to read from stdin)
        #[arg(long)]
        prompt: Option<String>,
        #[arg(long, value_enum)]
        provider: Option<LlmProvider>,
    },
    /// Start live transcription (TUI)
    Listen {
        #[arg(short = 'p', long, value_enum)]
        provider: Provider,

        #[arg(long, value_enum, default_value = "dual")]
        audio: AudioMode,
    },
    /// Configure an STT or LLM provider
    Connect {
        #[arg(long, value_enum)]
        r#type: Option<ConnectionType>,

        #[arg(long, value_enum)]
        provider: Option<ConnectProvider>,
    },
    /// Show configured providers and settings
    Status,
    /// Authenticate with char.com
    Auth,
    /// Open the desktop app or download page
    Desktop,
    /// Transcribe an audio file
    Batch {
        #[command(flatten)]
        args: BatchArgs,
    },
    /// Manage local models
    Model {
        #[command(subcommand)]
        command: ModelCommands,
    },
    /// Debug and diagnostic tools
    #[cfg(debug_assertions)]
    Debug {
        #[command(subcommand)]
        command: DebugCommands,
    },
    /// Generate shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

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

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ConnectionType {
    Stt,
    Llm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ConnectProvider {
    Deepgram,
    Soniox,
    Assemblyai,
    Openai,
    Gladia,
    Elevenlabs,
    Mistral,
    Fireworks,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus,
    Anthropic,
    Openrouter,
    GoogleGenerativeAi,
    AzureOpenai,
    AzureAi,
    Ollama,
    Lmstudio,
    Custom,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AudioMode {
    Dual,
    #[cfg(feature = "mock-audio")]
    Mock,
}

#[derive(clap::Args)]
pub struct BatchArgs {
    #[arg(long, value_name = "FILE", visible_alias = "file")]
    pub input: clio::InputPath,
    #[arg(short = 'p', long, value_enum)]
    pub provider: Provider,
    #[arg(long = "keyword", short = 'k', value_name = "KEYWORD")]
    pub keywords: Vec<String>,
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum ModelCommands {
    /// Show resolved paths for settings and model storage
    Paths,
    /// Show current STT and LLM provider/model configuration
    Current,
    /// List available models and their download status
    List {
        #[arg(long, value_enum)]
        kind: Option<ModelKind>,
        #[arg(long)]
        supported: bool,
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Manage downloadable Cactus models
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus {
        #[command(subcommand)]
        command: CactusCommands,
    },
    /// Download a model by name
    Download { name: String },
    /// Delete a downloaded model
    Delete { name: String },
}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
#[derive(Subcommand, Debug)]
pub enum CactusCommands {
    /// List available Cactus models
    List {
        #[arg(short = 'f', long, value_enum, default_value = "text")]
        format: OutputFormat,
    },
    /// Download a Cactus model by name
    Download { name: String },
    /// Delete a downloaded Cactus model
    Delete { name: String },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ModelKind {
    Stt,
    Llm,
}

#[cfg(debug_assertions)]
#[derive(Subcommand)]
pub enum DebugCommands {
    /// Real-time transcription from audio devices
    Transcribe {
        #[command(flatten)]
        args: TranscribeArgs,
    },
}

#[cfg(debug_assertions)]
#[derive(clap::Args)]
pub struct TranscribeArgs {
    #[arg(long, value_enum)]
    pub provider: DebugProvider,
    /// Model name (API model for cloud providers, model ID for local)
    #[arg(long, conflicts_with = "model_path")]
    pub model: Option<String>,
    /// Path to a local model directory on disk
    #[arg(long, conflicts_with = "model")]
    pub model_path: Option<PathBuf>,
    #[arg(long, env = "DEEPGRAM_API_KEY", hide_env_values = true)]
    pub deepgram_api_key: Option<String>,
    #[arg(long, env = "SONIOX_API_KEY", hide_env_values = true)]
    pub soniox_api_key: Option<String>,
    #[command(flatten)]
    pub audio: AudioArgs,
}

#[cfg(debug_assertions)]
#[derive(Clone, ValueEnum)]
pub enum DebugProvider {
    Deepgram,
    Soniox,
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    Cactus,
    ProxyHyprnote,
    ProxyDeepgram,
    ProxySoniox,
}

#[cfg(debug_assertions)]
#[derive(clap::Args)]
pub struct AudioArgs {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioSource,
}

#[cfg(debug_assertions)]
#[derive(Clone, ValueEnum)]
pub enum AudioSource {
    Input,
    Output,
    RawDual,
    AecDual,
    #[cfg(feature = "mock-audio")]
    Mock,
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
