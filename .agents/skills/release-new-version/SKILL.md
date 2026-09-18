---
name: release-new-version
description: Prepare BlackMushi Nightly builds and promote tested desktop stable versions with current CLI, local and hosted MCP, API, agent packages, and documentation. Deploy any required hosted services during the release. Validate and merge release updates before publishing. Distribute mobile builds when requested.
metadata:
  internal: true
---

# Release a New Version

Use this for Nightly builds, stable desktop releases, and requested mobile store distribution. A stable desktop release must come from `main`, after the changelog and required CLI, MCP, API, agent-package, and documentation updates are accurate, validated, and merged. Desktop and watchOS share the marketing version in `release-version.json`. iOS and Android use `apps/mobile/release-version.json`. Platform build numbers and publication schedules remain independent.

## Core Rule

Do not trigger a stable release from an unmerged branch. Complete the release surface review and changelog below, merge the required changes to `main`, then freeze the candidate and release that merged commit through its Nightly tag.

## Nightly and Stable Operations

- Existing users and the main download remain on stable. Nightly is an explicit
  separate-app install, with its own updater feed and CLI command. It opens the
  same local database as stable; settings, store, and sign-in stay per app.
- The team uses Nightly for daily meetings. Volunteers can join through the
  announcement in the next stable changelog and product-update newsletter.
- `.github/workflows/desktop_nightly.yaml` runs daily at 15:00 UTC (midnight KST)
  and can be dispatched manually from `main`. It runs desktop JS/i18n and native
  CI, including source CloudSync rebuilds, before building and publishing all
  desktop platforms through `desktop_cd.yaml` with `channel=nightly`.
- Nightly versions are `<shared-version>-nightly.<n>`, where `<n>` counts up
  from 1 for each base version (`1.4.24-nightly.1`, `1.4.24-nightly.2`, ...)
  and resets when `release-version.json` moves to the next stable. Each build
  snapshots `packages/changelog/nightly.md` into the app and a GitHub prerelease
  tagged `desktop_nightly_v<nightly-version>`. Maintain that file as curated,
  user-facing changes since the previous stable release; do not generate a raw
  commit dump. Nightly notes never belong in the website's stable changelog.
- Target weekly ordinary stable releases. Select a published Nightly commit,
  pin the team's app to it for 2–3 working days, and record real meeting results.
  Disable automatic updates while testing that candidate. CI or elapsed time
  alone is not evidence of use. Confirm recording/transcription, saved notes
  after restart, sync, and stable-to-candidate upgrades on shipped platforms.
- One release owner records the candidate, Nightly release/run, testing results,
  unresolved issues, and go/no-go decision in the release task. A serious
  regression postpones publication. Candidate fixes require renewed affected
  testing; newer main features wait for the next candidate.
- Nightly and stable are separate signed packages. Build stable from the tested
  commit and verify its install/upgrade behavior before publication; do not
  present the Nightly binary as byte-identical to the stable artifact.
- For an urgent stable hotfix, start from the latest stable tag, carry the
  minimal fix into main, and verify the patch. Record the owner's explicit
  exception to the usual Nightly testing period; do not bundle unrelated work.
  The candidate must still be merged into main before publication. If main has
  advanced, dispatch `desktop_cd.yaml` with `channel=nightly` on the merged
  hotfix branch, supplying its exact SHA, then use the resulting Nightly tag
  for stable verification and publication. This preserves the minimal patch.
- Shared APIs and synced data must stay compatible with existing stable clients.
  Nightly and stable write the same local database, and the apps refuse to run
  at the same time. A `-- breaking` migration published in Nightly locks stable
  users out of their notes until stable ships it: keep schema changes additive
  (see the root `AGENTS.md`), and land a breaking migration only in the
  candidate that becomes the next stable release, so the lockout ends when that
  release publishes.

### Publish and verify Nightly

```bash
gh workflow run desktop_nightly.yaml --ref main
gh run list --workflow desktop_nightly.yaml --limit 5
```

