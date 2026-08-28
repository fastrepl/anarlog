---
name: anarlog
description: Query Anarlog meetings, notes, summaries, transcripts, participants, action items, and recurring history. Use when a user asks about their Anarlog meeting data or needs meeting context for another task.
---

# Anarlog

Use hosted Cloud MCP as the default. Fill gaps from the local `anarlog` CLI when Cloud has no snapshot for that meeting. Meeting reads are safe. Writes are limited to staging proposals on the local CLI.

## Choose a source

1. If Cloud MCP tools are connected, call them first: `list_meetings`, `get_meeting`, `get_meeting_transcript`, and `get_recurring_meeting_history`.
2. If Cloud returns no match, snapshots are disabled, or the user is asking about a meeting that only exists on this machine, use `anarlog --json` against the local database.
3. Use a local `anarlog mcp` stdio server only to fill those same gaps. Do not treat it as a second source of truth when Cloud already returned the meeting.
4. If neither Cloud nor the local CLI is available, direct the user to enable **Cloud API & Connectors** and [installation](https://docs.anarlog.so/installation). Do not install software unless the user asks.

Never query or modify Anarlog's SQLite database directly. The CLI and MCP servers handle application-schema compatibility.

## Find the right meeting

1. List recent meetings or search by a short title fragment.
2. Use a meeting ID returned by the search. Never guess one.
3. Get the meeting before requesting its transcript. Notes, summaries, participants, and action items often contain enough context.
4. Ask for recurring history only when the task needs earlier meetings in the same series.
5. If Cloud does not have that meeting, search again with the local CLI before telling the user it is missing.

See [CLI commands](references/cli.md) and [MCP tools](references/mcp.md).

## Ground answers in tool output

- Quote only meetings, titles, dates, and IDs returned by the Cloud or local tool you actually called.
- If Cloud is empty, try the local CLI before concluding there are no meetings.
- Never invent meetings from the repo, chat, or similar-looking names. A host showing that a tool ran is not proof of the titles you then write.
- Name whether the data came from Cloud or the local database.

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
