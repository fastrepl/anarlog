---
name: gitbutler-pr-sweep
description: Check every branch or pull request represented in the current GitButler workspace, create missing PRs, address Bugbot comments and CI failures, then merge clean PRs to main, delete remote branches, and refresh the GitButler workspace.
---

# GitButler PR Sweep

Use this when the user asks to check, fix, merge, or clean up all branches or
PRs currently represented by the GitButler workspace.

## Ground Rules

- Use the `but` skill for all version-control write operations when the current
  branch is `gitbutler/workspace`.
- Use GitHub tools or `gh` for PR metadata, review comments, checks, logs, and
  merges.
- Do not merge a PR until Bugbot comments, actionable review comments, merge
  conflicts, and failing required checks are resolved.
- Keep fixes scoped to the PR branch that owns the failure.
- Preserve unrelated user or workspace changes. Do not reset, discard, or
  force-push without explicit user approval.
- Prefer amending marginal fixes into the relevant PR commit when the change is
  small and clearly belongs there.

## Workflow

1. Inspect workspace state.
   - Run `but status --json`.
   - Identify all applied GitButler branches and their upstream branches.
   - Map each branch to its GitHub PR. Use `gh pr list`, `gh pr view`, or GitHub
     app tools as needed.
   - For branches without a PR, create one before triage. Push the branch with
     `but push` first if GitHub cannot see it yet, then use the repository's
     normal PR creation flow.

2. Triage every PR.
   - Fetch unresolved review threads and comments.
   - Look specifically for Bugbot comments and treat them as actionable until
     proven otherwise.
   - Inspect CI status for every PR, including required checks and failed logs.
   - Record the branch, PR number, failures, and planned fix before editing.

3. Fix one PR at a time.
   - Apply or switch focus to the owning GitButler branch using `but` commands.
   - Make the smallest code change that resolves the issue.
   - Run the narrowest relevant verification first, then broaden when the change
     touches shared code.
   - Format with `pnpm exec dprint fmt` after code edits.
   - Commit or amend with `but commit` or `but amend` using IDs from fresh
     `but status --json` / `but show --json` output.
   - Push with `but push` when local commits need to update the PR.

4. Re-check PR readiness.
   - Re-read unresolved review threads and Bugbot comments.
   - Re-run or wait for GitHub checks as needed.
   - Verify each PR is mergeable, up to date enough for project policy, and has
     no failing required checks.

5. Merge clean PRs to main.
   - Merge only PRs that passed the readiness checks.
   - Use the repository's normal GitHub merge method unless the user specifies
     otherwise.
   - If PRs are stacked, merge from the base of the stack upward and re-check
     downstream PRs after each merge.

6. Clean up merged branches.
   - Delete merged remote branches after confirming the PR merge succeeded.
   - Do not delete branches for unmerged, blocked, or failed PRs.

7. Refresh the workspace.
   - Pull or update the GitButler workspace with `but pull --check --json`,
     followed by `but pull --json --status-after` when appropriate.
   - Run `but status --json` and report any remaining branches, uncommitted
     changes, conflicts, or blocked PRs.

## Reporting

Keep the final report concise:

- PRs merged.
- Branches deleted.
- Bugbot comments or CI failures fixed.
- Verification commands run.
- Any PRs left blocked and the concrete reason.