Verify the exact SHA and all called jobs, not only the aggregate status. A failed
run needs a fresh dispatch. Confirm the GitHub prerelease/tag, signed installers,
CrabNebula `nightly` downloads, and every platform's Nightly update response.
Install the published build and exercise Nightly-to-Nightly updating, auth,
sharing links, and the embedded CLI. Confirm stable remains on the stable feed.
The first published Nightly needs this verification before announcing it.

Do not send the newsletter or announce Nightly as available until the Nightly
builds, update feed, and `https://anarlog.so/download/nightly/` are live and verified.
Use the [newsletter skill](../product-update-newsletter/SKILL.md) for the announcement.
Nightly publication does not publish a website changelog or submit to stores.

## Scope Boundary

Establish the exact desktop version and requested mobile destination before
dispatching. A desktop release includes its existing Microsoft Store workflow;
it does not imply mobile submission. When mobile is requested, distinguish
TestFlight and Google Play internal testing from public App Store review and
Google Play production rollout. Honor authorization already given in the task;
do not ask again for an approved destination.

App Store here means the iOS app. The repository deliberately has no Mac App
Store release lane; do not recreate one as part of a desktop or mobile release.

Every desktop release includes the CLI, local and hosted MCP, API, agent-package,
and documentation freshness review below, and deploys any hosted service that
has unpublished changes this release needs. The CLI and local MCP ship inside the
desktop package; hosted services, plugin catalogs, and docs have separate
publication paths. Keep their versions independent and record their source SHAs.
An unchanged surface needs evidence that its published version still covers the
candidate; it does not need an artificial version bump or redeployment.

Honor existing authorization for service and documentation publication. If a
required external action is not authorized, finish preparing and validating the
concrete change before asking for that action. When a hosted service has
unpublished changes this release needs, dispatch its CD workflow during the
release; do not leave the deploy as a follow-up. Do not call the complete
release finished while a required surface is stale or awaiting publication.

Release and QA are separate, explicitly requested workflows. Do not read or
run `qa-critical-ux` or `qa-cli-mcp-api` solely because the user asked for a
release. A release does not require a report from either optional QA skill. The Nightly
candidate testing and final stable package verification above are part of this
release operation; report their actual evidence separately.
The contract, packaging, and publication checks in this skill are required
release verification; they do not invoke either optional QA workflow.

If the user explicitly asks for both release and QA, follow the requested
order and report the outcomes separately. Do not infer that a QA result
approves or blocks the release.

## Release Workflow Requirements

The desktop path covers macOS, Windows, and Linux. Requested mobile distribution
follows the separate native-build and store steps below.
The patched CloudSync vendor bundle is rebuilt from source and
cancellation-tested on every desktop lane: `rebuild-macos.sh` for Apple
Silicon and Intel, `rebuild-windows.sh` under UCRT64 in `windows_ci`, and
`rebuild-linux.sh` in `linux_ci` for x86_64 and aarch64. Each lane then runs
`cargo test -p cloudsync` and `cargo test -p db-core cloudsync::` against that
freshly built library, covering the stalled-network, logout, configuration
cleanup/init, worker-drain, and immediate-local-write cancellation gates.

The rebuild steps run on `workflow_dispatch` or the Nightly caller with
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
cat .github/workflows/stripe_cd.yaml
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

## Release Surface Review

Complete this before freezing the candidate, including when only preparing a
release. Review the full product diff since the last stable desktop tag, not
just CLI/API paths. Also compare each independently published surface with its
last published source SHA so previously unshipped changes are not missed.

For each user-facing change, record the affected surfaces, required updates,
validation, and publication status in the release task or PR. Use `unchanged`
or `not applicable` only with a concrete reason. Check that supported agent
workflows expose the new or changed product behavior. Fix drift before release;
an intentional capability difference must be documented. A missing capability
that needs a product decision requires an explicit deferral, not a silent skip.

