use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context as _;

use crate::artifact::{Artifact, SCHEMA_VERSION};
use crate::stats;

const MIB: f64 = 1024.0 * 1024.0;

type Extract = fn(&Artifact) -> Option<f64>;

/// Metrics compared across runtimes. Lower is better for every one of them.
const METRICS: &[(&str, Extract)] = &[
    ("CPU median (% of one core)", |a| {
        a.summary.cpu_percent.as_ref().map(|d| d.median)
    }),
    ("CPU p95 (%)", |a| {
        a.summary.cpu_percent.as_ref().map(|d| d.p95)
    }),
    ("CPU peak (%)", |a| {
        a.summary.cpu_percent.as_ref().map(|d| d.max)
    }),
    ("CPU time (core-seconds)", |a| {
        Some(a.summary.cpu_time_seconds)
    }),
    ("Memory median (MiB)", |a| {
        a.summary.memory_bytes.as_ref().map(|d| d.median / MIB)
    }),
    ("Memory peak (MiB)", |a| {
        a.summary.memory_bytes.as_ref().map(|d| d.max / MIB)
    }),
    ("Memory plateau (MiB)", |a| {
        a.summary.memory_plateau_bytes.map(|b| b / MIB)
    }),
    ("Memory growth (MiB/min)", |a| {
        a.summary.memory_growth_bytes_per_min.map(|b| b / MIB)
    }),
    ("Disk read (MiB)", |a| {
        Some(a.summary.disk_read_bytes_total as f64 / MIB)
    }),
    ("Disk write (MiB)", |a| {
        Some(a.summary.disk_write_bytes_total as f64 / MIB)
    }),
    ("Max process count", |a| {
        Some(a.summary.max_process_count as f64)
    }),
];

