# Desktop release procedures

Read this before dispatching desktop verification, builds, or publication. The
[release coordinator](../SKILL.md) owns scope, authorization, and completion.
Read [Release Docs](../../release-docs/SKILL.md) for the mandatory changelog
publication immediately after the app is published.

## Core Rule

Do not trigger a stable release from an unmerged branch. Complete the coordinator's release surface review and Release Docs' changelog gate, merge the required changes to `main`, then freeze the candidate and release that exact merged commit. New candidates use `main`; every verification, build, and publication run must execute at the recorded candidate SHA.

## Staging and Stable Operations

- Nightly publication is retired. Do not dispatch or recreate Nightly build,
  publication, or changelog workflows. Preserve historical immutable Nightly
  tags and existing installations' auth/share compatibility.
- Existing users and the main download remain on stable. Use signed staging
  artifacts from `desktop_cd.yaml` with `channel=staging` for candidate testing.
  Staging has a separate profile; do not reset or copy a user's live database
  merely to prepare a release.
- Target weekly ordinary stable releases. Pin the team to the exact candidate
  for 2–3 working days and record real meeting results. Disable automatic
  updates while testing. CI or elapsed time alone is not evidence of use.
  Confirm recording/transcription, saved notes after restart, sync, and
  stable-to-candidate upgrades on shipped platforms.
- One release owner records the candidate SHA, staging build/run, testing
  results, unresolved issues, and go/no-go decision in the release task. A
  serious regression postpones publication. Candidate fixes require renewed
  affected testing.
- Staging and stable are separate signed packages. Verify the final stable
  package's install/upgrade behavior before publication; do not present staging
  as byte-identical to stable.
- For an urgent stable hotfix, carry the minimal patch from the latest stable
  tag into main and verify it. Record the owner's explicit exception to the
  usual testing period; do not bundle unrelated work.
- Shared APIs and synced data must stay compatible with existing stable clients.
  Keep schema changes additive and downgrade-safe (see root `AGENTS.md`),
  including compatibility with existing Nightly installations that share the
  stable database.

New releases use `CANDIDATE_REF=main`. Keep its head at the recorded candidate
through verification, build, and publish dispatches. Check every run's `headSha`;
if main advances, do not combine old build evidence with the new workflow SHA.
Select and verify a fresh candidate before continuing. Historical immutable
Nightly refs remain supported for their existing candidates only.

## Release Workflow Requirements

The desktop path covers macOS, Windows, and Linux. Requested mobile distribution
follows the separate [mobile procedures](mobile.md).
The patched CloudSync vendor bundle is rebuilt from source and
cancellation-tested on every desktop lane: `rebuild-macos.sh` for Apple
Silicon and Intel, `rebuild-windows.sh` under UCRT64 in `windows_ci`, and
`rebuild-linux.sh` in `linux_ci` for x86_64 and aarch64. Each lane then runs
`cargo test -p cloudsync` and `cargo test -p db-core cloudsync::` against that
freshly built library, covering the stalled-network, logout, configuration
cleanup/init, worker-drain, and immediate-local-write cancellation gates.

The rebuild steps run on `workflow_dispatch` or a reusable caller with
`rebuild_cloudsync=true`, so a routine pull-request run does not prove them. Dispatch `desktop_ci.yaml` against the candidate SHA
and confirm the `cloudsync-windows-*` and `cloudsync-linux-*` artifacts before
treating a desktop lane as approved. Do not treat macOS artifacts or
Rust-only tests as cross-platform approval. Check the mobile coverage separately;
its current Android job does not provide the iOS cancellation-test coverage.

## Preflight

1. Inspect the workflow before assuming release behavior:

```bash
cat .github/workflows/desktop_cd.yaml
cat .github/workflows/desktop_ci.yaml
cat .github/workflows/desktop_publish.yaml
cat .github/workflows/desktop_store_publish.yaml
cat .github/workflows/cli_ci.yaml
cat .github/workflows/api_ci.yaml
cat .github/workflows/api_cd.yaml
cat .github/workflows/db_cd.yaml
cat .github/workflows/web_ci.yaml
cat .github/workflows/web_cd.yaml
```

2. Validate the explicit stable version requested by the user:

```bash
VERSION=<version>
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
node scripts/release-version.mjs "$VERSION"
node scripts/release-version.mjs --check "$VERSION"
test -f "packages/changelog/content/$VERSION.md"
```

Stable desktop releases never infer a version. The workflow requires the exact
stable semantic version to match `release-version.json` and a changelog file.
The version command also regenerates `apps/watch/apple/Version.xcconfig`;
commit both desktop version files with the release preparation changes. Expo
reads `apps/mobile/release-version.json`. A desktop bump does not change or
authorize mobile publication.

3. Identify the latest stable desktop tag and the commits that will ship:

```bash
gh release list --limit 20
gh api repos/fastrepl/anarlog/compare/<latest-desktop-tag>...main
git log --oneline <latest-desktop-tag>..<candidate-sha>
```

Verify the latest published, non-prerelease `desktop_v<semver>` tag through
GitHub. GitHub's compare file list can be truncated; use local history and diffs
for the complete changelog review. Use read-only `git` commands for inspection.
Use the `but` skill for local version control, and GitHub tools for PR metadata
and merges. Do not force-fetch tags or force-push to prepare a release.

