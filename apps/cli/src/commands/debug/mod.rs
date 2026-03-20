mod transcribe;

use std::path::PathBuf;

use clap::{Subcommand, ValueEnum};

use crate::error::CliResult;

#[derive(Subcommand)]
pub enum Commands {
    /// Real-time transcription from audio devices
    Transcribe {
        #[command(flatten)]
        args: TranscribeArgs,
    },
}

#[derive(Clone, Default, ValueEnum)]
pub enum TranscribeMode {
    /// Print transcription line-by-line to stderr (default)
    #[default]
    Raw,
    /// TUI with speaker labels, word-level tracking, and segmenting
    Rich,
}

#[derive(clap::Args)]
pub struct TranscribeArgs {
    #[arg(long, value_enum)]
    pub provider: DebugProvider,
    /// Display mode
    #[arg(long, value_enum, default_value = "raw")]
    pub mode: TranscribeMode,
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

#[derive(clap::Args)]
pub struct AudioArgs {
    #[arg(long, value_enum, default_value = "input")]
    pub audio: AudioSource,
}

#[derive(Clone, ValueEnum)]
pub enum AudioSource {
    Input,
    Output,
    RawDual,
    AecDual,
    Mock,
}

pub async fn run(command: Commands) -> CliResult<()> {
    match command {
        Commands::Transcribe { args } => transcribe::run(args).await,
    }
}