| Surface                     | Review against the candidate                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI and shared agent access | `apps/cli`, `crates/agent-access`, and CLI contract snapshots: commands, flags, JSON fields/errors, pagination, exports, local/cloud selection, and supported data formats. Trace changed product behavior through these implementations; rebuilding alone does not establish coverage.                                                                                                           |
| Local and hosted MCP        | `apps/cli/src/mcp.rs`, `crates/api-cloud/src/mcp.rs`, and `crates/mcp`: tool/resource schemas, output, protocol compatibility, and authentication. Derive expectations from current source and snapshots, never a hardcoded historical tool count. Preserve documented local proposal approval and hosted read-only boundaries.                                                                   |
| API and generated client    | `apps/api`, `crates/api-cloud`, and affected auth/sync crates; `apps/api/openapi.gen.json` and `packages/api-client/src/generated`. Check routes, payloads, errors, auth scopes, and compatibility with already shipped desktop/mobile/CLI clients.                                                                                                                                               |
| Agent skills and plugins    | Authored `skills/anarlog`, generated `agent-plugins/anarlog` and `docs/skill.md`, native manifests, and repository marketplace entries. Update instructions and examples, bump the plugin's own version when its package changes, and keep manifests and their tests aligned.                                                                                                                     |
| Documentation and discovery | Read `docs/AGENTS.md`; review affected product guides, installation/upgrade instructions, CLI/MCP/Cloud references, examples, troubleshooting, screenshots, `docs/docs.json`, and `apps/web/public/llms.txt`. Include the public skill, Mintlify's `llms.txt`/`llms-full.txt`, and the website changelog in publication verification. Document shipped behavior and actual platform availability. |
| Hosted services             | Compare each independently deployed service with its last published SHA/tag: API/hosted MCP (`api_cd.yaml`, `api_v*`), Stripe (`stripe_cd.yaml`, `stripe_v*`), hosted Postgres (`db_cd.yaml`), website (`web_cd.yaml`, `web_v*`), and Mintlify docs. Record whether the candidate needs a redeploy. |

Review related release dependencies when affected: SQLite/CloudSync and hosted
schema migrations, downgrade compatibility, native bindings, mobile/watch
consumers, billing/auth configuration, installer/updater and package channels,
and release version attribution in error reporting. Record backend-first rollout
and rollback requirements before publishing a client that depends on them.
Follow each component's instructions; this review does not authorize mobile
submission or unrelated infrastructure changes.

### Validate and synchronize

Run the full locally reproducible jobs from `cli_ci.yaml` and `api_ci.yaml` for
affected code, plus consumer checks from root `AGENTS.md`. CLI CI includes
command/MCP contract snapshots, documentation coverage, Linux/Windows smoke
tests, plugin checks, and Mintlify validation. API CI checks hosted MCP/auth and
OpenAPI generation. A passing snapshot or docs check does not prove that a new
product feature was considered; retain the surface review above.

When the API contract changes, regenerate the spec and client, review the diff,
and typecheck the client and affected consumers:

```bash
cargo test -p api gen_openapi_json
pnpm -F @anlg/api-client openapi
pnpm -F @anlg/api-client typecheck
```

Edit the authored agent skill and references, then generate their mirrors with
`node scripts/publish-anarlog-skill.mjs`. The script writes repository files; it
does not publish a website or install a plugin. Before every release, run:

```bash
node scripts/publish-anarlog-skill.mjs --check
node --test scripts/publish-anarlog-skill.test.mjs
```

Use the Mintlify version pinned in `cli_ci.yaml` to run `validate` and
`broken-links --check-anchors --check-redirects` from `docs/`. Run affected web
checks when changing website content or discovery files. Regenerate again to
confirm generated output is stable, and include all intended generated changes
before freezing the candidate.

## Changelog Gate

The changelog is required alongside the release surface review. Before releasing either channel:

Prepare, validate, and merge stable notes before freezing the desktop candidate,
but do not expose them on the website before the desktop release is published.
Deployable web builds verify published, non-draft, non-prerelease GitHub
`desktop_v<version>` releases and exclude every other version from both pages
and bundles. Before publication, check the deployed production site: its index
must omit the candidate and its direct changelog URL must return 404. A local
production-mode build can verify the behavior before deployment; `vite dev`
intentionally previews drafts and is not publication evidence. After GitHub/CrabNebula publication,
verify the website deployment includes the released notes (normally the Linux
APT web deploy); dispatch `web_cd.yaml` if no post-publication deploy covers it.
Do not change the frozen desktop candidate merely to publish its website notes.

1. Open `packages/changelog/content/AGENTS.md` and follow its instructions.
2. For stable, confirm `packages/changelog/content/<version>.md` exists. For Nightly, use `packages/changelog/nightly.md`.
3. Compare the file against the desktop user-facing changes since the latest `desktop_v*` tag.
4. If the changelog is missing or incomplete, update it before release.

Changelog entries should be worth reading for app users. Exclude internal-only refactors, CI changes, infra noise, and implementation details unless they explain a user-visible change. If a user-facing change came from a pull request by someone outside the Fastrepl org, that item must credit them. See `packages/changelog/content/AGENTS.md`.

Each changelog file must include:

```md
---
date: "YYYY-MM-DD"
summary: "One concise, user-facing sentence for the changelog index preview."
---
```

After editing the changelog, run:

```bash
pnpm exec dprint fmt --allow-no-files packages/changelog/content/<version>.md
pnpm exec dprint check --allow-no-files packages/changelog/content/<version>.md
pnpm -F @anlg/changelog typecheck
```

## Merge to Main

Only after the changelog and required release surface updates are accurate and
validation passes:

1. Commit the changelog and surface updates in coherent commits.
2. Open or update their PRs.
3. Wait for CI and required review state to be clear.
4. Merge the release preparation PRs to `main`.
5. Verify `main` contains the changelog and all required surface updates.
6. Record the resulting `main` SHA, publish a Nightly from it, and retain its
   immutable `desktop_nightly_v<nightly-version>` tag as `CANDIDATE_REF`.
7. Complete the Nightly candidate testing period before building stable. Keep
   development on main; do not replace the candidate with its latest head.

If using GitButler, prefer:

```bash
but diff
but commit -b chore/release-changelog -m "Update desktop release changelog

Refresh the desktop changelog for the next stable release." <file-or-hunk-ids>
but pr new chore/release-changelog -t
```

Use actual IDs from `but diff` / `but status -fv`; do not invent IDs.

## Deploy Hosted Services

This is a required release action, not optional follow-up. After the candidate
is on `main`, decide for each independently published service and then deploy
the ones that need it before publishing the desktop client.

For each service:

1. Find the last successful CD run and, when one exists, the published tag
   (`api_v*`, `stripe_v*`, `web_v*`). Hosted Postgres has no version tag; use
   the last successful `db_cd.yaml` run SHA.
2. Diff that SHA against the candidate for that service's source. Re-read the
   workflow if the checkout, image context, or migration path is unclear.
3. Dispatch the CD workflow on `main` when the candidate contains unpublished
   changes this release needs, or that users would otherwise miss. Reuse a live
   deployment only when its SHA already includes those changes.
4. An unchanged service needs that live SHA/version recorded. Do not dispatch a
   no-op redeploy to make the checklist look complete.
5. Confirm the target is BlackMushi before any service access; never access
   `*-char`. Check the run's `headSha` against the intended SHA and wait for
   the job and tag (when the workflow creates one).

```bash
gh workflow run api_cd.yaml --ref main
gh workflow run stripe_cd.yaml --ref main
gh workflow run db_cd.yaml --ref main
gh workflow run web_cd.yaml --ref main
gh run list --workflow api_cd.yaml --limit 3
gh run view <run-id> --json headSha,url
```

