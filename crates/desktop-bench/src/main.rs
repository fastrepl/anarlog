mod artifact;
mod report;
mod sampler;
mod stats;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use clap::{Args, Parser, Subcommand};
use sysinfo::System;

use crate::artifact::{Artifact, Build, Environment, Harness, SCHEMA_VERSION, Summary};
use crate::sampler::SamplerConfig;

/// Conditions PROTOCOL.md requires every comparable run to record.
const REQUIRED_CONDITIONS: &[&str] = &[
    "power",
    "display",
    "thermal",
    "locale",
    "cloudsync",
    "fixture_version",
];

#[derive(Parser)]
#[command(
    name = "anarlog-bench",
    about = "Whole-application benchmark harness for the Tauri vs GPUI desktop comparison (ANLG-320)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Launch (or attach to) the app, sample its whole process tree, and write
    /// one JSON artifact per trial.
    Run(Box<RunArgs>),
    /// Render a Markdown comparison from artifact files.
    Report(ReportArgs),
}

#[derive(Args)]
struct RunArgs {
    /// Runtime label the report groups by, e.g. `tauri` or `gpui`.
    #[arg(long)]
    runtime: String,
    /// Scenario id from the protocol, e.g. `idle-10m-sync-off`.
    #[arg(long)]
    scenario: String,
    /// Fixture id, e.g. `ordinary` or `power-user`.
    #[arg(long)]
    fixture: String,
    /// Build SHA of the app under test.
    #[arg(long)]
    build_sha: Option<String>,
    /// Release channel of the app under test (`stable`, `staging`, ...).
    #[arg(long)]
    channel: Option<String>,
    /// Sampling interval in milliseconds.
    #[arg(long, default_value_t = 1000)]
    interval_ms: u64,
    /// Stop after this many seconds. Defaults to sampling until the root exits.
    #[arg(long)]
    duration: Option<u64>,
    /// Samples before this offset are excluded from plateau and growth figures.
    #[arg(long, default_value_t = 60)]
    plateau_after: u64,
    /// Measured trials to run. Each trial relaunches the command.
    #[arg(long, default_value_t = 1)]
    trials: u32,
    /// Warm-up trials to run first; their artifacts are flagged and excluded
    /// from reports.
    #[arg(long, default_value_t = 0)]
    warmup_trials: u32,
    /// Case-insensitive process-name substrings to include even when the OS
    /// reparents them (e.g. `WebKit`, `anarlog`).
    #[arg(long = "include-name")]
    include_names: Vec<String>,
    /// Recorded run conditions as key=value (power, display, thermal, locale,
    /// cloudsync, ...). Repeatable.
    #[arg(long = "meta", value_parser = parse_key_value)]
    conditions: Vec<(String, String)>,
    /// Directory for artifacts.
    #[arg(long, default_value = "bench-results")]
    out: PathBuf,
    /// Attach to a running process instead of spawning one. Incompatible with
    /// `--trials` > 1 and with a command.
    #[arg(long, conflicts_with = "command")]
    pid: Option<u32>,
    /// Let the app under test write to this terminal.
    #[arg(long)]
    show_app_output: bool,
    /// Command to launch, after `--`.
    #[arg(last = true)]
    command: Vec<String>,
}

#[derive(Args)]
struct ReportArgs {
    /// Runtime whose medians the deltas are relative to.
    #[arg(long, default_value = "tauri")]
    baseline: String,
    /// Artifact JSON files.
    #[arg(required = true)]
    files: Vec<PathBuf>,
}

fn parse_key_value(raw: &str) -> Result<(String, String), String> {
    raw.split_once('=')
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .ok_or_else(|| format!("expected key=value, got {raw:?}"))
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Commands::Run(args) => run(*args),
        Commands::Report(args) => {
            let artifacts = report::load(&args.files)?;
            print!("{}", report::render_markdown(&artifacts, &args.baseline));
            Ok(())
        }
    }
}

