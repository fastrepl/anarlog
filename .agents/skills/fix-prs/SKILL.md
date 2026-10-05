---
name: fix-prs
description: Use only in the Anarlog repository when the active checkout branch is exactly gitbutler/workspace. Resolve conflicts in the current GitButler workspace and fix actionable code review findings and CI failures on its active PRs, including drafts. Validate and publish repairs on their owning branches, resolve addressed review threads, and leave PRs open.
---

# Fix workspace conflicts, PR reviews and CI

## Applicability

Use only in the Anarlog repository or its checkouts/worktrees when the active
checkout branch is exactly `gitbutler/workspace`. Before running this workflow,
check with `git symbolic-ref --quiet --short HEAD` (a read-only branch check).
On any other branch or detached HEAD, skip this skill and use the normal Git
workflow. Do not initialize GitButler or switch branches to activate the skill.
Use this checkout's skills, not a global copy or another checkout's skills.

Use this skill when asked to repair workspace conflicts, PR review findings,
or CI failures in the Anarlog GitButler repository.

Read the repository-local [but](../gitbutler/SKILL.md) skill and the active repository's AGENTS.md, workflow,
and testing skills when present. Use `but` for version control and the
repository's preferred authenticated forge tools for PR operations. Derive the
repository and configured target from the current workspace.

Invoking this workflow authorizes workspace updates, conflict resolutions and
their necessary history rewrites, in-scope fixes, local commits, protected pushes,
necessary CI reruns, and evidence-backed review replies/thread resolutions.
Creating the skill alone does not execute it. Do not merge, enable auto-merge,
create missing PRs, change draft state, or bypass branch protection. Other
pushed/shared history rewrites need existing authorization; a new focused commit
on the owning branch can avoid that dependency.

## Resolve workspace conflicts

1. Inspect applied stacks, dirty files and conflicted commits with
   `but status --json` (use `-fv` when file details are needed). Conflict repair
   covers all applied workspace branches by default, including branches without
   PRs; honor a narrower user scope. Record all local branch heads before any
   updates or repairs so rewritten descendants can be identified afterward.
   Coordinate with active agents before shared checkout mutations and preserve
   their unrelated edits.
2. Read the coordinator's pending merge operations, affected branch names and
   reviewed heads before updating. While any pending merge stack remains
   applied, skip the workspace-wide `but pull`; `but pull --check` does not make
   that pull selective. Exclude pending stacks from repairs, and run independent
   repairs only when every GitButler mutation and ancestor push scope can
   demonstrably leave those stacks untouched. Otherwise defer repairs until
   their merge operations reach a confirmed terminal state.
   Once this guard is satisfied, refresh from the configured target with
   `but pull`. When other agents' branches are applied or scope is narrower than
   the workspace, first run
   `but pull --check` to identify affected branches. The default workspace-wide
   invocation authorizes routine in-scope updates and conflict repairs, subject
   to AGENTS.md's approval boundaries. Ask before resolving semantic conflicts,
   dependency updates, generated-file conflicts, or conflicts involving another
   person's work unless existing user authorization explicitly covers that
   particular resolution. Also ask before modifying work outside the authorized
   scope. Keep the target and stack dependencies unchanged. If dirty changes
   block an update, use the `but` skill's parking/restoration procedure only for
   in-scope work.
3. Resolve branches from base toward tip and each branch's commits oldest-first.
   Use the installed `but` skill's conflict tools, preferably
   `but resolve conflicts <full-branch-name>` and
   `but resolve apply <path>:<conflict-number> --commit <full-branch-name>` with
   merged content on stdin or `--file`. Read base, ours and theirs and preserve
   both intended behaviors. Resolution rewrites commit IDs; use branch names
   and fresh output. Do not amend conflicted commits or use raw Git writes.
4. For uncommitted file conflicts, edit the intended content and then mark the
   paths resolved with `but resolve <path>...`. Keep the resulting changes on
   their owning branch when ownership is established; preserve ambiguous dirty
   work rather than commit it into a guessed branch. Regenerate generated files
   through their owning tasks when required instead of hand-editing artifacts.
5. Continue until scoped conflicts are cleared. Make routine resolutions
   directly; if the two sides require an unresolved product or architectural
   decision, report the exact alternatives and continue independent repairs.
   Run relevant checks on the resolved code and refresh the branch/PR map and
   heads before diagnosing reviews or CI. Track remaining conflicts as blockers
   even when every remote PR check is green.

Publish resolved branches that already have active PRs with the consolidated
repair push below. Keep branches without PRs local unless separately authorized
to publish them, and inspect ancestor push scope so they are not sent implicitly.

## Inventory all active PRs

