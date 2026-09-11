use std::collections::{BTreeMap, HashMap, HashSet};
use std::process::Child;
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use crate::artifact::{EndState, MemoryMetric, ProcessInfo, ProcessMatch, Sample};

/// Processes whose start time is within this many seconds before launch still
/// count as launched by the app; `start_time` has one-second resolution.
const START_TIME_SLACK_SECONDS: u64 = 1;

pub struct SamplerConfig {
    pub interval: Duration,
    /// `None` samples until the root process exits.
    pub duration: Option<Duration>,
    pub include_names: Vec<String>,
    /// Kill the whole process set when the duration elapses. Only sensible
    /// when the harness spawned the process.
    pub kill_at_end: bool,
}

pub struct Outcome {
    pub samples: Vec<Sample>,
    pub process_set: Vec<ProcessInfo>,
    pub end_state: EndState,
    pub errors: Vec<String>,
    pub memory_metric: MemoryMetric,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub start_time_unix: u64,
}

/// Decides which live processes belong to the app under test: the root, its
/// transitive descendants, and any process matching `include_names` that
/// started at or after launch. The last rule catches helpers that the OS
/// reparents (WebKit content processes, sidecars). `exclude` is the harness
/// itself, whose name and start time would otherwise match a broad pattern.
pub fn select_process_set(
    root: u32,
    exclude: u32,
    candidates: &[Candidate],
    include_names: &[String],
    launch_time_unix: u64,
) -> BTreeMap<u32, ProcessMatch> {
    let mut selected = BTreeMap::new();
    selected.insert(root, ProcessMatch::Root);

    let mut children_of: HashMap<u32, Vec<u32>> = HashMap::new();
    for candidate in candidates {
        if let Some(parent) = candidate.parent {
            children_of.entry(parent).or_default().push(candidate.pid);
        }
    }
    let mut stack = vec![root];
    let mut visited = HashSet::from([root]);
    while let Some(pid) = stack.pop() {
        for &child in children_of.get(&pid).into_iter().flatten() {
            if visited.insert(child) {
                selected.insert(child, ProcessMatch::Descendant);
                stack.push(child);
            }
        }
    }

    let patterns: Vec<String> = include_names.iter().map(|p| p.to_lowercase()).collect();
    if patterns.is_empty() {
        return selected;
    }
    let earliest = launch_time_unix.saturating_sub(START_TIME_SLACK_SECONDS);
    for candidate in candidates {
        if candidate.pid == exclude
            || selected.contains_key(&candidate.pid)
            || candidate.start_time_unix < earliest
        {
            continue;
        }
        let name = candidate.name.to_lowercase();
        if patterns.iter().any(|pattern| name.contains(pattern)) {
            selected.insert(candidate.pid, ProcessMatch::NamePattern);
        }
    }
    selected
}

fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_tasks()
        .with_cmd(UpdateKind::OnlyIfNotSet)
}

pub fn run(
    root_pid: u32,
    mut child: Option<Child>,
    launch_time_unix: u64,
    config: &SamplerConfig,
) -> Outcome {
    let mut system = System::new();
    let mut samples = Vec::new();
    let mut registry: BTreeMap<u32, ProcessInfo> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut end_state = EndState {
        root_exited: false,
        exit_code: None,
        killed_by_harness: false,
    };
    let memory_metric = if cfg!(target_os = "linux") {
        MemoryMetric::Pss
    } else {
        MemoryMetric::Rss
    };

    let interval = config.interval.max(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    let started = Instant::now();
    // CPU usage is a delta between refreshes; prime it before the first sample.
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());

    loop {
        std::thread::sleep(interval);
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind());
        let t_ms = started.elapsed().as_millis() as u64;

        let candidates: Vec<Candidate> = system
            .processes()
            .values()
            .filter(|process| process.thread_kind().is_none())
            .map(|process| Candidate {
                pid: process.pid().as_u32(),
                parent: process.parent().map(|pid| pid.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                start_time_unix: process.start_time(),
            })
            .collect();
        let selected = select_process_set(
            root_pid,
            std::process::id(),
            &candidates,
            &config.include_names,
            launch_time_unix,
        );

        let mut sample = Sample {
            t_ms,
            process_count: 0,
            cpu_percent: 0.0,
            memory_bytes: 0,
            rss_bytes: 0,
            disk_read_bytes: 0,
            disk_write_bytes: 0,
            thread_count: None,
        };
        let mut root_alive = false;
        for (&pid, &matched_by) in &selected {
            let Some(process) = system.process(Pid::from_u32(pid)) else {
                continue;
            };
            if pid == root_pid {
                root_alive = true;
            }
            sample.process_count += 1;
            sample.cpu_percent += f64::from(process.cpu_usage());
            sample.rss_bytes += process.memory();
            sample.memory_bytes += match memory_metric {
                MemoryMetric::Pss => pss_bytes(pid).unwrap_or_else(|| process.memory()),
                MemoryMetric::Rss => process.memory(),
            };
            let disk = process.disk_usage();
            sample.disk_read_bytes += disk.read_bytes;
            sample.disk_write_bytes += disk.written_bytes;
            if let Some(tasks) = process.tasks() {
                // `tasks` lists the non-main threads; the process itself is the main thread.
                *sample.thread_count.get_or_insert(0) += tasks.len() + 1;
            }

            registry
                .entry(pid)
                .and_modify(|info| info.last_seen_ms = t_ms)
                .or_insert_with(|| ProcessInfo {
                    pid,
                    parent_pid: process.parent().map(|p| p.as_u32()),
                    name: process.name().to_string_lossy().into_owned(),
                    cmd: process
                        .cmd()
                        .iter()
                        .map(|arg| arg.to_string_lossy().into_owned())
                        .collect(),
                    start_time_unix: process.start_time(),
                    first_seen_ms: t_ms,
                    last_seen_ms: t_ms,
                    matched_by,
                });
        }
        samples.push(sample);

        if let Some(child) = child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    end_state.root_exited = true;
                    end_state.exit_code = status.code();
                    break;
                }
                Ok(None) => {}
                Err(error) => errors.push(format!("failed to poll child process: {error}")),
            }
        } else if !root_alive {
            end_state.root_exited = true;
            break;
        }

        if let Some(duration) = config.duration
            && started.elapsed() >= duration
        {
            if config.kill_at_end {
                // Helpers first so the root cannot respawn them on the way out.
                for pid in selected.keys().filter(|pid| **pid != root_pid) {
                    if let Some(process) = system.process(Pid::from_u32(*pid)) {
                        process.kill();
                    }
                }
                if let Some(child) = child.as_mut() {
                    let _ = child.kill();
                    let _ = child.wait();
                } else if let Some(process) = system.process(Pid::from_u32(root_pid)) {
                    process.kill();
                }
                end_state.killed_by_harness = true;
            }
            break;
        }
    }

    Outcome {
        samples,
        process_set: registry.into_values().collect(),
        end_state,
        errors,
        memory_metric,
    }
}

