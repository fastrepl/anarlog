---
name: update-prs
description: Use only in the Anarlog repository when the active checkout branch is exactly gitbutler/workspace. Update titles and descriptions of all active PRs in the current GitButler workspace from each PR's complete published diff, including follow-up fixes, while preserving GitButler stack metadata and repository templates.
---

# Update workspace PR titles and descriptions

## Applicability

Use only in the Anarlog repository or its checkouts/worktrees when the active
checkout branch is exactly `gitbutler/workspace`. Before running this workflow,
check with `git symbolic-ref --quiet --short HEAD` (a read-only branch check).
On any other branch or detached HEAD, skip this skill and use the normal Git
workflow. Do not initialize GitButler or switch branches to activate the skill.
Use this checkout's skills, not a global copy or another checkout's skills.

Use this skill when asked to refresh active PR titles and descriptions in the Anarlog
GitButler workspace.

Read the repository-local [but](../gitbutler/SKILL.md) skill and the active repository's AGENTS.md and workflow
skill when present. Use `but` to inspect workspace branches and stack
relationships, and the repository's preferred authenticated forge tools for PR
metadata. Derive the repository from the current workspace.

Invoking this workflow authorizes editing titles and descriptions of scoped
PRs. Creating the skill alone does not execute it. Do not publish commits, create
or merge PRs, change bases/draft state, or modify reviewers, labels or milestones.

## Read the complete changes

1. Read `but status --json` and map applied branches to live open PRs by exact
   head repository and full branch name, including drafts. Exclude unrelated
   repository PRs and closed/merged PRs. Honor a narrower user selection and
   report ambiguous mappings rather than editing a guessed PR.
2. For each PR, retrieve its current title/body, base/head refs and SHAs,
   complete commit list and full base-to-head diff. Paginate files and commits;
   detect omitted/truncated patches and inspect those files through authenticated
   repository contents or available `but` inspection. Use the PR's actual base,
   which may be an earlier stack branch, not the workspace's combined diff.
3. Read all changed areas and relevant review fixes to understand the final
   implementation. Cover the entire PR, not just its latest commit, original
   description, or this chat's edits. In stacked PRs, describe the incremental
   contribution and material dependencies without claiming ancestors' work.
4. Use published changes as the source of truth. Local-only fixes are not yet
   part of the PR; report that gap instead of describing them as shipped or
   pushing them during a metadata-only task. Record validation evidence with
   its SHA. Old CI or local workspace tests do not prove the current PR passed.

## Rewrite and save

- Write a concise title naming the final change's intent. Lead the description
  with the concrete problem and resulting behavior. Explain notable decisions,
  relevant validation, and material risks/limitations in proportion to the PR.
  Use a before/after example when it makes behavior clearer.
- Rewrite around the complete final implementation, removing stale scope and
  superseded claims. Omit conversational history, abandoned approaches and a
  commit-by-commit diary. Do not add agent attribution or imply merge/deployment.
- Preserve the repository template's required sections, useful issue links,
  checklists, and unrelated reviewer notes. Update a checklist only with
  evidence. Preserve GitButler-generated stack footers/markers verbatim and
  retain native stack metadata; do not replace the body wholesale with prose
  that loses those relationships.
- Prepare all requested updates before writing. Immediately before each edit,
  re-read state, head/base SHAs and body. If the PR closed, skip it; if its code
  or body changed concurrently, recompute the description and preserve the new
  content. Skip already-accurate titles/descriptions so reruns avoid churn.
- Use a supported GitButler metadata command if the installed version provides
  one. Otherwise use the authenticated forge's PR edit API or CLI for title/body
  only; version-control writes still belong to `but`. For a CLI body, use a
  temporary UTF-8 file outside the repo and `--body-file`, with proper shell
  quoting for the title/path. Prefer structured API arguments to shell
  interpolation. Do not invent `but pr edit` flags.
- Re-read each edited PR to confirm exact title/body, preserved stack text and
  unchanged base/head. If code changed during the write, refresh the summary
  from that head before declaring it current. On an uncertain write result,
  inspect the saved metadata before retrying. Attach updated PRs through
  `mcp__codex_app__attach_artifact` when available.

Report updated PR links, unchanged PRs skipped, and any incomplete coverage or
unpublished changes. This workflow requires no source commit unless separately
requested source edits were made.