## Merge to Main

Only after the changelog and required release surface updates are accurate and
validation passes:

1. Commit the changelog and surface updates in coherent commits.
2. Open or update their PRs.
3. Wait for CI and required review state to be clear.
4. Merge the release preparation PRs to `main`.
5. Verify `main` contains the changelog and all required surface updates.
6. Record the resulting `main` SHA as the candidate and use `CANDIDATE_REF=main`.
7. Complete staging candidate testing before building stable. Verify main still
   equals that SHA before every dispatch; an advanced head needs fresh candidate
   verification, not reuse of another commit's evidence.

If using GitButler, prefer:

```bash
but diff
but commit -b chore/release-changelog -m "Update desktop release changelog

Refresh the desktop changelog for the next stable release." <file-or-hunk-ids>
but pr new chore/release-changelog -t
```

Use actual IDs from `but diff` / `but status -fv`; do not invent IDs.

## Trigger Stable Release

Dispatch desktop, CLI, and API verification from the frozen main candidate, then identify each run
and verify `headSha` equals the recorded candidate before accepting any job.
Reuse an existing successful run only if it covers the exact SHA and all
required jobs; path-filtered or skipped jobs are not coverage:

```bash
CANDIDATE_REF=main
gh workflow run desktop_ci.yaml --ref "$CANDIDATE_REF"
gh workflow run cli_ci.yaml --ref "$CANDIDATE_REF"
gh workflow run api_ci.yaml --ref "$CANDIDATE_REF"
```

Verify every native job and the source-rebuilt CloudSync artifacts, including
both macOS architectures, Windows, and both Linux architectures. Pull-request
runs skip the desktop native jobs. Require both CLI jobs and the API job to pass
for this candidate as well. Keep the candidate fixed through publication.

After candidate testing, verify the candidate still equals main,
then build the stable candidate without publishing:

```bash
gh workflow run desktop_cd.yaml \
  --ref "$CANDIDATE_REF" \
  -f channel=stable \
  -f candidate_sha=<40-character-main-sha> \
  -f include_windows=true \
  -f include_linux=true \
  -f version=<version>
```

Watch the dry-run build:

```bash
gh run list --workflow desktop_cd.yaml --limit 5
gh run view <run-id> --json headSha,url
gh run watch <run-id>
```

The run's `headSha` must equal the recorded release-candidate SHA. A mismatch
blocks acceptance even if the workflow succeeds.

Do not use GitHub's rerun button for a failed stable candidate or optional
Linux audio QA run. Dispatch a fresh run instead; publication only accepts
first-attempt run IDs so evidence cannot be mixed across attempts.

The dry-run workflow must:

- use the exact explicit stable version
- build both Apple Silicon and Intel macOS artifacts
- build the signed Windows and Linux artifacts for the same version and commit
- upload a draft CrabNebula release without publishing it
- upload `desktop-release-provenance-<version>-<sha>`, including the exact
  artifact hashes and pinned CrabNebula CLI version, asset ID, and SHA-256

Complete the final packaged CLI and stdio MCP checks on every shipped platform
through [Release CLI](../../release-cli/SKILL.md) and
[Release MCP](../../release-mcp/SKILL.md). Reuse their artifact evidence; a staging
package or developer binary is not evidence for the final stable package.

Before the desktop publish dispatch, finish the [Release API](../../release-api/SKILL.md) deploys
that the candidate depends on. Do not publish a client whose required server
behavior is still unavailable.

After the exact dry-run artifacts pass the required platform gates and the
candidate is still merged into main, publish only through the provenance
workflow. Do not run `desktop_linux_audio_qa` as a publish gate; Linux is
covered by the same dry-run provenance as macOS and Windows. That workflow
remains available for optional debugging.

```bash
gh workflow run desktop_publish.yaml \
  --ref "$CANDIDATE_REF" \
  -f version=<version> \
  -f candidate_sha=<40-character-main-sha> \
  -f dry_run_id=<dry-run-id> \
  -f include_windows=true \
  -f include_linux=true
```

Watch that workflow to completion. It must verify the dry-run run identity,
artifact hashes, CrabNebula tool identity and hash, main ancestry, and the
immutable tag before publishing. It must also verify every file mirrored to
GitHub against the provenance manifest.

As soon as GitHub/CrabNebula publication succeeds, complete the publication
steps in [Release Docs](../../release-docs/SKILL.md). A successful desktop
publish job alone does not complete the release.

The publish workflow calls `desktop_store_publish.yaml` with
`submit_to_stores=true` for Microsoft Store certification. Inspect that job and
the resulting submission separately from GitHub/CrabNebula publication. For
Linux, the workflow waits for the generated package metadata PR's checks,
merges that exact PR head, calls `web_cd.yaml` with the merged commit, and
verifies the live signed APT metadata for both architectures on `anarlog.so`.
Require `linux-package-bump`, `linux-apt-deploy`, and `linux-apt-verify` to succeed;
a metadata PR or successful merge alone is not APT publication. Failed checks
leave the PR open and fail the release workflow for follow-up. Arch `PKGBUILD`
and `.SRCINFO` updates ship in this repository; there is no AUR publication
workflow. Check the AUR registry before claiming an AUR release.
