---
name: release-new-version
description: Release a new desktop stable version for Anarlog. Use when asked to cut, publish, or prepare a new stable desktop release after checking and merging the changelog.
metadata:
  internal: true
---

# Release a New Desktop Version

Use this for stable desktop releases. A stable release must come from `main`, after the changelog for the computed version is present, accurate, validated, and merged.

## Core Rule

Do not trigger a stable release from an unmerged branch. First make the changelog up to date, merge that changelog change to `main`, then run the stable release from `main`.

## QA Gate

After the changelog is merged to `main`, read and run
`../qa-critical-ux/SKILL.md` against that final commit. Require a recorded PASS
for both:

- native Dev QA pinned with `ANARLOG_QA_GIT_SHA` and a helper manifest with
  `git_dirty=false`
- the run-scoped staging artifact whose Actions head SHA exactly matches the
  Dev manifest's `git_head_sha`

The manifest's `git_head_sha` is the exact final `main` commit, including the
changelog, not a synthetic GitButler workspace HEAD. The report must include
that Dev SHA, staging run URL and head SHA, staging artifact SHA-256, and the
critical checklist results. Any failure, missing evidence, SHA mismatch,
rebuild, or later source change blocks stable unless the user explicitly
waives that specific gate.

This release path approves macOS, Windows, and Linux. Mobile remains closed.
The patched CloudSync vendor bundle is rebuilt from source and
cancellation-tested on every desktop lane: `rebuild-macos.sh` for Apple
Silicon and Intel, `rebuild-windows.sh` under UCRT64 in `windows_ci`, and
`rebuild-linux.sh` in `linux_ci` for x86_64 and aarch64. Each lane then runs
`cargo test -p cloudsync` and `cargo test -p db-core cloudsync::` against that
freshly built library, covering the stalled-network, logout, configuration
cleanup/init, worker-drain, and immediate-local-write cancellation gates.

The rebuild steps run only on `workflow_dispatch`, so a routine pull-request
run does not prove them. Dispatch `desktop_ci.yaml` against the candidate SHA
and confirm the `cloudsync-windows-*` and `cloudsync-linux-*` artifacts before
treating a desktop lane as approved. Do not treat macOS artifacts or
Rust-only tests as cross-platform approval, and do not open the mobile lane
until its bundle gets the same treatment.

## Preflight

1. Inspect the workflow before assuming release behavior:

```bash
sed -n '1,280p' .github/workflows/desktop_cd.yaml
```

2. Validate the explicit stable version requested by the user:

```bash
VERSION=<version>
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
test -f "packages/changelog/content/$VERSION.md"
```

Stable desktop releases never infer a version. The workflow requires the exact
stable semantic version and a matching changelog file.

3. Identify the latest stable desktop tag and the commits that will ship:

```bash
git fetch --tags --force
git tag -l 'desktop_v*' --sort=-v:refname | grep -E '^desktop_v[0-9]+\.[0-9]+\.[0-9]+$' | head -n1
git log --oneline <latest-desktop-tag>..HEAD
```

Use read-only `git` commands for inspection. If the workspace is on `gitbutler/workspace`, use the `but` skill for commits, pushes, PRs, merges, and other write operations.

## Changelog Gate

The changelog is the release gate. Before releasing:

1. Open `packages/changelog/content/AGENTS.md` and follow its instructions.
2. Confirm `packages/changelog/content/<version>.md` exists.
3. Compare the file against the desktop user-facing changes since the latest `desktop_v*` tag.
4. If the changelog is missing or incomplete, update it before release.

Changelog entries should be worth reading for app users. Exclude internal-only refactors, CI changes, infra noise, and implementation details unless they explain a user-visible change.

Each changelog file must include:

```md
---
date: "YYYY-MM-DD"
summary: "One concise, user-facing sentence for the changelog index preview."
---
```

After editing the changelog, run:

```bash
pnpm exec dprint fmt
pnpm -F @hypr/changelog typecheck
```

