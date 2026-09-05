# desktop-bench

Whole-application benchmark harness for comparing the Tauri and GPUI desktop
builds under [ANLG-320](https://linear.app/fastrepl-inc/issue/ANLG-320/migrate-the-desktop-application-from-tauri-to-gpui).
It launches an app, samples its entire process tree (CPU, memory, disk,
threads), writes one versioned JSON artifact per trial, and renders Markdown
comparisons from artifacts. The measurement rules live in
[PROTOCOL.md](./PROTOCOL.md).

## Usage

```sh
# One warm-up plus five measured 10-minute idle trials of a release Tauri build.
cargo run --release -p desktop-bench -- run \
  --runtime tauri --build-sha "$(git rev-parse --short HEAD)" --channel stable \
  --scenario idle-10m-sync-off --fixture ordinary \
  --duration 600 --warmup-trials 1 --trials 5 \
  --include-name WebKit --include-name anarlog \
  --meta power=plugged --meta display=2560x1440@60 --meta thermal=nominal \
  --meta locale=en-US --meta cloudsync=off --meta fixture_version=1 \
  --out bench-results \
  -- /Applications/Anarlog.app/Contents/MacOS/anarlog

# Same scenario against the GPUI build.
cargo run --release -p desktop-bench -- run \
  --runtime gpui --build-sha "$(git rev-parse --short HEAD)" \
  --scenario idle-10m-sync-off --fixture ordinary \
  --duration 600 --warmup-trials 1 --trials 5 --include-name anarlog \
  --meta power=plugged --meta display=2560x1440@60 --meta thermal=nominal \
  --meta locale=en-US --meta cloudsync=off --meta fixture_version=1 \
  --out bench-results \
  -- target/release/anarlog-gpui --db-path fixtures/ordinary/app.db

# Compare.
cargo run --release -p desktop-bench -- report --baseline tauri bench-results/*.json
```

`--pid <n>` attaches to a running process instead of launching one (single
trial, no kill at the end). `--interval-ms` defaults to 1000 and cannot go
below 200. `--plateau-after` (default 60 s) sets where the memory plateau and
growth window starts. `--show-app-output` lets the app write to the terminal.

Fixture data and scenario drivers are not part of this crate; the harness
records the `--scenario` and `--fixture` ids you pass and samples whatever the
launched command does. Scenario automation and in-app metrics (launch to
interactive, input latency, render counts) arrive with the Phase 2 test hooks.

## Layout

```
src/
  main.rs      CLI (run, report), artifact assembly, environment capture
  sampler.rs   process-set selection and the sampling loop (sysinfo)
  artifact.rs  schema v1 types and summary derivation
  report.rs    artifact loading and Markdown comparison
  stats.rs     median, percentile, cv, slope, integral
build.rs       records the pinned sysinfo version in artifacts
```