| Service | Workflow | Deploy when | Live check |
| --- | --- | --- | --- |
| API and hosted MCP | `api_cd.yaml` | Unpublished API, hosted MCP, auth, or related proxy changes | `/health` reports the new `api_v*` version; MCP discovery and an authenticated read succeed when credentials exist |
| Stripe billing | `stripe_cd.yaml` | Unpublished `apps/stripe` or image-context changes | CD succeeded and tagged `stripe_v*`; `/health` on the BlackMushi Stripe app returns ok |
| Hosted Postgres | `db_cd.yaml` | Unpublished `supabase/` migrations this release needs | Linked BlackMushi project only; `supabase db push` completed. Run `db_ci.yaml` coverage first when migrations changed |
| Website | `web_cd.yaml` | Unpublished website, changelog, or download-page changes not already covered by the Linux APT web deploy | Live `anarlog.so` URLs show the candidate content |
| Docs | Mintlify connected deploy (no GitHub CD) | Unpublished `docs/` or public skill content after merge | Live `https://docs.anarlog.so` pages, `skill.md`, and LLM indexes |

If the desktop client depends on new server or schema behavior, those deploys
must succeed before `desktop_publish.yaml`. Independent website or docs updates
can finish in parallel, but the release is incomplete until they are live or
explicitly deferred.

## Trigger Stable Release

Dispatch desktop, CLI, and API verification from the candidate Nightly tag, then identify each run
and verify `headSha` equals the recorded candidate before accepting any job.
Reuse an existing successful run only if it covers the exact SHA and all
required jobs; path-filtered or skipped jobs are not coverage:

```bash
CANDIDATE_REF=desktop_nightly_v<nightly-version>
gh workflow run desktop_ci.yaml --ref "$CANDIDATE_REF"
gh workflow run cli_ci.yaml --ref "$CANDIDATE_REF"
gh workflow run api_ci.yaml --ref "$CANDIDATE_REF"
```

Verify every native job and the source-rebuilt CloudSync artifacts, including
both macOS architectures, Windows, and both Linux architectures. Pull-request
runs skip the desktop native jobs. Require both CLI jobs and the API job to pass
for this candidate as well. Keep the candidate fixed through publication.

After candidate testing, verify the candidate remains an ancestor of main,
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

Verify the bundled CLI in the candidate artifacts on every shipped platform,
using the platform runner when necessary. Confirm `APP_VERSION` reached the
CLI build and the packaged executable's `--version` reports the explicit desktop
version. Check its help and a real stdio MCP initialize/discovery exchange
against the reviewed contract, using an isolated fixture database. Keep stdout
protocol-only and verify clean shutdown. A developer binary on `PATH` is not
evidence for the packaged CLI. After updating, verify the supported CLI installer
resolves to the new bundled executable; record unavailable platform checks.

Before the desktop publish dispatch, finish the hosted-service deploys above
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

## Publish and Verify Related Surfaces

Complete the hosted-service deploys above, then verify each live result:

1. **API and hosted MCP:** confirm `/health` reports the deployed API version,
   MCP authentication discovery and OAuth resource metadata are correct, and an
   authenticated read exercises the affected contract when credentials are
   available. Health or an unauthenticated `401` alone does not verify tool
   behavior; report unavailable authenticated checks explicitly.
2. **Stripe:** confirm the `stripe_v*` tag matches the deploy run SHA and the
   BlackMushi billing app `/health` returns ok.
3. **Hosted Postgres:** if `db_cd.yaml` ran, record that run URL and SHA. If
   it did not run, record the last applied migration SHA and why no push was
   needed.
4. **Docs and website:** verify the configured Mintlify deployment includes the
   merged docs changes, then check affected live pages, examples, navigation,
   `https://docs.anarlog.so/skill.md`, and its LLM indexes for the changed content.
   Verify website/changelog/discovery updates through `web_cd.yaml` and the live
   URLs. Reuse the desktop Linux APT web deployment when it already includes
   them. Mintlify and Vercel are separate deployments; a merge, HTTP 200, or a
   successful web build does not establish that the docs content is current.
