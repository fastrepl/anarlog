mod cli;
mod commands;
mod config;
mod error;
mod output;
mod stt;
#[cfg(feature = "standalone")]
pub(crate) mod tui;

use crate::cli::{Cli, Commands};
use crate::error::CliResult;
use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(base) = &cli.global.base {
        config::paths::set_base(base.clone());
    }

    if cli.global.no_color || std::env::var_os("NO_COLOR").is_some() {
        colored::control::set_override(false);
    }

    let trace_buffer = init_tracing(&cli);

    if let Err(error) = run(cli, trace_buffer).await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "standalone")]
type OptTraceBuffer = Option<tui::TraceBuffer>;
#[cfg(not(feature = "standalone"))]
type OptTraceBuffer = ();

fn init_tracing(cli: &Cli) -> OptTraceBuffer {
    let level = cli.verbose.tracing_level_filter();

    let wants_json = matches!(
        cli.command,
        Some(Commands::Transcribe {
            args: commands::transcribe::Args {
                format: cli::OutputFormat::Json,
                ..
            },
        })
    );

    #[cfg(feature = "standalone")]
    let wants_json = wants_json
        || matches!(
            cli.command,
            Some(Commands::Record {
                args: commands::record::Args {
                    format: cli::OutputFormat::Json,
                    ..
                },
            })
        );

    #[cfg(feature = "standalone")]
    let wants_capture = !wants_json
        && std::io::IsTerminal::is_terminal(&std::io::stderr())
        && matches!(
            cli.command,
            Some(Commands::Transcribe { .. } | Commands::Models { .. })
        );

    #[cfg(feature = "standalone")]
    if wants_capture {
        let buf = tui::new_trace_buffer();
        init_tracing_capture(level, buf.clone());
        return Some(buf);
    }

    if wants_json {
        init_tracing_json(level);
    } else {
        init_tracing_stderr(level);
    }

    #[cfg(feature = "standalone")]
    return None;
    #[cfg(not(feature = "standalone"))]
    return ();
}

fn init_tracing_stderr(level: tracing_subscriber::filter::LevelFilter) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn init_tracing_json(level: tracing_subscriber::filter::LevelFilter) {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[cfg(feature = "standalone")]
fn init_tracing_capture(level: tracing_subscriber::filter::LevelFilter, buffer: tui::TraceBuffer) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    let capture = tui::CaptureLayer::new(buffer);
    tracing_subscriber::registry()
        .with(filter)
        .with(capture)
        .init();
}

fn analytics_client() -> hypr_analytics::AnalyticsClient {
    let mut builder = hypr_analytics::AnalyticsClientBuilder::default();
    if std::env::var_os("DO_NOT_TRACK").is_none() {
        if let Some(key) = option_env!("POSTHOG_API_KEY") {
            builder = builder.with_posthog(key);
        }
    }
    builder.build()
}

fn track_command(client: &hypr_analytics::AnalyticsClient, subcommand: &'static str) {
    let client = client.clone();
    tokio::spawn(async move {
        let machine_id = hypr_host::fingerprint();
        let payload = hypr_analytics::AnalyticsPayload::builder("cli_command_invoked")
            .with("subcommand", subcommand)
            .with("app_identifier", "com.char.cli")
            .with("app_version", option_env!("APP_VERSION").unwrap_or("dev"))
            .build();
        let _ = client.event(machine_id, payload).await;
    });
}

#[cfg(feature = "desktop")]
pub(crate) async fn init_pool() -> CliResult<sqlx::SqlitePool> {
    let paths = config::paths::resolve_paths();

    let db = if cfg!(debug_assertions) {
        hypr_db_core2::Db3::connect_memory_plain()
            .await
            .map_err(|e| error::CliError::operation_failed("db connect", e.to_string()))?
    } else {
        let db_path = paths.base.join("app.db");
        hypr_db_core2::Db3::connect_local_plain(&db_path)
            .await
            .map_err(|e| error::CliError::operation_failed("db connect", e.to_string()))?
    };

    hypr_db_app::migrate(db.pool())
        .await
        .map_err(|e| error::CliError::operation_failed("db migrate", e.to_string()))?;
    config::settings::migrate_json_settings_to_db(db.pool(), &paths.base).await;
    Ok(db.pool().clone())
}

fn stt_overrides(
    global: &cli::GlobalArgs,
    provider: Option<stt::SttProvider>,
) -> stt::SttOverrides {
    stt::SttOverrides {
        provider,
        base_url: global.base_url.clone(),
        api_key: global.api_key.clone(),
        model: global.model.clone(),
        language: global.language.clone(),
    }
}

async fn run(cli: Cli, trace_buffer: OptTraceBuffer) -> CliResult<()> {
    let analytics = analytics_client();

    if let Some(ref command) = cli.command {
        let subcommand: &'static str = command.into();
        track_command(&analytics, subcommand);
    }

    let _quiet = cli.verbose.is_silent();
    let Cli {
        command,
        global,
        verbose: _,
    } = cli;

    match command {
        Some(Commands::Transcribe { args }) => {
            let overrides = stt_overrides(&global, Some(args.provider));
            commands::transcribe::run(args, overrides, trace_buffer).await
        }
        #[cfg(feature = "standalone")]
        Some(Commands::Models { command }) => commands::model::run(command, trace_buffer).await,
        #[cfg(feature = "standalone")]
        Some(Commands::Record { args }) => commands::record::run(args, quiet).await,
        Some(Commands::Completions { shell }) => {
            cli::generate_completions(shell);
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(Commands::Desktop) => {
            use commands::desktop::DesktopAction;
            match commands::desktop::run()? {
                DesktopAction::OpenedApp => eprintln!("Opened desktop app"),
                DesktopAction::OpenedDownloadPage => {
                    eprintln!("Desktop app not found — opened download page")
                }
            }
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(Commands::Bug) => {
            commands::bug::run()?;
            eprintln!("Opened bug report page in browser");
            Ok(())
        }
        #[cfg(feature = "standalone")]
        Some(Commands::Hello) => {
            commands::hello::run()?;
            eprintln!("Opened char.com in browser");
            Ok(())
        }

        #[cfg(feature = "task")]
        Some(Commands::Claude { command }) => commands::claude::run(command).await,
        #[cfg(feature = "task")]
        Some(Commands::Codex { command }) => commands::codex::run(command).await,
        #[cfg(feature = "task")]
        Some(Commands::Opencode { command }) => commands::opencode::run(command).await,
        #[cfg(feature = "desktop")]
        Some(Commands::Meetings { command }) => {
            let pool = init_pool().await?;
            commands::meetings::run(&pool, command).await
        }
        #[cfg(feature = "desktop")]
        Some(Commands::Humans { command }) => {
            let pool = init_pool().await?;
            commands::humans::run(&pool, command).await
        }
        #[cfg(feature = "desktop")]
        Some(Commands::Orgs { command }) => {
            let pool = init_pool().await?;
            commands::orgs::run(&pool, command).await
        }
        #[cfg(feature = "desktop")]
        Some(Commands::Export { command }) => {
            let pool = init_pool().await?;
            commands::export::run(&pool, command).await
        }
        None => {
            use clap::CommandFactory;
            Cli::command().print_help().ok();
            println!();
            Ok(())
        }
    }
}
