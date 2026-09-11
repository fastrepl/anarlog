use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::stats::{self, Distribution};

/// Bump only with a PROTOCOL.md entry. Reports refuse to mix versions.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub schema_version: u32,
    pub harness: Harness,
    pub build: Build,
    pub environment: Environment,
    /// Free-form recorded conditions the harness cannot measure itself:
    /// power, display, thermal, locale, CloudSync state, etc.
    pub conditions: BTreeMap<String, String>,
    pub fixture: String,
    pub scenario: String,
    pub trial: u32,
    pub warmup: bool,
    pub started_at: String,
    pub ended_at: String,
    pub sample_interval_ms: u64,
    pub plateau_after_seconds: u64,
    pub command: Option<Vec<String>>,
    pub root_pid: u32,
    pub memory_metric: MemoryMetric,
    pub process_set: Vec<ProcessInfo>,
    pub samples: Vec<Sample>,
    pub summary: Summary,
    pub end_state: EndState,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Harness {
    pub name: String,
    pub version: String,
    pub sysinfo_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    /// `tauri`, `gpui`, or another label; the report groups by it.
    pub runtime: String,
    pub sha: Option<String>,
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub arch: String,
    pub host_name: Option<String>,
    pub cpu_brand: Option<String>,
    pub logical_cores: usize,
    pub physical_cores: Option<usize>,
    pub total_memory_bytes: u64,
}

/// Which per-process memory figure `Sample::memory_bytes` sums. RSS
/// double-counts shared pages across processes; PSS does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMetric {
    /// Linux `/proc/<pid>/smaps_rollup` Pss, summed over the process set.
    Pss,
    /// Resident set size as reported by the OS, summed over the process set.
    Rss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub cmd: Vec<String>,
    pub start_time_unix: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub matched_by: ProcessMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessMatch {
    Root,
    Descendant,
    /// Matched `--include-name` and started at or after launch; catches
    /// helpers that were reparented away from the app (WebKit, sidecars).
    NamePattern,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub t_ms: u64,
    pub process_count: usize,
    /// Sum over the process set; 100.0 is one logical core.
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub rss_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub thread_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub duration_ms: u64,
    pub sample_count: usize,
    pub cpu_percent: Option<Distribution>,
    /// Integral of `cpu_percent` over time, in core-seconds.
    pub cpu_time_seconds: f64,
    pub memory_bytes: Option<Distribution>,
    /// Median `memory_bytes` of samples after `plateau_after_seconds`.
    pub memory_plateau_bytes: Option<f64>,
    /// Least-squares slope of `memory_bytes` after `plateau_after_seconds`.
    pub memory_growth_bytes_per_min: Option<f64>,
    pub disk_read_bytes_total: u64,
    pub disk_write_bytes_total: u64,
    pub max_process_count: usize,
    pub max_thread_count: Option<usize>,
}