- Map applied stacks with `but status --json` and reconcile exact head branches
  against live open PRs, including drafts. Scope is workspace PRs, not every open
  PR in the repository. Record base-to-tip dependencies, PR URLs and remote SHAs.
- For every PR, paginate submitted reviews, review threads and their comments,
  conversation comments, check runs and commit statuses. Include unresolved
  outdated findings, review summaries and linked reviewer findings. Read failed
  job logs and inspect pending/cancelled checks; green CI alone says nothing
  about unresolved reviews. Inaccessible sources are coverage gaps.
- Keep a findings ledger keyed by PR plus stable thread/comment/check ID: source,
  issue, owning branch/commit, verified head, disposition, evidence, next action.
  Deduplicate reports while retaining their links. Reconcile all outstanding
  findings on each round, including previously resolved findings whose fix is
  still unverified. Do not limit later passes to new comments.
- Trace each finding at its PR's published head and through the entire stack,
  including upper PRs. A fix higher in the stack is a dependency, not proof the
  earlier PR is fixed. Verify that fix and its tests, record its PR/SHA, and do
  not duplicate it or resolve the lower thread as fixed on its own head.

## Repair from base upward

1. Work in batches of about 10 PRs, nearest trunk first, with full-stack context.
   Complete independent repairs while recording blocked PRs. Validate reviewer
   claims against actual behavior; do not blindly implement every suggestion.
   Dismiss a false positive only with specific code/test evidence. Escalate an
   unresolved product decision while continuing unrelated fixes.
2. Classify CI failures from logs. Fix PR-caused failures on the branch that
   introduced them. Compare against target-branch failures before changing code
   for an inherited break. Keep infrastructure, secret/permission, and external
   service failures explicit; do not weaken checks or broaden credentials.
   Rerun an unchanged failure once only when there is evidence of a transient
   failure or a repaired external prerequisite. Repeated identical failures
   need diagnosis or a blocker, not a rerun loop.
3. Preserve unrelated dirty files and other agents' commits. Inspect fresh
   `but diff` IDs and make the smallest complete change on the existing owning
   branch, with meaningful regression coverage and repository-required checks.
   Amend an appropriate unpublished commit; otherwise create a focused commit
   on that branch. Never invent a repair branch stacked above the issue.
4. Compare all local branch heads with the pre-repair snapshot, including
   descendants rewritten by updates or repairs without direct file edits.
   For each affected stack, select its topmost affected branch; pushing it
   publishes that branch and its ancestors, not its descendants. Check every
   included branch's destination and current remote head, then inspect scope
   with `but push <topmost-affected-branch> --dry-run`. Obtain authorization for
   any otherwise unrelated publication or history rewrite before including it.
   Consolidate fixes and push once per affected stack through that branch.
   Verify every affected PR's published head, including rewritten descendants.
   Do not overwrite newer remote work, bypass hooks, or skip force-push
   protections. Refresh IDs after mutations when the next operation needs them.

Repeat conflict repair after updates or mutations introduce new conflicts.
Being behind trunk alone is not a reason to publish equivalent rewritten history.

## Verify and finish

- Re-read every affected remote head after pushing and verify each fix at the
  published SHA with relevant checks. Attach PRs being updated with
  `mcp__codex_app__attach_artifact` when available.
- Resolving addressed review threads is a required part of fixing PRs. For each
  verified fix, reply briefly with the change and commit/test evidence, then
  resolve the thread through the forge. Explain evidence-backed false positives
  before resolving them too. Include human and automated reviews, including
  outdated unresolved threads. A push, passing check, reply, or outdated flag
  does not resolve a thread. Re-fetch threads to confirm resolution was saved.
  Leave unverified or blocked findings open and report their links and next
  action. If resolution fails or permissions are missing, report that blocker
  and carry the pending resolution into the follow-up.
- Check live CI, reviews and the complete ledger again for all scoped PRs.
  New heads invalidate old check conclusions. Repairs, local tests, published
  fixes, remote CI and resolved reviews are separate facts. Completion requires
  no unresolved workspace conflicts, no actionable findings, no addressed review
  threads left unresolved, and successful applicable checks on current heads;
  pending checks, inaccessible findings, dependencies and external failures
  remain explicit.
- Finish available repairs before an external wait. Follow the `but` skill's
  continuation procedure: reuse/create a thread heartbeat for pending CI/review
  verification, save repo, PRs, heads, ledger, conflict state, completed repairs
  and blockers, and
  recheck live state on resumption. Notify only for meaningful changes; pause
  once complete or waiting only for user input. Do not watch, sleep or repeatedly
  poll. Never claim a follow-up exists if scheduling failed.

Report conflict resolutions, fixes and commits, local validation, published
heads, review resolutions, and remaining conflicts/CI/findings with links. Leave
all PRs open.
