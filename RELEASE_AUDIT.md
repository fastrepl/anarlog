# Release audit

Heavy, cross-platform verification is deliberately **not** run on every PR. It is
batched into a short audit performed just before cutting a release. This keeps
per-PR feedback fast (so the Bugbot → fix → push loop stays cheap) while the real
"moment of truth" happens once, on purpose.

## CI model

- **Per PR (fast lane, Linux only):** lint, format, typecheck, unit/integration
  tests, and Linux `cargo check`/`cargo test`. Deduplicated via
  `concurrency: cancel-in-progress`, so rapid pushes cancel superseded runs.
- **On merge to `main` + nightly (`schedule`):** the full desktop matrix
  (macOS, Windows, Linux arm64, Swift) and the mobile native builds
  (iOS, watchOS, Android). Nightly catches platform breakage within a day and
  attributes it to a small window — keeping the release audit a clean diff review
  rather than a regression hunt.
- **Release (this audit):** full builds + signing + real-hardware QA.

## Audit checklist

Run these before publishing a stable desktop release.

1. **Read the cumulative diff since the last version.**
   `git diff <last-stable-tag>..main -- apps/desktop/src-tauri plugins crates apps/desktop/src`
   (see the `diff` task in `Taskfile.yaml`). Polish from first principles:
   simplify, delete dead code, reconcile inconsistencies introduced across PRs.

2. **Confirm the heavy suites are green** on the release candidate:
   - `desktop_ci` and `mobile_ci` — trigger via `workflow_dispatch` on the
     candidate (or confirm the latest nightly on `main` passed).
   - `pro_api_e2e` — nightly/dispatch (live provider APIs).

3. **Build + sign all platforms** via `desktop_cd` (`staging` first, then
   `stable`). This produces the signed macOS/Windows/Linux artifacts.

4. **Real-hardware QA** (cannot run in CI/Cloud Agent):
   - Critical Pro user journey on a Mac — follow `.agents/skills/qa-critical-ux`.
   - Linux system-audio capture — `desktop_linux_audio_qa` against the candidate.

5. **Changelog** — add the entry via `.agents/skills/new-changelog`.

6. **Cut the release** — follow `.agents/skills/release-new-version`.

## Notes

- Anything that fails incidentally but is out of scope for the release gate is
  tracked in Linear, not patched into the candidate (see `qa-critical-ux`).
- macOS/iOS/watchOS/Windows verification requires real Apple/Windows machines;
  the Linux Cloud Agent covers authoring + Linux-native checks only.