impl Summary {
    pub fn from_samples(samples: &[Sample], plateau_after_seconds: u64) -> Self {
        let t_seconds: Vec<f64> = samples.iter().map(|s| s.t_ms as f64 / 1000.0).collect();
        let cpu: Vec<f64> = samples.iter().map(|s| s.cpu_percent).collect();
        let memory: Vec<f64> = samples.iter().map(|s| s.memory_bytes as f64).collect();

        let plateau_start_ms = plateau_after_seconds * 1000;
        let plateau: Vec<&Sample> = samples
            .iter()
            .filter(|s| s.t_ms >= plateau_start_ms)
            .collect();
        let plateau_minutes: Vec<f64> = plateau.iter().map(|s| s.t_ms as f64 / 60_000.0).collect();
        let plateau_memory: Vec<f64> = plateau.iter().map(|s| s.memory_bytes as f64).collect();

        Self {
            duration_ms: samples.last().map(|s| s.t_ms).unwrap_or(0),
            sample_count: samples.len(),
            cpu_percent: Distribution::of(&cpu),
            cpu_time_seconds: stats::integrate(&t_seconds, &cpu) / 100.0,
            memory_bytes: Distribution::of(&memory),
            memory_plateau_bytes: (!plateau_memory.is_empty())
                .then(|| stats::median(&plateau_memory)),
            memory_growth_bytes_per_min: stats::slope(&plateau_minutes, &plateau_memory),
            disk_read_bytes_total: samples.iter().map(|s| s.disk_read_bytes).sum(),
            disk_write_bytes_total: samples.iter().map(|s| s.disk_write_bytes).sum(),
            max_process_count: samples.iter().map(|s| s.process_count).max().unwrap_or(0),
            max_thread_count: samples.iter().filter_map(|s| s.thread_count).max(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndState {
    pub root_exited: bool,
    pub exit_code: Option<i32>,
    pub killed_by_harness: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t_ms: u64, cpu: f64, memory: u64) -> Sample {
        Sample {
            t_ms,
            process_count: 2,
            cpu_percent: cpu,
            memory_bytes: memory,
            rss_bytes: memory,
            disk_read_bytes: 10,
            disk_write_bytes: 5,
            thread_count: Some(8),
        }
    }

    #[test]
    fn summary_integrates_cpu_and_fits_memory_growth_after_plateau() {
        let samples = vec![
            sample(0, 200.0, 100),
            sample(1000, 200.0, 900),
            sample(2000, 100.0, 1000),
            sample(62_000, 100.0, 1600),
            sample(122_000, 100.0, 2200),
        ];
        let summary = Summary::from_samples(&samples, 2);

        assert_eq!(summary.sample_count, 5);
        assert_eq!(summary.duration_ms, 122_000);
        // 2 cores for 1s, 1.5 cores for 1s, then 1 core for 120s.
        assert!((summary.cpu_time_seconds - 123.5).abs() < 1e-9);
        assert_eq!(summary.memory_plateau_bytes, Some(1600.0));
        assert!((summary.memory_growth_bytes_per_min.unwrap() - 600.0).abs() < 1e-6);
        assert_eq!(summary.disk_read_bytes_total, 50);
        assert_eq!(summary.max_process_count, 2);
        assert_eq!(summary.max_thread_count, Some(8));
    }

    #[test]
    fn summary_of_no_samples_is_empty_not_a_panic() {
        let summary = Summary::from_samples(&[], 60);
        assert_eq!(summary.sample_count, 0);
        assert!(summary.cpu_percent.is_none());
        assert!(summary.memory_plateau_bytes.is_none());
        assert!(summary.memory_growth_bytes_per_min.is_none());
    }

    #[test]
    fn artifact_round_trips_through_json() {
        let artifact = Artifact {
            schema_version: SCHEMA_VERSION,
            harness: Harness {
                name: "anarlog-bench".into(),
                version: "0.1.0".into(),
                sysinfo_version: "0.38".into(),
            },
            build: Build {
                runtime: "gpui".into(),
                sha: Some("abc".into()),
                channel: None,
            },
            environment: Environment {
                os_name: Some("Linux".into()),
                os_version: None,
                kernel_version: None,
                arch: "x86_64".into(),
                host_name: None,
                cpu_brand: None,
                logical_cores: 4,
                physical_cores: Some(2),
                total_memory_bytes: 1,
            },
            conditions: BTreeMap::from([("power".to_string(), "plugged".to_string())]),
            fixture: "ordinary".into(),
            scenario: "idle".into(),
            trial: 1,
            warmup: false,
            started_at: "2026-09-05T00:00:00Z".into(),
            ended_at: "2026-09-05T00:00:01Z".into(),
            sample_interval_ms: 1000,
            plateau_after_seconds: 0,
            command: Some(vec!["app".into()]),
            root_pid: 42,
            memory_metric: MemoryMetric::Pss,
            process_set: vec![],
            samples: vec![sample(0, 1.0, 1)],
            summary: Summary::from_samples(&[sample(0, 1.0, 1)], 0),
            end_state: EndState {
                root_exited: false,
                exit_code: None,
                killed_by_harness: true,
            },
            errors: vec![],
        };

        let json = serde_json::to_string(&artifact).unwrap();
        let parsed: Artifact = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.memory_metric, MemoryMetric::Pss);
        assert_eq!(parsed.samples, artifact.samples);
        assert!(json.contains("\"memory_metric\":\"pss\""));
    }
}