5. **Agent packages:** verify the published repository manifests and skill
   mirrors, and any affected external catalog's accepted version. Check the
   supported install/update path resolves the expected version. An existing
   installed plugin cache may remain older; record that separately and provide
   update instructions rather than claiming all installations updated.

Record each publication's source SHA, version (where applicable), run/deployment
URL, and observed result. API and plugin versions remain independent. Desktop
and watchOS share a marketing version; mobile uses its own. Each store's
actual availability must be verified separately. If a required surface is
deferred, state its impact and the user's explicit deferral.

## Mobile Store Distribution

Read `apps/mobile/AGENTS.md`, `apps/mobile/app.json`,
`apps/mobile/app.config.ts`, `apps/mobile/eas.json`, the EAS build hook, and the
current `mobile_ci.yaml`. Use Expo's store-distribution skill when available and
verify commands against the installed EAS CLI and current official docs:

- https://docs.expo.dev/submit/ios/
- https://docs.expo.dev/submit/android/
- https://docs.expo.dev/submit/eas-json/
- https://docs.expo.dev/build-reference/app-versions/

### Identity, versions, and credentials

Run EAS commands from `apps/mobile` with `APP_VARIANT=stable`. Use the repository's
`stable` profile, not an assumed `production` profile. Pin and record the EAS CLI
version used for the release. Verify the signed-in account and project before
building or submitting. Current repository identities are:

- Project: `@john_fastrepl/anarlog-mobile`, ID `fcaa4e46-0da5-4dfc-a9e2-6447de0030d2`
- iOS bundle ID and Android package: `so.anarlog.mobile`
- App Store Connect app ID: `6807350358`
- EAS build environment: `production`; app variant: `stable`

Re-read these values rather than treating this list as authority if configuration
changes. Never access an environment whose name matches `*-char`.

The mobile marketing version comes from `apps/mobile/release-version.json`
through `apps/mobile/app.config.ts`. Run
`node scripts/release-version.mjs --mobile --check` before building. Use
`node scripts/release-version.mjs --mobile <major.minor.patch>` to prepare a
new mobile version. Do not bump desktop `release-version.json` for a mobile
release, and do not edit the generated watch configuration from a mobile bump.
Keep `appVersionSource: remote` and `autoIncrement: true` for iOS build numbers
and Android version codes, and never reset those counters to match the marketing
version. Check remote build history and store versions before selecting a
build. Merge intentional version/profile changes before freezing the candidate.

Confirm signing and submission credential availability without printing secrets.
Use credentials already managed by EAS where possible. Google Play submission
requires the correct app record and a service account with access to its testing
track. Do not commit credential files or broaden account permissions to bypass a
failed submission.

### Native verification and candidate builds

Dispatch `mobile_ci.yaml` from `main` and verify its run SHA matches the mobile
candidate. Wait for `mobile_checks`, `ios_build`, `android_build`, `watchos_build`,
and the aggregate job; pull-request runs skip native builds.

- iOS dispatch rebuilds the CloudSync framework, runs `test-ios.sh` on a
  simulator, builds Release, and verifies embedded CloudSync and App Shortcuts.
- Android dispatch rebuilds all shipped CloudSync ABIs, builds Release, and
  checks the packaged libraries for the request-deadline patch marker. The
  current workflow does not execute the native CloudSync cancellation suite on
  Android. Report this coverage gap; a marker check is not a passing runtime test.
- Record the `cloudsync-ios-<sha>` and `cloudsync-android-<sha>` artifacts and all
  job results. Never report desktop tests as native mobile coverage.

Build from a clean checkout of the merged candidate, including Git LFS objects
and submodules. Do not upload a combined GitButler workspace or use `EAS_NO_VCS`
to hide source identity. If using an exported source archive, record its origin
SHA and hash and verify that no local changes entered it. The EAS post-install
hook must generate the native bridge before packaging. EAS builds use the
candidate's committed CloudSync bundle; a separate CI rebuild alone does not
prove which bytes are embedded in a signed store artifact.

