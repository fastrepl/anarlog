# Release audit

Heavy, cross-platform verification is deliberately **not** run on every PR. It is
batched into a short audit performed just before cutting a release. This keeps
per-PR feedback fast (so the Bugbot → fix → push loop stays cheap) while the real
"moment of truth" happens once, on purpose.

## CI model

- **Per PR:** lint, format, typecheck, and unit/integration tests. Desktop CI
  runs JS and i18n checks on PRs and pushes to `main`, with no native desktop jobs.
  Rapid pushes cancel superseded runs of the same event and ref.
- **Desktop native checks:** macOS, Windows, Linux x86_64/ARM64, and Swift run
  daily at 09:00 UTC (18:00 KST) and through `workflow_dispatch` for release
  candidates. Pushes to `main` do not cancel scheduled or manual verification.
- **Mobile native builds:** iOS, watchOS, and Android follow `mobile_ci`'s
  separate schedule and release requirements.
- **Release (this audit):** full builds + signing + real-hardware QA.

## Audit checklist

Run these before publishing a stable desktop release.

1. **Read the cumulative diff since the last version.**
   `git diff <last-stable-tag>..main -- apps/desktop/src-tauri plugins crates apps/desktop/src`
   (see the `diff` task in `Taskfile.yaml`). Polish from first principles:
   simplify, delete dead code, reconcile inconsistencies introduced across PRs.

2. **Confirm the heavy suites are green** on the release candidate:
   - `desktop_ci` — trigger via `workflow_dispatch` on the candidate. CloudSync
     source rebuilds and platform artifacts require dispatch; a nightly run
     does not replace this release check.
   - `mobile_ci` — follow its separate native-build and release requirements.
   - `pro_api_e2e` — nightly/dispatch (live provider APIs).

3. **Build + sign all platforms** via `desktop_cd` (`staging` first, then
   `stable`). This produces the signed macOS/Windows/Linux artifacts.

4. **Real-hardware QA** (cannot run in CI/Cloud Agent):
   - Critical Pro user journey on a Mac — follow `.agents/skills/qa-critical-ux`.
     Linux and Windows ship from the same dry-run provenance; do not run a
     separate Linux-only audio QA gate before publish.

5. **Changelog** — add the entry via `.agents/skills/new-changelog`.

6. **Cut the release** — follow `.agents/skills/release-new-version`.

## Notes

- Anything that fails incidentally but is out of scope for the release gate is
  tracked in Linear, not patched into the candidate (see `qa-critical-ux`).
- macOS/iOS/watchOS/Windows verification requires real Apple/Windows machines;
  the Linux Cloud Agent covers authoring + Linux-native checks only.