/// Proportional set size from `/proc/<pid>/smaps_rollup`, which shares pages
/// fairly between processes so a sum over the tree does not double count.
#[cfg(target_os = "linux")]
fn pss_bytes(pid: u32) -> Option<u64> {
    let rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok()?;
    parse_pss_kib(&rollup).map(|kib| kib * 1024)
}

#[cfg(not(target_os = "linux"))]
fn pss_bytes(_pid: u32) -> Option<u64> {
    None
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_pss_kib(smaps_rollup: &str) -> Option<u64> {
    smaps_rollup
        .lines()
        .find_map(|line| line.strip_prefix("Pss:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(pid: u32, parent: Option<u32>, name: &str, start: u64) -> Candidate {
        Candidate {
            pid,
            parent,
            name: name.to_string(),
            start_time_unix: start,
        }
    }

    #[test]
    fn process_set_includes_transitive_descendants() {
        let candidates = [
            candidate(1, None, "init", 0),
            candidate(100, Some(1), "app", 1000),
            candidate(101, Some(100), "helper", 1000),
            candidate(102, Some(101), "grandchild", 1001),
            candidate(200, Some(1), "unrelated", 1000),
        ];
        let set = select_process_set(100, 1, &candidates, &[], 1000);
        assert_eq!(
            set,
            BTreeMap::from([
                (100, ProcessMatch::Root),
                (101, ProcessMatch::Descendant),
                (102, ProcessMatch::Descendant),
            ])
        );
    }

    #[test]
    fn name_pattern_catches_reparented_helpers_started_after_launch() {
        let candidates = [
            candidate(1, None, "launchd", 0),
            candidate(100, Some(1), "Anarlog", 1000),
            candidate(300, Some(1), "com.apple.WebKit.WebContent", 1000),
            candidate(301, Some(1), "com.apple.WebKit.Networking", 999),
            candidate(302, Some(1), "com.apple.WebKit.WebContent", 500),
            candidate(303, Some(1), "Safari", 1000),
        ];
        let set = select_process_set(100, 1, &candidates, &["webkit".to_string()], 1000);
        assert_eq!(set.get(&300), Some(&ProcessMatch::NamePattern));
        assert_eq!(set.get(&301), Some(&ProcessMatch::NamePattern));
        assert_eq!(set.get(&302), None, "started long before launch");
        assert_eq!(set.get(&303), None);
    }

    #[test]
    fn descendants_win_over_name_matches_and_cycles_terminate() {
        let candidates = [
            candidate(100, Some(101), "app", 1000),
            candidate(101, Some(100), "WebKit", 1000),
        ];
        let set = select_process_set(100, 1, &candidates, &["webkit".to_string()], 1000);
        assert_eq!(set.get(&101), Some(&ProcessMatch::Descendant));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn harness_never_selects_itself_even_when_its_name_matches() {
        let candidates = [
            candidate(50, Some(1), "anarlog-bench", 1000),
            candidate(100, Some(50), "anarlog-gpui", 1000),
        ];
        let set = select_process_set(100, 50, &candidates, &["anarlog".to_string()], 1000);
        assert_eq!(set, BTreeMap::from([(100, ProcessMatch::Root)]));
    }

    #[test]
    fn parses_pss_from_smaps_rollup() {
        let rollup = "00400000-7fff Rss:  1234 kB\nRss:                1234 kB\nPss:                 567 kB\nPss_Anon:            100 kB\n";
        assert_eq!(parse_pss_kib(rollup), Some(567));
        assert_eq!(parse_pss_kib("Rss: 1 kB\n"), None);
    }
}
