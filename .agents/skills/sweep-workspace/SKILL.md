---
name: sweep-workspace
description: Use only in the Anarlog repository when the active checkout branch is exactly gitbutler/workspace. Coordinate a GitButler workspace PR sweep by merging already-ready PRs, updating the workspace, then running create-prs, fix-prs, and update-prs in that order.
---

# Sweep GitButler workspace

## Applicability

Use only in the Anarlog repository or its checkouts/worktrees when the active
checkout branch is exactly `gitbutler/workspace`. Before running this workflow,
check with `git symbolic-ref --quiet --short HEAD` (a read-only branch check).
On any other branch or detached HEAD, skip this skill and use the normal Git
workflow. Do not initialize GitButler or switch branches to activate the skill.
Use this checkout's skills, not a global copy or another checkout's skills.

Use this skill when asked to sweep PRs in the Anarlog GitButler workspace.

Read the repository-local [but](../gitbutler/SKILL.md) skill and the active repository's AGENTS.md and workflow
skill when present. Use `but` for version control and the repository's preferred
authenticated forge tools for PR operations. Derive the repository and configured
target from the current workspace.

This skill coordinates the three focused workflows. Read the repository-local
skill when its stage begins: [create-prs](../create-prs/SKILL.md),
[fix-prs](../fix-prs/SKILL.md), and [update-prs](../update-prs/SKILL.md).
Do not automatically spawn subagents or run stages concurrently.

A sweep invocation authorizes in-scope PR creation, pushes, necessary history
rewrites, workspace updates, conflict repairs, review replies/resolutions,
metadata edits, and ready non-draft merges. Preserve unrelated work and active
collaborators' edits. Editing this skill does not execute a sweep.

## 1. Merge PRs that are already ready

- Inspect `but status --json` and map applied branches to live open PRs by exact
  head repository/branch. Record bases, remote SHAs and stack dependencies. Honor
  a narrower user scope; skip missing, closed or merged PRs in this stage.
- Inspect current published code, all paginated reviews/threads/conversation
  findings and current checks. Merge only non-drafts with passing applicable
  required checks, satisfied review requirements, no conflicts, and no remaining
  actionable or unverified findings. Include outdated unresolved findings and
  linked reviewer reports; incomplete access/coverage blocks readiness.
- Do not fix code, publish local changes or create PRs to make this initial queue
  ready. An unpublished fix does not clear a finding on the remote head. A fix
  in an upper PR qualifies only when that verified fix is included in the same
  ready merge prefix; an earlier prefix without it is not safe.
- Merge nearest trunk first, in batches of at most 10 PRs total. Prerequisites
  must already be merged or included in the same supported stack merge. Skip
  blocked ancestors and their descendants; continue independent ready stacks.
- Immediately before each request, recheck every included PR's head, base,
  draft state, checks, reviews and mergeability. Use the repository's merge
  method and the reviewed SHA; changed heads invalidate readiness.
- Prefer a supported GitButler PR merge operation with head protection. Otherwise
  use authenticated forge tools: ordinary GitHub PRs support `gh pr merge <PR>
--match-head-commit <SHA>` with the repo's merge method. Native GitHub stacks
  need the supported asynchronous stack merge API; verify its current request
  fields and head guard rather than guessing or falling back to a legacy merge.
  Neither `but land` nor enabling auto-merge substitutes for this stage.
- Confirm each requested merge through live terminal PR/operation state before
  advancing dependent PRs or cleaning up branches. Re-read affected descendants
  after each confirmed merge. Async acceptance is not merge completion; retain
  pending operation IDs for continuation and do not mutate affected stacks
  while their merge is pending.

## 2. Update the workspace

Before any workspace update, confirm pending merge operations have reached a
terminal state. While a merge is pending, defer `but pull` and all create, fix,
and metadata-update operations on its affected stacks. Continue independent
stacks only when the available operations can demonstrably leave pending stacks
untouched; `but pull --check` is inspection, not permission to rebase them. If
pending stacks cannot be excluded from the workspace update, defer the update
and dependent stages until confirmation, retaining operation IDs and reviewed
heads for the follow-up.

After this guard is satisfied, update from the configured target with `but pull`
so confirmed merges are incorporated before running the focused workflows.
With other agents' branches applied, use `but pull --check` first to understand
affected work. The sweep authorizes scoped updates; work outside that scope needs
authorization before it moves. Preserve dirty work and follow the `but` skill's
parking/restoration rules only for in-scope changes. Do not repoint the target.

Record any conflicts introduced by the update for `fix-prs`. If those conflicts
prevent PR creation, let `create-prs` report the blocked branches, then resolve
them in the next stage; retry creation for those branches on the next sweep.
If the update itself cannot complete, report its blocker instead of claiming
the workspace is current or proceeding on dependent work.

## 3. Run the focused workflows in order

1. **`create-prs`** — create missing PRs for the remaining scoped branches.
2. **`fix-prs`** — resolve workspace conflicts and fix active PR reviews and CI.
3. **`update-prs`** — rewrite active PR titles/descriptions from their complete
   published changes, including the repairs just made.

Pass the repository, scope, current stack map, known blockers, and pending merge
operation IDs, affected branch names and reviewed heads between stages.
Exclude stacks with pending merges from every stage; child workflows must not
pull, publish, edit metadata, or otherwise mutate those stacks. Refresh live
branch/PR mappings after mutations. A blocked branch or pending CI
does not prevent independent work in later stages. Run the metadata stage after
all currently actionable fixes are prepared/published, even when remote checks
are pending. Local-only changes remain an explicit description gap.

Do not merge newly created or repaired PRs at the end of this pass. Once ready,
they enter the initial merge stage of the next sweep.

## Continue when external work is pending

The coordinator owns follow-up scheduling; child workflows report waits to it
instead of creating duplicate monitors. Finish actionable stages before waiting.
Use available per-PR review/CI subscriptions, or reuse/create a thread heartbeat
every 10 minutes when subscriptions are unavailable. Do not watch, sleep or
repeatedly poll in the foreground.
Schedule a next sweep for newly created/repaired PRs awaiting the initial merge
stage even if their checks are already green; use the thread heartbeat when no
further subscription event is expected.
Retain already-ready PRs deferred solely by the merge batch limit as actionable
work and schedule their next sweep even when no external checks are pending.

Save repo, scope, stack map, remote heads, findings/coverage, pending merge IDs,
completed stages, conflicts and required user decisions. Resume by confirming
pending merges, then run the same merge → workspace update → create → fix →
update sequence using fresh evidence. A timer does not supply user approval.
Notify only for meaningful changes, completion, failure or required action.
Pause when all scoped work is complete, or only user decisions/drafts remain
and no actionable work or external checks are pending. Report scheduling failures.

Report confirmed merges, workspace update, PRs created, repairs, metadata updates
and remaining blockers briefly. Keep local verification, remote checks, merges
and deployment as separate facts.