## Merge to Main

Only after the changelog is accurate and validation passes:

1. Commit the changelog change.
2. Open or update the changelog PR.
3. Wait for CI and required review state to be clear.
4. Merge the changelog PR to `main`.
5. Verify `main` contains `packages/changelog/content/<version>.md`.
6. Record the resulting `main` SHA and complete the QA Gate against that exact
   commit before triggering stable.

If using GitButler, prefer:

```bash
but diff
but commit chore/release-changelog -c -m "Update desktop release changelog

Refresh the desktop changelog for the next stable release." --changes <ids>
but pr new <branch-id> -t
```

Use actual IDs from `but diff` / `but status -fv`; do not invent IDs.

## Trigger Stable Release

After the changelog merge and QA Gate pass on the same `main` SHA, verify
`main` has not moved, then build the stable candidate without publishing:

```bash
gh workflow run desktop_cd.yaml \
  --ref main \
  -f channel=stable \
  -f publish=false \
  -f version=<version>
```

Watch the dry-run build:

```bash
gh run list --workflow desktop_cd.yaml --branch main --limit 5
gh run view <run-id> --json headSha,url
gh run watch <run-id>
```

The run's `headSha` must equal the Dev/staging candidate SHA. A mismatch blocks
acceptance even if the workflow succeeds.

Do not use GitHub's rerun button for a failed stable candidate or Linux audio
gate. Dispatch a fresh run instead; publication only accepts first-attempt run
IDs so evidence cannot be mixed across attempts.

The dry-run workflow must:

- use the exact explicit stable version
- build both Apple Silicon and Intel macOS artifacts
- build the signed Windows and Linux artifacts for the same version and commit
- upload a draft CrabNebula release without publishing it
- upload `desktop-release-provenance-<version>-<sha>`, including the exact
  artifact hashes and pinned CrabNebula CLI version, asset ID, and SHA-256

Run the Linux virtual-audio gate against that exact dry-run candidate:

```bash
gh workflow run desktop_linux_audio_qa.yaml \
  --ref main \
  -f version=<version> \
  -f candidate_sha=<40-character-main-sha> \
  -f dry_run_id=<dry-run-id>
```

Watch the run and verify its head SHA is still the candidate SHA. It must
produce passing `linux-audio-qa-<version>-x64` and
`linux-audio-qa-<version>-arm64` evidence artifacts. This gate proves virtual
microphone/system routing, capture, separation, and persistence with
`NO_AEC=1`; it does not waive physical-device or AEC validation.

After the exact dry-run artifacts and Linux audio QA run pass the required
platform gates and `main` still points to the candidate SHA, publish only
through the provenance workflow:

```bash
gh workflow run desktop_publish.yaml \
  --ref main \
  -f version=<version> \
  -f candidate_sha=<40-character-main-sha> \
  -f dry_run_id=<dry-run-id> \
  -f audio_qa_run_id=<linux-audio-qa-run-id>
```

Watch that workflow to completion. It must verify the dry-run run identity,
artifact hashes, both Linux audio evidence artifacts, CrabNebula tool
identity and hash, current `main`, and the immutable tag before publishing. It
must also verify every file mirrored to GitHub against the provenance manifest.

## Final Checks

Before reporting success, capture:

- computed stable version
- dry-run workflow URL and head SHA
- Linux audio QA workflow URL, head SHA, and x64/ARM64 evidence artifacts
- publish workflow URL and head SHA
- `desktop_v<version>` tag
- GitHub release URL
- whether CrabNebula publish completed
- changelog URL
- stable DMG SHA-256
- installed stable critical-QA PASS

If the workflow fails, inspect the failed job logs with:

```bash
gh run view <run-id> --log-failed
```

After the workflow succeeds, follow the QA skill's post-publish stable gate:
download the matching DMG from the GitHub release, install it, and use Computer
Use to repeat the core checks. Do not declare the release complete until both
the stable workflow and installed stable QA succeed.