pub fn load(paths: &[impl AsRef<Path>]) -> anyhow::Result<Vec<Artifact>> {
    let mut artifacts = Vec::with_capacity(paths.len());
    for path in paths {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let artifact: Artifact = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if artifact.schema_version != SCHEMA_VERSION {
            anyhow::bail!(
                "{} has schema_version {} but this harness reports version {}",
                path.display(),
                artifact.schema_version,
                SCHEMA_VERSION
            );
        }
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

/// Markdown comparison grouped by scenario and fixture. Warm-up trials are
/// excluded. Each cell is the median across measured trials with the
/// coefficient of variation across those trials; deltas compare medians
/// against `baseline`.
pub fn render_markdown(artifacts: &[Artifact], baseline: &str) -> String {
    let mut out = String::new();
    let measured: Vec<&Artifact> = artifacts.iter().filter(|a| !a.warmup).collect();
    let warmups = artifacts.len() - measured.len();

    writeln!(out, "# Benchmark comparison").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "{} artifacts, {} measured trials, {} warm-up trials excluded. Schema v{}. Cells are `median (cv)` across trials; Δ compares medians against `{}`. Lower is better for every metric.",
        artifacts.len(),
        measured.len(),
        warmups,
        SCHEMA_VERSION,
        baseline
    )
    .unwrap();

    let mut groups: BTreeMap<(String, String), BTreeMap<String, Vec<&Artifact>>> = BTreeMap::new();
    for artifact in &measured {
        groups
            .entry((artifact.scenario.clone(), artifact.fixture.clone()))
            .or_default()
            .entry(artifact.build.runtime.clone())
            .or_default()
            .push(artifact);
    }

    for ((scenario, fixture), by_runtime) in &groups {
        writeln!(out).unwrap();
        writeln!(out, "## Scenario `{scenario}`, fixture `{fixture}`").unwrap();
        writeln!(out).unwrap();

        let memory_metrics: Vec<String> = by_runtime
            .iter()
            .map(|(runtime, runs)| {
                let metrics: std::collections::BTreeSet<String> = runs
                    .iter()
                    .map(|a| format!("{:?}", a.memory_metric).to_lowercase())
                    .collect();
                format!(
                    "{runtime}: {}",
                    metrics.into_iter().collect::<Vec<_>>().join("/")
                )
            })
            .collect();
        writeln!(
            out,
            "Memory metric by runtime: {}.",
            memory_metrics.join(", ")
        )
        .unwrap();
        let shas: Vec<String> = by_runtime
            .iter()
            .map(|(runtime, runs)| {
                let shas: std::collections::BTreeSet<&str> =
                    runs.iter().filter_map(|a| a.build.sha.as_deref()).collect();
                format!(
                    "{runtime}: {}",
                    if shas.is_empty() {
                        "unknown".to_string()
                    } else {
                        shas.into_iter().collect::<Vec<_>>().join(", ")
                    }
                )
            })
            .collect();
        writeln!(out, "Build SHA by runtime: {}.", shas.join("; ")).unwrap();
        writeln!(out).unwrap();

        let runtimes: Vec<&String> = by_runtime.keys().collect();
        write!(out, "| Metric |").unwrap();
        for runtime in &runtimes {
            write!(out, " {runtime} (n={}) |", by_runtime[*runtime].len()).unwrap();
        }
        for runtime in runtimes.iter().filter(|r| r.as_str() != baseline) {
            write!(out, " Δ {runtime} vs {baseline} |").unwrap();
        }
        writeln!(out).unwrap();
        write!(out, "| --- |").unwrap();
        for _ in &runtimes {
            write!(out, " ---: |").unwrap();
        }
        for _ in runtimes.iter().filter(|r| r.as_str() != baseline) {
            write!(out, " ---: |").unwrap();
        }
        writeln!(out).unwrap();

        for (label, extract) in METRICS {
            write!(out, "| {label} |").unwrap();
            let medians: BTreeMap<&String, Option<f64>> = runtimes
                .iter()
                .map(|runtime| {
                    let values: Vec<f64> = by_runtime[*runtime]
                        .iter()
                        .filter_map(|artifact| extract(artifact))
                        .collect();
                    let cell = aggregate(&values);
                    write!(out, " {} |", format_cell(cell)).unwrap();
                    (*runtime, cell.map(|(median, _)| median))
                })
                .collect();
            let base = medians.get(&baseline.to_string()).copied().flatten();
            for runtime in runtimes.iter().filter(|r| r.as_str() != baseline) {
                let delta = match (base, medians[*runtime]) {
                    (Some(b), Some(v)) if b != 0.0 => Some((v - b) / b * 100.0),
                    _ => None,
                };
                write!(out, " {} |", format_delta(delta)).unwrap();
            }
            writeln!(out).unwrap();
        }
    }
    out
}

fn aggregate(values: &[f64]) -> Option<(f64, Option<f64>)> {
    if values.is_empty() {
        return None;
    }
    let median = stats::median(values);
    let mean = stats::mean(values);
    let cv = (mean != 0.0 && values.len() > 1).then(|| stats::std_dev(values) / mean);
    Some((median, cv))
}

fn format_cell(cell: Option<(f64, Option<f64>)>) -> String {
    match cell {
        None => "—".to_string(),
        Some((median, None)) => format_number(median),
        Some((median, Some(cv))) => format!("{} (cv {:.0}%)", format_number(median), cv * 100.0),
    }
}

fn format_number(value: f64) -> String {
    if value.abs() >= 100.0 {
        format!("{value:.0}")
    } else if value.abs() >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

fn format_delta(delta: Option<f64>) -> String {
    match delta {
        None => "—".to_string(),
        Some(d) => format!("{d:+.1}%"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::artifact::*;

    fn artifact(runtime: &str, trial: u32, warmup: bool, cpu: f64, memory: f64) -> Artifact {
        let sample = Sample {
            t_ms: 0,
            process_count: 1,
            cpu_percent: cpu,
            memory_bytes: memory as u64,
            rss_bytes: memory as u64,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            thread_count: None,
        };
        Artifact {
            schema_version: SCHEMA_VERSION,
            harness: Harness {
                name: "anarlog-bench".into(),
                version: "test".into(),
                sysinfo_version: "test".into(),
            },
            build: Build {
                runtime: runtime.into(),
                sha: Some(format!("{runtime}-sha")),
                channel: None,
            },
            environment: Environment {
                os_name: None,
                os_version: None,
                kernel_version: None,
                arch: "x86_64".into(),
                host_name: None,
                cpu_brand: None,
                logical_cores: 1,
                physical_cores: None,
                total_memory_bytes: 0,
            },
            conditions: BTreeMap::new(),
            fixture: "ordinary".into(),
            scenario: "idle".into(),
            trial,
            warmup,
            started_at: String::new(),
            ended_at: String::new(),
            sample_interval_ms: 1000,
            plateau_after_seconds: 0,
            command: None,
            root_pid: 1,
            memory_metric: MemoryMetric::Rss,
            process_set: vec![],
            samples: vec![sample.clone()],
            summary: Summary::from_samples(&[sample], 0),
            end_state: EndState {
                root_exited: false,
                exit_code: None,
                killed_by_harness: true,
            },
            errors: vec![],
        }
    }

    #[test]
    fn report_uses_trial_medians_and_excludes_warmups() {
        let artifacts = vec![
            artifact("tauri", 0, true, 900.0, 1.0),
            artifact("tauri", 1, false, 100.0, 200.0 * MIB),
            artifact("tauri", 2, false, 120.0, 200.0 * MIB),
            artifact("tauri", 3, false, 80.0, 200.0 * MIB),
            artifact("gpui", 1, false, 50.0, 100.0 * MIB),
            artifact("gpui", 2, false, 50.0, 100.0 * MIB),
        ];
        let markdown = render_markdown(&artifacts, "tauri");

        assert!(markdown.contains("5 measured trials, 1 warm-up trials excluded"));
        assert!(markdown.contains("| gpui (n=2) |"));
        assert!(markdown.contains("| tauri (n=3) |"));
        let cpu_row = markdown
            .lines()
            .find(|line| line.starts_with("| CPU median"))
            .unwrap();
        assert!(cpu_row.contains("| 50.0 (cv 0%) |"), "{cpu_row}");
        assert!(cpu_row.contains("| 100 (cv 16%) |"), "{cpu_row}");
        assert!(cpu_row.ends_with("| -50.0% |"), "{cpu_row}");
        let memory_row = markdown
            .lines()
            .find(|line| line.starts_with("| Memory median"))
            .unwrap();
        assert!(
            memory_row.contains("| 100 (cv 0%) |") && memory_row.contains("| 200 (cv 0%) |"),
            "{memory_row}"
        );
        assert!(markdown.contains("Build SHA by runtime: gpui: gpui-sha; tauri: tauri-sha."));
    }

    #[test]
    fn load_rejects_other_schema_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.json");
        let mut artifact = artifact("tauri", 1, false, 1.0, 1.0);
        artifact.schema_version = SCHEMA_VERSION + 1;
        std::fs::write(&path, serde_json::to_string(&artifact).unwrap()).unwrap();

        let error = load(&[&path]).err().unwrap().to_string();
        assert!(error.contains("schema_version"), "{error}");

        artifact.schema_version = SCHEMA_VERSION;
        std::fs::write(&path, serde_json::to_string(&artifact).unwrap()).unwrap();
        assert_eq!(load(&[&path]).unwrap().len(), 1);
    }
}
