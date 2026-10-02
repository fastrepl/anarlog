---
name: create-prs
description: Use only in the Anarlog repository when the active checkout branch is exactly gitbutler/workspace. Create missing pull requests for branches in the current GitButler workspace, preserving stack bases and skipping existing PRs. Use when asked to open or publish workspace PRs that do not exist yet.
---

# Create missing GitButler PRs

## Applicability

Use only in the Anarlog repository or its checkouts/worktrees when the active
checkout branch is exactly `gitbutler/workspace`. Before running this workflow,
check with `git symbolic-ref --quiet --short HEAD` (a read-only branch check).
On any other branch or detached HEAD, skip this skill and use the normal Git
workflow. Do not initialize GitButler or switch branches to activate the skill.
Use this checkout's skills, not a global copy or another checkout's skills.

Use this skill when asked to create missing PRs for branches in the Anarlog GitButler
workspace.

Read the repository-local [but](../gitbutler/SKILL.md) skill and the active repository's AGENTS.md and workflow
skill when present. Use `but` for version control and stack-aware PR creation,
and the repository's preferred authenticated forge tools for reads. Derive the
repository, push remote, and target from the current workspace.

Invoking this workflow authorizes creating missing PRs and the in-scope pushes
that creation requires. Creating this skill alone does not run that workflow.
Do not merge PRs, enable auto-merge, or publish unrelated work.

## Discover missing PRs

1. Read `but status --json` to map applied branches, their commits, existing
   review IDs, and dependencies from base to tip. Scope defaults to all applied
   branches; honor a narrower user selection. Exclude trunk, workspace/internal
   refs, empty branches, and branches already merged upstream.
2. Verify each branch against live forge state using the exact head repository
   and full head branch name. A cached review ID is only a lead. Include drafts
   as existing PRs and paginate results. Skip branches with an open matching PR.
   Investigate a closed/merged PR before treating the branch as new work; do not
   automatically reopen it or publish already-landed commits. Report ambiguous
   matches instead of guessing.
3. Inspect each missing branch's complete changes relative to its intended PR
   base with `but show` / `but diff`, including all commits. For a stacked PR,
   describe that branch's contribution and dependency rather than repeating
   ancestors' changes. Preserve unrelated uncommitted changes; do not commit
   everything merely to publish PRs.
4. Check the actual push destinations with `but push <branch-name> --dry-run`
   where publishing could include ancestors or unfamiliar tracking. Never push
   a feature branch into trunk. If creation would publish unrelated ancestor
   work or rewrite shared history without authorization, finish independent PRs
   and report the specific blocked branch. Keep hooks and push protections on.

## Create and verify

- Work from base toward tip. Recheck for an open matching PR immediately before
  each creation to make reruns safe. If none are missing, report that and stop.
- Prepare the title and description from the full branch diff, repository PR
  template, and verified validation. Lead with the problem and resulting
  behavior; include material limitations and existing issue links. Keep it
  succinct, without agent attribution or unsupported test/shipping claims.
- Write a temporary UTF-8 message file outside the repo: first line is the
  title, then a blank line and the body. Use:

  ```sh
  but pr new <full-branch-name> -F <message-file>
  ```

  This already pushes the branch and its ancestors. Do not push separately or
  use `gh pr create`, which loses GitButler's stack base/metadata handling.
  Add `--draft` when requested or the work is knowingly incomplete; preserve
  the state of existing PRs. Prefer individual creation in base order when
  every new PR needs its own accurate description. Whole-stack `-t` creation
  gives ancestors default messages and needs a subsequent metadata pass.

- Inspect creation output for partial success. On an error or uncertain result,
  re-query matching PRs and remote heads before retrying. A stack-sync warning
  can follow a successful creation; repair/report that gap rather than create
  a duplicate.
- Verify each created PR's URL, open state, published head, base and stack
  relationship through the forge. Attach every created PR using
  `mcp__codex_app__attach_artifact` when available.

Report created PR links, existing PRs skipped, and any blocked branches. PR
creation does not establish green CI, approval, merge, or deployment.
