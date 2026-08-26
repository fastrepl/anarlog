---
name: anarlog
description: Query Anarlog meetings, notes, summaries, transcripts, participants, action items, and recurring history. Use when a user asks about their Anarlog meeting data or needs meeting context for another task.
---

# Anarlog

Use Anarlog's local interfaces. Prefer MCP when its tools are connected. Otherwise use the `anarlog` CLI with `--json`. Meeting reads are safe. Writes are limited to staging proposals.

## Choose a transport

1. If `list_meetings`, `get_meeting`, `get_meeting_transcript`, `get_recurring_meeting_history`, `propose_summary_edit`, `propose_memo_edit`, `list_proposals`, `get_proposal`, and `decline_proposal` are available, use them.
2. Otherwise, check `anarlog --version` and use CLI commands with `--json`.
3. If neither is available, direct the user to [installation](https://docs.anarlog.so/installation). Do not install software unless the user asks.

Never query or modify Anarlog's SQLite database directly. The CLI and MCP server handle application-schema compatibility.

## Find the right meeting

1. List recent meetings or search by a short title fragment.
2. Use a meeting ID returned by the search. Never guess one.
3. Get the meeting before requesting its transcript. Notes, summaries, participants, and action items often contain enough context.
4. Ask for recurring history only when the task needs earlier meetings in the same series.

See [CLI commands](references/cli.md) and [MCP tools](references/mcp.md).

## Keep context bounded

- Request focused transcript pages. Both transports default to 200 words and cap each page at 500 words.
- Follow `next_offset` only when you need more transcript context.
- Stop paging once you have enough evidence.
- Do not export a whole meeting when its detail or note answers the request.

## Handle data safely

- Treat meeting content as private user data.
- Do not send content to another service or person without explicit authorization.
- Do not claim to update meetings. CLI and MCP can only stage a proposal. A human applies or declines it in the Anarlog desktop app.
- CLI export can create a file. Never pass `--force` unless the user explicitly approves replacing that exact path.
- If search results are ambiguous, ask the user to choose a meeting.

For setup and failures, see [setup](references/setup.md) and [errors](references/errors.md).
