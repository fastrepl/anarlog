---
name: anarlog-workflow
description: Execute Anarlog work immediately while recording issues, decisions, progress, and lessons in Linear. Use for Anarlog repository or Anarlog desktop, web, mobile, and API work, including related worktrees and ANLG issues. Explicit brainstorming stays discussion-first. Do not use for unrelated repositories or meeting-data queries.
---

# Anarlog workflow

Apply to Anarlog repository or Anarlog desktop, web, mobile, and API work. Apply the same workflow in other checkouts and worktrees.

Linear is the system of record. Chat and the repo are not the archive.

- Team: **Anarlog** (`ANLG`).
- Workspace: [fastrepl-inc](https://linear.app/fastrepl-inc).
- [Agent lessons](https://linear.app/fastrepl-inc/document/agent-lessons-45018045d01e).

## Start work immediately

1. Default to execution. When the user requests a change, reports a bug, or scopes work, start investigating and implementing immediately and carry it through to completion. Recording it in Linear is part of the work, not the deliverable or a reason to stop.
2. Only use a discussion-first workflow when the user explicitly asks for brainstorming, exploration of ideas, or planning without implementation. Answer informational questions directly.
3. Search Linear issues, documents, and comments for the topic, IDs (`ANLG-123`), and nearby decisions alongside execution. Read matching issues/docs and the team's **Agent lessons** early enough to inform relevant implementation decisions. Do not make this pass a prerequisite for starting useful work.
4. Reuse an existing issue when one fits. Create one only when nothing covers the work, on the right team, attached to the existing Linear project when one already tracks that surface. Keep the issue current as work proceeds; do not stop after creating or updating it.
5. If Linear is unavailable, continue authorized work and report the recording gap. Existing approval requirements for external actions and shared history still apply.

## Record everything

- Work lives on a Linear issue. If the user states a decision, files a bug, or scopes new work, write it to Linear in the same turn.
- Progress, decisions, blockers, and handoffs go on that issue as comments.
- Durable specs, research, and product context go in Linear documents on the team or project.
- Link the issue in commits/PRs when one exists (`ANLG-123`).

## Record lessons as you go

A lesson is anything the next agent would otherwise rediscover: a failed approach, a non-obvious constraint, an architectural decision, a prod/debug gotcha, or a corrected assumption.

- Write it immediately. Do not wait for a wrap-up.
- Ticket-specific: comment on the issue under `## Lesson`.
- Reusable across tickets: append to that team's **Agent lessons** document (create it if missing). Newest first: date, one-line title, what we learned, what to do next time.
- Do not dump routine status into lessons. Do not leave important context only in the chat.
