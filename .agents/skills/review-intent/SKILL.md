---
name: review-intent
description: Required for creating or updating Anarlog PRs and creating, amending, squashing, or rewording commits. Explain PR context, intention, and direction; record concrete actions in commits; connect the work to the correct ANLG Linear issue. Applies on every branch in this repository.
---

# Anarlog review intent

The PR describes the intended displacement from the old behavior to the new
behavior. Each commit describes an action contributing to that displacement.
Give AI and human reviewers enough context to assess whether the actions add up
to the intended outcome and identify the remaining gap. Review does not replace
the author's verification or decide product intent on the author's behalf.

Use this skill before composing or changing a commit message or PR title/body.
Follow [anarlog-workflow](../anarlog-workflow/SKILL.md) for Linear records. This skill
sets the writing contract; it does not authorize pushing, publishing, merging,
or rewriting shared history. Use the repo's GitButler skill on
`gitbutler/workspace`, including its branch ownership and selected-hunk rules.

## Gather the actual change

- Read the request, the matching Linear issue and relevant decisions, then the
  complete scoped diff. Separate observed behavior from hypotheses; do not invent
  a motivation from filenames or an existing commit title.
- For a commit, inspect exactly the selected changes. For a PR, inspect every
  commit and the full diff against its actual base. In a stack, describe this
  branch's contribution and name any material dependency.
- For an existing PR, describe its published head. Local-only changes are not
  part of that PR. Keep validation tied to the version and environment checked.

## Write the PR's context, intention, and direction

Use a short title stating the intended outcome. In the repository PR template's
Summary, write these three distinct parts, normally one or two sentences each:

- **Context:** What happened before? Describe the existing behavior, trigger,
  or constraint and who or what it affected.
- **Intention:** Why was that off? Explain the mismatch and the outcome the
  change should achieve. This is the reason, not a list of edited files.
- **Direction:** How should it change? Explain the chosen approach and expected
  resulting behavior, including a material tradeoff or boundary when needed.

Keep Demo and Verification sections required by the template. Report actual
checks and meaningful gaps; distinguish local checks, CI, hosted behavior, and
release status. Link the active ANLG issue, and label completed predecessors as
related history rather than using closing keywords for them.

Do not paste a commit log, file inventory, or conversation transcript. Scale the
writing to the change. Preserve useful reviewer notes and GitButler stack
metadata when updating a PR. If the scope changes, rewrite all three parts
around the final change rather than appending a diary of fixes.

## Write each commit's actions

- Give the title a concrete action and its intended effect, such as "Preserve
  reply receipts across reloads" rather than "Fix review comments".
- In a short body, describe the actions actually taken and notable changes to
  behavior or structure. Include the reason only where it explains an action.
  Use a short paragraph or a few bullets; do not repeat the PR's three sections.
- Keep each commit coherent and include its relevant regression coverage. Each
  action should contribute to the PR's direction; split unrelated work.
- Link the active issue (ANLG-123) when one exists. Do not claim unrun checks,
  deployment, or user acceptance. Never add agent attribution or co-author trailers.
- Commit a completed working checkpoint. Amend a marginal follow-up into the
  relevant unpublished local commit when ownership and intent are clear. Do not
  rewrite pushed, reviewed, shared, or another agent's history without permission.
  Updating a PR description alone does not authorize rewording its commits.

## Anarlog example

Illustrative only; these are not claims about current implementation or checks.

**Context:** Closing a tab could race with a remote note update.
**Intention:** Closing a view should never erase a newer meeting note.
**Direction:** Keep tab lifecycle separate from document persistence and reject saves based on an outdated document.

Example commit:

```text
Preserve newer notes when a tab closes

Remove document writes from tab cleanup, guard saves with document identity,
and cover a remote update arriving before the tab closes.
```

## Before saving

Read the PR and commit messages together: can a reviewer explain the original
situation, why it needed changing, the chosen direction, and how each commit
moves toward it? Resolve mismatches between prose and diff before saving. Keep
review feedback focused on correctness, missing cases, and that intended outcome;
record a new product decision explicitly if the direction itself changes.
