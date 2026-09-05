# Benchmark protocol (ANLG-320, schema v1)

This is the operational version of the protocol in
[ANLG-320](https://linear.app/fastrepl-inc/issue/ANLG-320/migrate-the-desktop-application-from-tauri-to-gpui).
The issue owns the scenarios, fixtures, metrics list, gates, and caveats; this
file owns what the harness actually records and how numbers are derived, so a
result produced today can be compared with one produced after the GPUI port.
Changing anything below requires bumping `SCHEMA_VERSION` in
`src/artifact.rs` and adding an entry to the history at the bottom.

## Trial procedure

1. Build both apps in release mode from recorded commit SHAs with devtools and
   debug-only behaviour disabled. Debug builds are invalid for comparison.
2. Use a fixed, named machine, plugged in, same display resolution and refresh
   rate, power mode, OS version, locale, app settings, and fixture version.
   Record them with `--meta` (see below). Quiesce other workloads.
3. Use isolated fixture data only; never point a run at a production profile.
4. Run one warm-up plus five measured trials per (runtime, scenario, fixture):
   `--warmup-trials 1 --trials 5`. Randomise Tauri/GPUI order between
   scenarios where practical.
5. Keep every artifact. Reports are generated from artifacts only; numbers are
   never hand-copied.

## What one artifact is

One artifact is one trial of one scenario on one fixture with one runtime. It
is written as pretty-printed JSON to
`<out>/<runtime>_<scenario>_<fixture>_<trial|warmup><n>_<utc timestamp>.json`.

### Identity and context

| Field                           | Meaning                                                                                  |
| ------------------------------- | ---------------------------------------------------------------------------------------- |
| `schema_version`                | `1`. Reports refuse to mix versions.                                                     |
| `harness`                       | Harness crate name and version, and the `sysinfo` version read from `Cargo.lock`.        |
| `build.runtime`                 | Label the report groups by: `tauri`, `gpui`, or anything else being compared.            |
| `build.sha`, `build.channel`    | From `--build-sha` and `--channel`. Always pass the SHA.                                  |
| `environment`                   | OS name/version/kernel, arch, host name, CPU brand, logical/physical cores, total memory. |
| `conditions`                    | Free-form `--meta key=value` pairs. Required keys: `power`, `display`, `thermal`, `locale`, `cloudsync` (`off`, `caught-up`, or `catch-up`), `fixture_version`. |
| `fixture`, `scenario`, `trial`, `warmup` | Scenario/fixture ids from ANLG-320; trial numbers restart at 1 for measured trials. Warm-ups are flagged and excluded from reports. |
| `started_at`, `ended_at`        | UTC RFC 3339.                                                                            |
| `command`, `root_pid`           | What was launched (or attached to).                                                      |

### Process set

The harness samples the whole application, not one PID. A process belongs to
the set when it is the root, a transitive child of the root (by parent PID at
sample time), or when its name contains an `--include-name` pattern
(case-insensitive) and it started no earlier than one second before launch.
The last rule exists because WebKit content/network processes and sidecars can
be reparented away from the app. The harness never includes itself.

Every process ever seen in the set is listed in `process_set` with pid, parent,
name, command line, start time, first/last seen offsets, and `matched_by`
(`root`, `descendant`, `name_pattern`). Validate this list before trusting a
run: a Tauri run on macOS must show the WebKit helpers, and no run should show
unrelated processes.

Recommended patterns: Tauri on macOS `--include-name WebKit --include-name anarlog`;
GPUI `--include-name anarlog`. Threads are never counted as processes.

### Samples

`samples[]` are raw, one per `--interval-ms` (default 1000 ms, floor 200 ms),
summed over the process set:

| Field              | Definition                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------ |
| `t_ms`             | Milliseconds since launch (or attach).                                                           |
| `process_count`    | Live processes in the set.                                                                       |
| `cpu_percent`      | Sum of per-process CPU since the previous sample; **100.0 = one logical core**. A process contributes 0 in the first sample after it appears (delta semantics). |
| `memory_bytes`     | Sum of the platform memory metric below.                                                         |
| `rss_bytes`        | Sum of resident set sizes, kept for cross-checking. It double counts shared pages.                |
| `disk_read_bytes`, `disk_write_bytes` | Bytes since the previous sample.                                                     |
| `thread_count`     | Linux only: `/proc/<pid>/task` entries summed over the set. `null` elsewhere.                    |

`memory_metric` names what `memory_bytes` sums:

| Platform | `memory_metric` | Definition                                                                                  |
| -------- | --------------- | ------------------------------------------------------------------------------------------- |
| Linux    | `pss`           | `Pss` from `/proc/<pid>/smaps_rollup`, so shared pages are split between processes and the sum over the set is meaningful. Falls back to RSS per process if unreadable. |
| macOS    | `rss`           | Resident size from the kernel. Physical footprint (`phys_footprint`, what Activity Monitor calls Memory) is **not** yet recorded; treat macOS memory as an upper bound until it is. |
| Windows  | `rss`           | Working set.                                                                                |

GPU memory, GPU time, energy, wakeups, context switches, and network bytes are
not sampled in v1. Report them as missing, not zero.

### Summary

Derived from `samples[]` by the harness and re-derivable from them:

| Field                          | Definition                                                                              |
| ------------------------------ | --------------------------------------------------------------------------------------- |
| `cpu_percent`                  | Distribution (count, mean, median, nearest-rank p95, min, max, coefficient of variation) of `cpu_percent`. |
| `cpu_time_seconds`             | Trapezoidal integral of `cpu_percent` over time divided by 100: core-seconds consumed.  |
| `memory_bytes`                 | Distribution of `memory_bytes`.                                                         |
| `memory_plateau_bytes`         | Median of `memory_bytes` for samples at or after `--plateau-after` seconds (default 60). |
| `memory_growth_bytes_per_min`  | Least-squares slope of `memory_bytes` over minutes for the same window. The soak gate reads this. |
| `disk_read_bytes_total`, `disk_write_bytes_total` | Sums.                                                                  |
| `max_process_count`, `max_thread_count` | Maxima.                                                                        |

`end_state` records whether the root exited on its own, its exit code, and
whether the harness killed the process set at `--duration`. `errors` lists
sampling problems. A trial whose root exited before the scenario ended is not
a valid measurement unless the scenario is an exit scenario.

## Reports

`anarlog-bench report --baseline tauri <artifacts...>` groups measured trials
by (scenario, fixture) and runtime, takes the median across trials for each
summary metric, prints the coefficient of variation across trials next to it,
and prints the delta of medians against the baseline. Every listed metric is
lower-is-better. The report also prints the build SHAs and memory metrics per
runtime so a mixed or debug comparison is visible on the page.

## Not covered by the harness (needs app-side hooks)

Launch-to-interactive, input-to-paint latency, FPS/long-frame rate,
React render counts vs GPUI invalidation counts, IPC/live-query counts, SQLite
query and WAL statistics, CloudSync throughput, and functional correctness per
scenario require minimal measurement hooks inside both apps (ANLG-320 Phase 2
"deterministic test hooks"). When they land, they should write into the same
artifact under a new top-level `app_metrics` object and bump the schema.

## Schema history

- **v1** — process-tree CPU/memory/disk/thread sampling, summaries, conditions
  metadata, Markdown report.