For an exported source tree, resolve both `EAS_PROJECT_ROOT` and `apps/mobile`
to their physical paths before invoking EAS. Confirm their relative path is
exactly `apps/mobile`. On macOS, mixing `/tmp` with its physical `/private/tmp`
path produces an invalid project directory in the remote build job. Check the
job's `projectRootDirectory` before accepting a build.

```bash
APP_VARIANT=stable eas build --platform ios --profile stable --non-interactive --no-wait
APP_VARIANT=stable eas build --platform android --profile stable --non-interactive --no-wait
APP_VARIANT=stable eas build:view <build-id> --json
```

Record each build ID, source SHA, profile, app version, build number/version
code, status, artifact URL, and SHA-256. Verify an iOS device archive and Android
AAB, not a simulator build or development APK. Inspect packaged identity,
CloudSync inclusion, and signing metadata before submission. Never select
`--latest` when concurrent builds can select a different candidate.

### TestFlight and Google Play internal testing

Use the explicit completed build IDs:

```bash
APP_VARIANT=stable eas submit --platform ios --profile stable --id <ios-build-id> --non-interactive --no-auto-testflight-setup --wait
APP_VARIANT=stable eas submit --platform android --profile stable --id <android-build-id> --non-interactive --wait
```

For Google Play internal testing, require the selected submission profile to
specify `android.track: internal`. A `completed` release makes the build
available on that track; `draft` uploads it without completing rollout. Do not
silently switch to `production`. An EAS `distribution: internal` build is a
separate sideloading mechanism and is not Play internal testing.

Follow the returned submission URLs to terminal status. For iOS, verify Apple
processing completes and the exact version/build appears in TestFlight; check
availability to the intended existing tester group. External TestFlight testing
can require Beta App Review. For Android, verify the exact version code on the
internal track and the release status. Report missing credentials, app setup,
store processing, or tester-group access as concrete pending steps.

Current EAS Submit enables automatic TestFlight setup by default, which can
create a group and invite every App Store Connect admin. Keep
`--no-auto-testflight-setup` unless those invitations were explicitly requested.
An upload request alone does not authorize inviting additional testers.

### Public App Store and Google Play releases

Run only when public distribution was requested. EAS iOS submission uploads to
App Store Connect/TestFlight; public distribution additionally requires selecting
the build for an App Store version, completing metadata and review requirements,
submitting for review, and verifying the approved release becomes available.
Google Play public distribution requires an explicitly selected production
profile or promotion of the verified internal build, with the requested rollout
fraction and release status. Keep store review and public availability distinct
from a successful upload. Never accept new legal agreements on the user's behalf.

## Final Checks

Before reporting success, capture:

- explicit stable version, candidate SHA, Nightly release/tag and testing evidence
- dry-run workflow URL and head SHA
- publish workflow URL and head SHA
- `desktop_v<version>` tag
- GitHub release URL
- whether CrabNebula publish completed
- changelog URL
- CLI version from each platform's packaged artifact and installer/update result
- CLI/API candidate CI runs and contract checks, including stdio MCP discovery
- hosted API/MCP deployed version, source SHA, deployment URL, and live checks
- Stripe deploy decision, `stripe_v*` tag or reuse reason, and live `/health`
- hosted Postgres deploy decision, `db_cd.yaml` run or reuse reason
- published docs, skill, LLM indexes, and website content verification
- plugin package/catalog version and install/update result when affected
- release surface review, with reasons for unchanged/not-applicable surfaces and
  explicit deferrals or pending publications
- stable DMG SHA-256
- Microsoft Store submission/certification state and Linux package publication state
- mobile version, build IDs/numbers, candidate SHA, artifact hashes, submission
  URLs, TestFlight processing/tester availability, and Google Play track/version
  code when mobile distribution was requested
- pending store reviews, native coverage gaps, or unavailable checks, separately
  from completed publication

If the workflow fails, inspect the failed job logs with:

```bash
gh run view <run-id> --log-failed
```
