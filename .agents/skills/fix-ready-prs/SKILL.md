---
name: fix-ready-prs
description: Inspect every open non-draft PR for CI failures and unresolved Cursor Bugbot findings, then fix them on the existing PR branches. Use when asked to check active PRs, triage ready PRs, fix CI, or address Bugbot comments.
metadata:
  internal: true
---

# Fix Ready PRs

Work through every **open, non-draft** pull request. Fix CI failures and
unresolved Bugbot findings on the existing PR branch. Do not open a new PR for
an existing one.

Drafts are out of scope unless the user names them.

## 1. Inventory

```bash
gh pr list --state open --limit 200 \
  --json number,title,isDraft,headRefName,url,mergeStateStatus
```

Keep only `isDraft == false`. Record number, branch, and URL.

## 2. Collect failures

For each ready PR, gather **current-head** CI and **unresolved** Bugbot threads.
Do not treat an in-progress check as green.

### CI

```bash
gh pr checks <n>
gh pr view <n> --json statusCheckRollup,headRefOid
```

Treat as a failure when a required or repo workflow check is `FAILURE`,
`TIMED_OUT`, `ACTION_REQUIRED`, or `CANCELLED` on the current head and has not
been superseded by a newer run.

Ignore:

- `SKIPPED` / `NEUTRAL`
- Netlify header/pages/redirect checks and canceled deploy previews
- Mintlify Deployment skips
- macOS/Windows/iOS jobs that this repo skips on pull requests

If a failing job has logs, pull them:

```bash
gh run view <run-id> --log-failed
```

If checks are still `IN_PROGRESS` / `pending`, wait until they finish or fail
before declaring the PR clean. Re-poll rather than guessing.

### Bugbot

Use review threads, not only the Bugbot check conclusion. A green Bugbot check
can still leave unresolved comments, and an older finding may already be
fixed.

```bash
gh api graphql -f query='
  query($n:Int!) {
    repository(owner:"fastrepl", name:"anarlog") {
      pullRequest(number:$n) {
        reviewThreads(first:50) {
          nodes {
            isResolved
            isOutdated
            comments(first:5) {
              nodes { author { login } path line body }
            }
          }
        }
      }
    }
  }' -F n=<n>
```

Fix a thread when:

- the author is `cursor` / `cursor[bot]`
- `isResolved` is false
- the comment is a Bugbot finding (`<!-- BUGBOT_BUG_ID:` or a severity heading)
- it still applies to the current head (`isOutdated` is false, or outdated but
  the same bug is still in the code)

Skip resolved threads and outdated findings whose code already changed.

Also read `gh pr view <n> --comments` and `gh api repos/fastrepl/anarlog/pulls/<n>/comments`
in case Bugbot posted a review comment that is not yet a thread.

## 3. Fix on the PR branch

For each PR that has work:

1. `git fetch origin <headRefName>`
2. `git checkout -B <headRefName> origin/<headRefName>`
3. Reproduce from the failing log or Bugbot location
4. Make the smallest change that fixes the failure or finding
5. Run the locally available checks from the CI workflows that the changed
   paths trigger (`AGENTS.md` pre-commit verification)
6. Commit on that branch (do not amend someone else's commit)
7. `git push origin <headRefName>`

Do not bundle unrelated PR fixes onto one branch. Do not retarget or close PRs.

If a finding is a false positive, leave it. Do not resolve GitHub review
threads unless the user asked; the code change is the fix.

## 4. Recheck

After pushing, re-run the inventory for that PR. Confirm:

- new CI is running or green
- the previous failure is gone or replaced by a new run
- the Bugbot thread is outdated or will be re-reviewed on the new commit

If a new failure or finding appears, fix it too.

## 5. Report

Summarize every ready PR:

- clean (CI green, no unresolved Bugbot)
- waiting (checks still running)
- fixed (what changed, PR number)
- skipped draft

Do not merge.
