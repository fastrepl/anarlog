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

## Preflight

1. Inspect the workflow before assuming release behavior:

```bash
sed -n '1,280p' .github/workflows/desktop_cd.yaml
```

2. Check the version inputs that the workflow will use:

```bash
doxxer --config doxxer.desktop.toml current
doxxer --config doxxer.desktop.toml next patch
```

Stable desktop releases use `next patch` unless the user explicitly asks for an override version.

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
2. Confirm `packages/changelog/content/<next-patch-version>.md` exists.
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

If using GitButler, prefer:

```bash
but diff
but commit chore/release-changelog -c -m "Update desktop release changelog

Refresh the desktop changelog for the next stable release." --changes <ids>
but pr new <branch-id> -t
```

Use actual IDs from `but diff` / `but status -fv`; do not invent IDs.

## Trigger Stable Release

After the changelog merge lands on `main`, trigger the stable desktop workflow from `main`:

```bash
gh workflow run desktop_cd.yaml \
  --ref main \
  -f channel=stable \
  -f publish=true \
  -f version=
```

Then watch the run:

```bash
gh run list --workflow desktop_cd.yaml --branch main --limit 5
gh run watch <run-id>
```

The stable workflow should:

- compute the version from `doxxer --config doxxer.desktop.toml next patch` when no explicit version is supplied
- build both Apple Silicon and Intel macOS artifacts
- draft, upload, and publish the CrabNebula release when `publish=true`
- create or update `desktop_v<version>`
- create the GitHub release pointing to `https://anarlog.so/changelog/<version>`

## Final Checks

Before reporting success, capture:

- computed stable version
- workflow run URL
- `desktop_v<version>` tag
- GitHub release URL
- whether CrabNebula publish completed
- changelog URL

If the workflow fails, inspect the failed job logs with:

```bash
gh run view <run-id> --log-failed
```

Do not declare the release complete until the stable workflow succeeds.
