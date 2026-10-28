---
name: anarlog-workflow
description: Execute Anarlog work immediately while preserving useful decisions and non-obvious lessons in Linear. Use for Anarlog repository or Anarlog desktop, web, mobile, and API work, including related worktrees and ANLG issues. Explicit brainstorming stays discussion-first. Do not use for unrelated repositories or meeting-data queries.
---

# Anarlog workflow

Apply to Anarlog repository or Anarlog desktop, web, mobile, and API work. Apply the same workflow in other checkouts and worktrees.

Use Linear issues for work details, decisions, and substantive updates. Keep full implementation and verification evidence in commits, PRs, and CI, and link it from the issue. Follow [review-intent](../review-intent/SKILL.md) whenever writing commits or PRs.

Write everything you log or edit in English, even when the conversation is in another language: Linear issues, comments, and documents; Slack updates; commits, PRs, and review replies; code comments and docs. Only replies to the user in the conversation follow the user's language, and localized product content such as translation catalogs keeps its target language.

Do not read, create, edit, or comment in Notion for Anarlog work. Anarlog does not use Notion; keep its decisions and durable context in Linear.

- Team: **Anarlog** (`ANLG`).
- Workspace: [fastrepl-inc](https://linear.app/fastrepl-inc).
- [Agent lessons](https://linear.app/fastrepl-inc/document/agent-lessons-45018045d01e).

## Start work immediately

1. Default to execution. When the user requests a change, reports a bug, or scopes work, start investigating and implementing immediately and carry it through to completion. Document only when the criteria below are met; documentation is not a prerequisite or the deliverable.
2. Only use a discussion-first workflow when the user explicitly asks for brainstorming, exploration of ideas, or planning without implementation. Answer informational questions directly.
3. Search Linear issues, documents, and comments for the topic, IDs (`ANLG-123`), and nearby decisions alongside execution. Read matching issues/docs and the team's **Agent lessons** early enough to inform relevant implementation decisions. Do not make this pass a prerequisite for starting useful work.
4. Before recording work details, search for an existing issue by topic, symptom, component, and known IDs, including completed issues. Read matching descriptions, recent comments, status, and relations. Reuse a matching active issue; if none covers this work, create one in the right team and relevant existing project. Follow the completed-issue rule below. Do not create tickets for routine execution logs.
5. If Linear is unavailable, continue authorized work and report a recording gap only when there is useful context to preserve. Existing approval requirements for external actions and shared history still apply.

## Reuse active issues; keep completed work closed

- A completed issue is historical context, not the container for new work. Never
  reopen it or silently expand its scope to track a recurrence, regression, or
  follow-up. Search for an active follow-up first; if none fits, create a new
  issue describing what is different now and add the completed issue using
  Linear's **related** relationship. Preserve existing relations.
- Keep canceled and duplicate issues closed too; follow a duplicate's canonical
  link before choosing the active issue. Do not create a replacement for work
  already completed unless there is actual new work to track.
- Record the current context, intended outcome, and direction on the active
  issue. Leave concise updates for material decisions, changed scope, findings,
  blockers, and outcomes. Link the PR or evidence and state any remaining work;
  avoid copying full diffs, command logs, or every intermediate action.
- Read recent comments before posting so reruns do not repeat an update. Update
  status only to match the issue's actual acceptance criteria; a commit or green
  CI alone does not prove deployment or user acceptance.

## Record facts worth remembering

Before writing a comment or document, ask: will this help someone make a future decision or avoid rediscovering something non-obvious, and is it missing from the existing record? If not, do not post.

- Preserve consequential decisions and their rationale, surprising findings, non-obvious constraints, corrected assumptions, and failed approaches with a reusable lesson.
- Record a blocker or handoff only when someone needs to act, including the missing context and required next step.
- Skip investigation-start announcements, step-by-step progress, standalone commit/push notices, test counts, and formatting results. Summarize meaningful outcomes and remaining validation on the issue; keep detailed logs in the PR or CI.
- Do not copy information already captured in an issue, document, or PR. Link to the source when it supports a new fact; include only the context needed to understand that fact.
- Keep each note short: what we learned, why it matters, and what it changes next time. No required headings or per-turn documentation quota. If nothing worth remembering emerged, write nothing.
- Link the issue in commits/PRs when one exists (`ANLG-123`).

## Put knowledge where it belongs

- Work details, findings, decisions, and outcomes: a concise update on the matching active issue, creating one after searching if needed. Link completed predecessors as related; do not revive them.
- Lessons reusable across tickets: the team's **Agent lessons** document (create it if missing). Check for an existing entry and update it rather than repeating it. Use a date, short title, lesson, and future action; newest first.
- Durable specs, research conclusions, and product context: the relevant Linear team or project document, only when there is substantive context to preserve.
- Use one appropriate home for each fact. Do not duplicate it across an issue comment and a lessons document, or post a separate announcement that it was documented.

## Keep Slack updates selective

- Post only when the user has requested a Slack update or an explicitly invoked workflow authorizes it. This skill does not authorize unsolicited messages.
- For authorized updates, ask whether the channel's audience needs to know or act now: a consequential outcome, a decision needing input, or a blocker requiring help. A fact worth storing is not automatically worth broadcasting.
- Skip routine progress, test results, commit/PR notices already covered by integrations, and announcements that a Linear note or document was updated. If nothing warrants the audience's attention, stay quiet unless the user explicitly requested that specific update.
- Consolidate related outcomes into one short message: what changed for the audience, any action needed, and a source link. Continue an existing thread for follow-ups; avoid repeated top-level posts or cross-posting without a distinct audience need.
- Account for indirect noise: Linear comments may already appear in Slack through an integration. Apply the same documentation threshold before writing them, and do not send a second Slack copy.
- Notification routing and subscription settings are separate from agent writing behavior. Do not claim notifications are suppressed unless those settings were actually changed and verified.
