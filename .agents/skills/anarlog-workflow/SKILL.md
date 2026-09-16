---
name: anarlog-workflow
description: Execute Anarlog work immediately while preserving useful decisions and non-obvious lessons in Linear. Use for Anarlog repository or Anarlog desktop, web, mobile, and API work, including related worktrees and ANLG issues. Explicit brainstorming stays discussion-first. Do not use for unrelated repositories or meeting-data queries.
---

# Anarlog workflow

Apply to Anarlog repository or Anarlog desktop, web, mobile, and API work. Apply the same workflow in other checkouts and worktrees.

Use Linear for work tracking and facts worth remembering. Keep implementation and verification details in commits, PRs, and CI.

- Team: **Anarlog** (`ANLG`).
- Workspace: [fastrepl-inc](https://linear.app/fastrepl-inc).
- [Agent lessons](https://linear.app/fastrepl-inc/document/agent-lessons-45018045d01e).

## Start work immediately

1. Default to execution. When the user requests a change, reports a bug, or scopes work, start investigating and implementing immediately and carry it through to completion. Document only when the criteria below are met; documentation is not a prerequisite or the deliverable.
2. Only use a discussion-first workflow when the user explicitly asks for brainstorming, exploration of ideas, or planning without implementation. Answer informational questions directly.
3. Search Linear issues, documents, and comments for the topic, IDs (`ANLG-123`), and nearby decisions alongside execution. Read matching issues/docs and the team's **Agent lessons** early enough to inform relevant implementation decisions. Do not make this pass a prerequisite for starting useful work.
4. Reuse an existing issue when one fits. Create one for work that needs tracking, such as a feature, substantial behavior change, or unresolved bug; do not create an issue just to log a small completed edit. Use the right team and existing project. Update status when it changes without adding a routine progress comment.
5. If Linear is unavailable, continue authorized work and report a recording gap only when there is useful context to preserve. Existing approval requirements for external actions and shared history still apply.

## Record facts worth remembering

Before writing a comment or document, ask: will this help someone make a future decision or avoid rediscovering something non-obvious, and is it missing from the existing record? If not, do not post.

- Preserve consequential decisions and their rationale, surprising findings, non-obvious constraints, corrected assumptions, and failed approaches with a reusable lesson.
- Record a blocker or handoff only when someone needs to act, including the missing context and required next step.
- Skip investigation-start announcements, step-by-step progress, change summaries, commit/push notices, test counts, formatting results, and routine verification caveats. These belong in the task response, PR, or CI, not another Linear comment.
- Do not copy information already captured in an issue, document, or PR. Link to the source when it supports a new fact; include only the context needed to understand that fact.
- Keep each note short: what we learned, why it matters, and what it changes next time. No required headings or per-turn documentation quota. If nothing worth remembering emerged, write nothing.
- Link the issue in commits/PRs when one exists (`ANLG-123`).

## Put knowledge where it belongs

- Ticket-specific findings or decisions: a concise comment on the existing issue.
- Lessons reusable across tickets: the team's **Agent lessons** document (create it if missing). Check for an existing entry and update it rather than repeating it. Use a date, short title, lesson, and future action; newest first.
- Durable specs, research conclusions, and product context: the relevant team or project document, only when there is substantive context to preserve.
- Use one appropriate home for each fact. Do not duplicate it across an issue comment and a lessons document, or post a separate announcement that it was documented.

## Keep Slack updates selective

- Post only when the user has requested a Slack update or an explicitly invoked workflow authorizes it. This skill does not authorize unsolicited messages.
- For authorized updates, ask whether the channel's audience needs to know or act now: a consequential outcome, a decision needing input, or a blocker requiring help. A fact worth storing is not automatically worth broadcasting.
- Skip routine progress, test results, commit/PR notices already covered by integrations, and announcements that a Linear note or document was updated. If nothing warrants the audience's attention, stay quiet unless the user explicitly requested that specific update.
- Consolidate related outcomes into one short message: what changed for the audience, any action needed, and a source link. Continue an existing thread for follow-ups; avoid repeated top-level posts or cross-posting without a distinct audience need.
- Account for indirect noise: Linear comments may already appear in Slack through an integration. Apply the same documentation threshold before writing them, and do not send a second Slack copy.
- Notification routing and subscription settings are separate from agent writing behavior. Do not claim notifications are suppressed unless those settings were actually changed and verified.
