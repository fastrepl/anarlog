---
name: new-changelog
description: Prepare Desktop or Mobile release notes, grounded in changes since the preceding release and gated on verified availability.
metadata:
  internal: true
---

## Channel contract

Nightly publication is retired. Prepare stable notes in
`packages/changelog/content/desktop/<version>.md`, covering all desktop user-facing
changes since the previous stable, including changes once described in Nightly
notes. Preserve historical Nightly notes; do not generate or announce new Nightly
releases.

Only released stable entries appear on the website. Deployable builds include a
versioned file only when GitHub has a published, non-draft, non-prerelease
`desktop_v<version>` release. A file on main or its frontmatter date is not
publication evidence. Local development can preview stable drafts.

## Stable version

Determine the next desktop version by inspecting `.github/workflows/desktop_cd.yaml` and running:

```bash
doxxer --config doxxer.desktop.toml current
doxxer --config doxxer.desktop.toml next patch
```

Create the new Markdown file in `packages/changelog/content/desktop` for that version.
Read that directory’s `AGENTS.md`, then run
`node scripts/sync-desktop-changelogs.mjs` to refresh legacy desktop URLs and
include the generated copies in the same commit.

When preparing a release, also follow the
[release surface review](../release-new-version/SKILL.md#release-surface-review).
Check the product changes for CLI, local and hosted MCP, API, agent-package, and
documentation updates before freezing the release candidate. Record any gaps
in the release task; a changelog alone does not establish release readiness.
Creating a changelog does not itself dispatch or publish a release.
Keep the notes prepared and merged before freezing the desktop candidate, but
verify that the public index and direct version URL exclude them until release.
For an authorized stable release, [Release Docs](../release-docs/SKILL.md) owns
website publication immediately after the app is published and verifies the full
version page and index. It reuses the post-publication Linux APT web deployment
when suitable or dispatches the missing website deployment within that release.
The user must not need to ask separately. This authoring skill alone still does
not authorize a release or deployment.

Each changelog file must start with frontmatter that includes both `date` and
`summary`:

```md
---
date: "YYYY-MM-DD"
summary: "One concise, user-facing sentence for the changelog index preview."
---
```

Keep `summary` plain text. Do not use markdown or custom tags in it. The web
changelog index renders this field directly, so it should describe the release
at a glance without leaking implementation details.

Follow the writing rules in `packages/changelog/content/AGENTS.md`, including
[contributor credit](../../../packages/changelog/content/AGENTS.md#contributor-credit).

## Mobile releases

Use `packages/changelog/content/mobile/<version>.md` and read that directory’s
`AGENTS.md`. Select the explicit requested marketing version or
`apps/mobile/release-version.json`; never bump or reuse the desktop version.
Ground beta notes in the preceding available mobile release's source SHA.
Ground stable notes in the preceding stable mobile release's source SHA so
changes previously tested in beta remain included. With no prior release,
review the initial mobile user-facing behavior without inventing publication.
The changelog workflow accepts `stream`, `channel`, and an optional `version`.
Use its optional `source_sha` to select an exact candidate or promote an existing
beta build without including later commits. Without it, stable preparation uses
HEAD when only a beta record exists for the version; revising notes for an
already published release in the same channel uses its recorded source.

Prepare notes before freezing the candidate. Do not create the publication
record during preparation. After verified store/tester availability,
[Release Docs](../release-docs/SKILL.md) owns the adjacent `<version>.json`
record and website publication. Record only available platforms and preserve
channel/destination labels. A submitted build is not a published release.

## Contributor credit

If a user-facing change came from a pull request by someone outside the
Fastrepl org, acknowledge them on that changelog item. Do not credit org
members, collaborators, owners, or bots.

Look up each merged PR in the version range and credit the author when
`author_association` is not `MEMBER`, `OWNER`, or `COLLABORATOR`, and the
user is not a bot. Put the thanks at the end of the item, after the
user-facing sentence, and link the GitHub username. Do not put credits in
`summary`.

```md
- Use the actual default microphone on Linux instead of silently recording
  from ALSA's null device. Thanks [@jacopone](https://github.com/jacopone).
```
