---
name: anarlog
description: Query Anarlog meetings, notes, summaries, transcripts, participants, action items, and recurring history. Use when a user asks about their Anarlog meeting data or needs meeting context for another task.
---

# Anarlog

Prefer local Anarlog data when the agent can access it. Use OAuth-connected Cloud MCP when running remotely or when local data is unavailable. Meeting reads are safe. Writes are limited to staging proposals on the local CLI.

## Choose a source

1. Honor an explicit local-only or Cloud request. Do not silently switch its source.
2. Otherwise, prefer a connected local MCP server. If shell access is available, select the CLI executable available on `PATH`: `anarlog-cli` on Flatpak, otherwise `anarlog`. Use that executable for every CLI command below, including the `--json doctor` readiness check. Read with the selected executable and `--json meetings --source local list` when ready, even if Cloud MCP is connected.
3. When running remotely without the user's local database, or when the CLI or database is absent, use connected Cloud MCP: `list_meetings`, `get_meeting`, `get_meeting_transcript`, and `get_recurring_meeting_history`. If MCP is unavailable but CLI login is available, use the selected executable with `--json meetings --source cloud list` for the same hosted snapshots.
4. A local database compatibility, permission, or operation error needs to be reported; do not hide it by switching to Cloud. An empty local result does not mean the database is unavailable. If the requested meeting is missing, an already-connected alternate source can fill the gap unless the user restricted the source. Identify that source and keep the meeting's subsequent reads on it.
5. Local reads need no Cloud login, Pro subscription, or completed sync. If neither source is available, offer [local CLI installation](references/setup.md) for an agent on the user's computer, or **Cloud API & Connectors** plus OAuth for remote access. Do not install software or enable Cloud uploads unless the user asks.

Never query or modify Anarlog's SQLite database directly. The CLI and MCP servers handle application-schema compatibility.

## Find the right meeting

1. List recent meetings or search by a short title fragment. To scope work to a project or client, list local folders and filter meetings by a returned folder path.
2. Use a meeting ID returned by the search. Never guess one.
3. Get the meeting before requesting its transcript. Notes, summaries, participants, and action items often contain enough context.
4. Ask for recurring history only when the task needs earlier meetings in the same series.
5. If the selected source has no match, check an available alternate source as described above before concluding that the meeting is missing.

See [CLI commands](references/cli.md) and [MCP tools](references/mcp.md).

## Ground answers in tool output

- Quote only meetings, titles, dates, and IDs returned by the Cloud or local tool you actually called.
- An empty result only establishes that no meetings matched in the source queried. Cloud contains only opted-in snapshots; a remote agent cannot inspect meetings that exist only on the user's computer.
- Never invent meetings from the repo, chat, or similar-looking names. A host showing that a tool ran is not proof of the titles you then write.
- Name whether the data came from Cloud or the local database. Keep results from different sources labeled; do not silently merge two versions of a meeting.

## Report freshness accurately

- Local reads include unsynced changes. Do not block them while Cloud Sync is behind or offline.
- Cloud Sync carries encrypted data. Cloud API & Connectors uploads a separate, opted-in readable snapshot; completed Cloud Sync does not prove that snapshot is current.
- Report sync status or snapshot freshness only when a tool returns it. A meeting's `updated_at` is not a last-sync or upload timestamp. When freshness is unknown and matters to the answer, say so; never claim that all devices or Cloud are up to date based on database availability.

## Keep context bounded

- Request focused transcript pages. Both transports default to 200 words and cap each page at 500 words.
- Follow `next_offset` only when you need more transcript context.
- Stop paging once you have enough evidence.
- Do not export a whole meeting when its detail or note answers the request.

## Handle data safely

- Treat meeting content as private user data.
- Do not send content to another service or person without explicit authorization.
- Cloud MCP is read-only. To stage an edit, use the local CLI or local MCP proposal tools. A human applies or declines it in the Anarlog desktop app.
- CLI export can create a file. Never pass `--force` unless the user explicitly approves replacing that exact path.
- If search results are ambiguous, ask the user to choose a meeting.

For setup and failures, see [setup](references/setup.md) and [errors](references/errors.md).
