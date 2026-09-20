---
name: release-docs
description: Prepare Anarlog release notes and documentation, publish the website changelog when the app is published, and verify the live content without a separate request.
metadata:
  internal: true
---

# Release Docs

Review every release. Inherit scope and session authorization from
[Release a New Version](../release-new-version/SKILL.md). Own documentation and
changelog publication through completion, not just the source file or PR.
For mobile-only distribution, review affected mobile and shared documentation;
desktop changelog preparation and publication below are not applicable.

## Before candidate freeze

1. Read `docs/AGENTS.md` and [New Changelog](../new-changelog/SKILL.md).
   Compare all user-facing changes since the previous stable release with
   `packages/changelog/content/<version>.md`, `docs/`, `docs/docs.json`, and
   `apps/web/public/llms.txt`. Cover guides, installation/upgrade instructions,
   CLI/MCP/API examples, screenshots, troubleshooting, and platform availability.
   Preserve external-contributor credits and describe shipped behavior only.
2. Prepare the changelog with its `date` and plain-text `summary`. Run changed-file
   formatting, `pnpm -F @anlg/changelog typecheck`, and affected web checks from
   `web_ci.yaml`/root `AGENTS.md`. For docs, use the Mintlify version pinned in
   `cli_ci.yaml` to run `validate` and
   `broken-links --check-anchors --check-redirects` from `docs/`; preview affected
   pages and navigation. Coordinate public skill changes with
   [Agent packages](../release-cli/references/agent-packages.md).
3. Merge required content before freezing the desktop candidate. Keep unreleased
   notes hidden: deployable web builds include only published, non-draft,
   non-prerelease GitHub `desktop_v<version>` releases. Verify the production index
   omits the candidate and its direct URL returns 404 before publication. A local
   production build can test this; `vite dev` intentionally previews drafts.

## Publish with the app

These steps are part of the authorized stable release, without another user prompt.
Read `desktop_publish.yaml`, `web_cd.yaml`, and `apps/web/changelog-build.ts`
before selecting the deployment; do not assume merging the notes publishes them.

1. Wait for the exact GitHub/CrabNebula app version to be published. A website
   build made before that event deliberately excludes its notes, even when its
   source contains the Markdown. Do not weaken that gate or move the desktop tag.
2. Complete the website publication in the same release operation. The desktop
   workflow currently calls `web_cd.yaml` through `linux-apt-deploy` after merging
   Linux metadata. Follow an existing matching deploy to completion and reuse it
   if its build began after app publication and its source includes the notes.
3. If no such deployment exists, including when Linux is excluded, verify the
   approved `main` source contains the notes and dispatch `web_cd.yaml --ref main`
   with `gh workflow run`. Verify the returned run's `headSha` against that source
   and follow it to completion. Avoid duplicate deployments and preserve newer APT
   metadata; never deploy the old desktop candidate over newer website/package
   content. If `main` contains unrelated unapproved web changes, resolve that source
   scope before deploying and report the concrete publication blocker.
4. Fetch `https://anarlog.so/changelog/<version>/` and
   `https://anarlog.so/changelog/` from the live site. Verify the expected version,
   summary and release-note sections/body, plus the index link. For the latest
   release, verify it is marked Latest. HTTP 200, a generic page, a source file, a
   search-engine snapshot, or a green build alone is insufficient. If content is
   stale, inspect the actual deployment revision and cache response and resolve it
   within this release operation. Do not report completion with missing notes.
5. Verify GitHub release notes link to the working page and check public download
   and update routes for the published version. In-app bundled notes, updater
   payload notes, GitHub release text, and the website are separate surfaces; do
   not use one as proof of another or promise inline notes where only a link exists.

## Other documentation and completion

Publish changed Mintlify content through its configured connected deployment and
verify the actual changed content at `https://docs.anarlog.so`, including
`skill.md`, `llms.txt`, and `llms-full.txt` when affected. Mintlify and Vercel are
separate publication paths; inspect current configuration rather than assuming
that a website deployment also updates docs. Reuse unchanged surfaces with evidence.

Return content revisions, validation, website run URL/SHA, live changelog page and
index evidence, other published docs, and any blocker or explicit deferral to the
coordinator. Do not send announcements or newsletters unless separately requested.