fn run(args: RunArgs) -> anyhow::Result<()> {
    if args.pid.is_none() && args.command.is_empty() {
        anyhow::bail!("pass a command after `--` or attach with --pid");
    }
    if args.pid.is_some() && (args.trials != 1 || args.warmup_trials != 0) {
        anyhow::bail!("--pid cannot relaunch the app; use --trials 1 without warm-ups");
    }
    std::fs::create_dir_all(&args.out)
        .with_context(|| format!("failed to create {}", args.out.display()))?;

    let environment = environment();
    let conditions: BTreeMap<String, String> = args.conditions.iter().cloned().collect();
    let missing = missing_conditions(&conditions);
    if !missing.is_empty() {
        eprintln!(
            "[anarlog-bench] warning: not comparable under PROTOCOL.md, missing --meta {}",
            missing.join(", ")
        );
    }
    let config = SamplerConfig {
        interval: Duration::from_millis(args.interval_ms),
        duration: args.duration.map(Duration::from_secs),
        include_names: args.include_names.clone(),
        kill_at_end: args.pid.is_none(),
    };

    let total = args.warmup_trials + args.trials;
    for index in 0..total {
        let warmup = index < args.warmup_trials;
        let trial = if warmup {
            index + 1
        } else {
            index + 1 - args.warmup_trials
        };
        let label = if warmup { "warm-up" } else { "trial" };
        eprintln!(
            "[anarlog-bench] {label} {trial}/{}: {} {} on {}",
            if warmup {
                args.warmup_trials
            } else {
                args.trials
            },
            args.runtime,
            args.scenario,
            args.fixture
        );

        let started_at = chrono::Utc::now();
        let launch_time_unix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let (root_pid, child) = match args.pid {
            Some(pid) => (pid, None),
            None => {
                let mut command = Command::new(&args.command[0]);
                command.args(&args.command[1..]);
                if !args.show_app_output {
                    command.stdout(Stdio::null()).stderr(Stdio::null());
                }
                let child = command
                    .spawn()
                    .with_context(|| format!("failed to launch {:?}", args.command))?;
                (child.id(), Some(child))
            }
        };

        let outcome = sampler::run(root_pid, child, launch_time_unix, &config);
        let ended_at = chrono::Utc::now();
        let summary = Summary::from_samples(&outcome.samples, args.plateau_after);

        let artifact = Artifact {
            schema_version: SCHEMA_VERSION,
            harness: Harness {
                name: env!("CARGO_PKG_NAME").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                sysinfo_version: sysinfo_version(),
            },
            build: Build {
                runtime: args.runtime.clone(),
                sha: args.build_sha.clone(),
                channel: args.channel.clone(),
            },
            environment: environment.clone(),
            conditions: conditions.clone(),
            fixture: args.fixture.clone(),
            scenario: args.scenario.clone(),
            trial,
            warmup,
            started_at: started_at.to_rfc3339(),
            ended_at: ended_at.to_rfc3339(),
            sample_interval_ms: args.interval_ms,
            plateau_after_seconds: args.plateau_after,
            command: (!args.command.is_empty()).then(|| args.command.clone()),
            root_pid,
            memory_metric: outcome.memory_metric,
            process_set: outcome.process_set,
            samples: outcome.samples,
            summary,
            end_state: outcome.end_state,
            errors: outcome.errors,
        };

        let file_name = format!(
            "{}_{}_{}_{}{}_{}.json",
            sanitize(&args.runtime),
            sanitize(&args.scenario),
            sanitize(&args.fixture),
            if warmup { "warmup" } else { "trial" },
            trial,
            started_at.format("%Y%m%dT%H%M%SZ")
        );
        let path = args.out.join(file_name);
        std::fs::write(&path, serde_json::to_vec_pretty(&artifact)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
        eprintln!(
            "[anarlog-bench] wrote {} ({} samples, {} processes, cpu median {}, memory median {})",
            path.display(),
            artifact.summary.sample_count,
            artifact.process_set.len(),
            artifact
                .summary
                .cpu_percent
                .as_ref()
                .map(|d| format!("{:.1}%", d.median))
                .unwrap_or_else(|| "n/a".into()),
            artifact
                .summary
                .memory_bytes
                .as_ref()
                .map(|d| format!("{:.1} MiB", d.median / 1024.0 / 1024.0))
                .unwrap_or_else(|| "n/a".into()),
        );
        if artifact.end_state.root_exited && artifact.summary.sample_count == 0 {
            anyhow::bail!("the app exited before the first sample; check the command");
        }
    }
    Ok(())
}

fn environment() -> Environment {
    let mut system = System::new();
    system.refresh_cpu_list(sysinfo::CpuRefreshKind::nothing());
    system.refresh_memory();
    Environment {
        os_name: System::name(),
        os_version: System::os_version(),
        kernel_version: System::kernel_version(),
        arch: System::cpu_arch(),
        host_name: System::host_name(),
        cpu_brand: system.cpus().first().map(|cpu| cpu.brand().to_string()),
        logical_cores: system.cpus().len(),
        physical_cores: System::physical_core_count(),
        total_memory_bytes: system.total_memory(),
    }
}

fn sysinfo_version() -> String {
    env!("SYSINFO_VERSION").to_string()
}

fn missing_conditions(conditions: &BTreeMap<String, String>) -> Vec<&'static str> {
    REQUIRED_CONDITIONS
        .iter()
        .copied()
        .filter(|key| !conditions.contains_key(*key))
        .collect()
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_value_parsing_trims_and_rejects_missing_keys() {
        assert_eq!(
            parse_key_value(" power = plugged ").unwrap(),
            ("power".to_string(), "plugged".to_string())
        );
        assert!(parse_key_value("=x").is_err());
        assert!(parse_key_value("novalue").is_err());
    }

    #[test]
    fn missing_conditions_lists_only_absent_required_keys() {
        let conditions = BTreeMap::from([
            ("power".to_string(), "plugged".to_string()),
            ("display".to_string(), "x".to_string()),
            ("extra".to_string(), "y".to_string()),
        ]);
        assert_eq!(
            missing_conditions(&conditions),
            vec!["thermal", "locale", "cloudsync", "fixture_version"]
        );
    }

    #[test]
    fn file_name_components_are_filesystem_safe() {
        assert_eq!(sanitize("idle 10m/sync:off"), "idle_10m_sync_off");
        assert_eq!(sanitize("power-user"), "power-user");
    }

    #[test]
    fn cli_requires_a_command_or_pid() {
        let cli = Cli::try_parse_from([
            "anarlog-bench",
            "run",
            "--runtime",
            "gpui",
            "--scenario",
            "idle",
            "--fixture",
            "ordinary",
            "--meta",
            "power=plugged",
            "--",
            "/bin/sleep",
            "5",
        ])
        .unwrap();
        let Commands::Run(args) = cli.command else {
            panic!("expected run");
        };
        assert_eq!(args.command, vec!["/bin/sleep", "5"]);
        assert_eq!(
            args.conditions,
            vec![("power".to_string(), "plugged".to_string())]
        );

        assert!(
            Cli::try_parse_from([
                "anarlog-bench",
                "run",
                "--runtime",
                "gpui",
                "--scenario",
                "idle",
                "--fixture",
                "ordinary",
                "--pid",
                "1",
                "--",
                "app",
            ])
            .is_err()
        );
    }
}
