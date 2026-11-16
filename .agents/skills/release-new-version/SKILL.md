---
name: release-new-version
description: Coordinate focused release skills to prepare, publish, and verify Anarlog stable desktop releases and affected public surfaces, plus mobile distribution when requested.
metadata:
  internal: true
---

# Release a New Version

Use this skill when preparing, publishing, or verifying an Anarlog release. This
is the coordinator: read all five focused skills below for the surface review,
then execute their required work within the requested release scope. The user
does not need to invoke each skill or ask separately for the changelog.
Repository paths are relative to the checkout root; workflow filenames below and
in the child skills refer to `.github/workflows/`.

## Scope and authorization

- For desktop preparation or publication, select the stable version using the
  policy below; the user does not need to supply a version. For mobile-only distribution, require the mobile version; no desktop version or desktop release work is required.
  Publish only a verified candidate merged into `main`, never the combined
  GitButler workspace. Nightly publication is retired; preserve historical tags
  and installed-client compatibility.
- A desktop or mobile publication request includes its own changelog, affected public
  surfaces, and website publication. Desktop releases also include the existing
  Microsoft Store lane. Preparation alone does not
  authorize publication. Mobile releases ship publicly to the App Store and Google Play.
  There is no Mac App Store release lane; do not recreate one.
- Child skills inherit session authorization and scope. Continue already
  authorized publication without asking again. They do not authorize unrelated
  deployments, announcements, emails, customer-data resets, or store agreements.
- Review every surface; update only those with drift. Keep API, website, plugin,
  and mobile versions independent. Record reuse evidence, inapplicability with a
  reason, or explicit user deferral with impact. Do not redeploy unchanged services
  merely to refresh timestamps. A website build before desktop publication cannot
  expose the new changelog and is not reusable evidence for that publication step.
- Preserve final stable package checks, exact source/provenance requirements,
  and downgrade compatibility in the desktop procedure. Proceed with publication
  when the required technical release gates pass. Broad QA through
  `qa-critical-ux` or `qa-cli-mcp-api` is separate and runs
  only when requested; packaging and release verification remain required.

## Desktop version selection

- Honor a version supplied by the user. When continuing this release, reuse its
  recorded version rather than selecting another one on each run.
- Otherwise, query GitHub releases and select the next patch after the highest
  published, non-draft, non-prerelease `desktop_v<major>.<minor>.<patch>` version.
  Paginate as needed; ignore mobile, Nightly, draft, and prerelease entries.
  For example, `desktop_v1.4.32` selects `1.4.33`.
- State the selected version and baseline, record them in the release issue,
  and proceed without confirmation. Do not infer a minor or major bump from
  the size of the diff or use the combined workspace's version as the baseline.
- Check existing release issues, preparation PRs, drafts, and tags for the
  selected version. Resume matching work; ask only when the baseline cannot be
  verified or conflicting release ownership/version intent needs a decision.
  Never overwrite a published release or move an immutable tag.
- Pass the selected exact version to the version script and every release
  workflow. Automatic selection does not change publication authorization,
  candidate verification, or any other release gate.

## Release Surface Review

Review the full product diff since the previous published release of the requested
product, not only each surface's own paths. Also compare each independently
published surface with its live revision so previously unshipped changes are included. Implement required updates before
freezing the candidate. Document intentional capability differences or explicit
deferrals; do not silently omit CLI, MCP, or docs support for a new product feature.

| Skill                                              | Owns                                                                                                                                |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| [Release Docs](../release-docs/SKILL.md)           | Changelog preparation, website publication after the app release, live index/page checks, Mintlify guides, examples, and discovery. |
| [Release API](../release-api/SKILL.md)             | API/generated contracts, hosted services, billing, hosted database, compatibility, deployment, and live operations.                 |
| [Release CLI](../release-cli/SKILL.md)             | Commands, help/output, shared agent access, packaged CLI, install/update path, and shared agent packages.                           |
| [Release MCP](../release-mcp/SKILL.md)             | Local and hosted protocols, tools/resources, schemas, authentication, and shipped runtime checks.                                   |
| [Release Telemetry](../release-telemetry/SKILL.md) | Product events, crash reporting, consent/redaction, release attribution, delivery, and symbolication.                               |

CLI and MCP share one [agent-package review](../release-cli/references/agent-packages.md).
Reuse evidence and publish shared packages once. Each skill returns decisions,
candidate/published revisions, checks, live or artifact evidence, and unresolved work.

## Execution order

1. Record the selected version, platforms/channel, prior published versions, and
   candidate source in the matching Linear release issue. Follow
   [Anarlog workflow](../anarlog-workflow/SKILL.md); keep one release record.
2. Read the five surface skills and current workflows. Implement and validate
   needed changes, including the Desktop or Mobile changelog for each requested stream. Prepare
   Mobile notes before freeze; record availability only after verifying the build
   is available to the intended store/tester audience.
   Merge release preparation to `main`, then freeze its exact SHA. A new source change requires renewed affected
   verification; do not mix runs or artifacts from different candidates.
3. When desktop is in scope, follow [Desktop release procedures](references/desktop.md) for native CI,
   CloudSync source rebuilds, staging verification, first-attempt stable dry run, packaged
   verification, upgrade/restart, immutable tags, and publication. Follow
   [Mobile store distribution](references/mobile.md) only when requested.
4. Publish required backward-compatible hosted dependencies through Release API
   before dependent clients. Publish other affected surfaces through their owning
   skills; verify actual workflow SHAs and terminal results.
5. **After verified app availability, complete Release Docs publication.**
   For desktop, reuse a successful post-publication Linux APT web deploy that includes the notes;
   otherwise dispatch `web_cd.yaml` within the existing release authorization.
   Follow it to completion and verify the full version page and public index.
   For mobile, merge the verified availability record before the website build.
   Do not leave this for a user reminder, separate request, or later release.
6. Complete each affected skill's shipped/live checks and the release record below.
   If a workflow fails, inspect `gh run view <run-id> --log-failed`. Report the
   concrete unfinished step; do not label the overall release complete while
   required publication or verification remains pending without explicit deferral.

## Final Checks

Record applicable checks in the release issue with links to existing evidence
rather than duplicate routine comments. For mobile-only distribution, mark desktop
version, staging, artifacts, Desktop changelog, and desktop store checks not applicable;
review affected shared surfaces and complete the mobile checks.

- Selected exact stable version and baseline, candidate SHA, staging run and verification evidence,
  first-attempt dry-run and publish URLs/head SHAs.
- Immutable `desktop_v<version>` tag, GitHub/CrabNebula publication, asset
  hashes/signatures, final stable install/upgrade/restart, and updater/download routes.
- Packaged CLI version on each shipped platform, installer/update resolution,
  CLI/API CI and contracts, local stdio MCP and hosted MCP checks.
- Hosted API, billing, and Postgres deployment/reuse decisions; source revisions,
  run URLs, versions where applicable, and affected live behavior.
- Website deployment URL/SHA after app publication, changelog version URL and index
  content, published Mintlify docs/skill/LLM indexes, and agent-package/catalog
  version and installation evidence when affected.
- Telemetry update/reuse decisions, consent checks, shipped version attribution,
  destination delivery and symbolication evidence where affected.
- Microsoft Store submission versus certification/availability, Linux APT results
  versus AUR availability, and any pending external processing.
- When mobile was requested: source/version, build IDs/numbers, hashes, submission
  URLs, App Store review/release status, Google Play production version code,
  verified Mobile publication record, and live Mobile changelog page/index.
- Reasons for unchanged/not-applicable surfaces, explicit deferrals and impact,
  native coverage gaps, unavailable checks, and unresolved incidents. Publication
  does not by itself establish customer recovery or fix every known incident.

App publication, overall release completion, and store approval are separate
facts. Report them accurately; never make the user discover a missing publication.
