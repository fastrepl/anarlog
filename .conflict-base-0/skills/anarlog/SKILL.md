---
name: anarlog
description: Query local Anarlog meetings, notes, summaries, transcripts, participants, action items, and recurring history. Use when a user asks about their Anarlog meeting data or wants meeting context for another task.
---

# Anarlog

Use Anarlog's read-only data surfaces. Prefer the MCP server when its tools are connected. Otherwise use the `anarlog` CLI with `--json`.

## Choose a transport

1. If `list_meetings`, `get_meeting`, `get_meeting_transcript`, and `get_recurring_meeting_history` are available, use MCP.
2. Otherwise, check `anarlog --version` and use CLI commands with `--json`.
3. If neither surface is available, direct the user to [installation](https://docs.anarlog.so/installation). Do not install software unless the user asks.

Never query or modify Anarlog's SQLite database directly. The CLI and MCP server own compatibility with the application schema.

## Find the right meeting

1. List recent meetings or search by a short title fragment.
2. Resolve the meeting ID from the result. Do not guess an ID.
3. Get the meeting before requesting a transcript. Notes, summaries, participants, and action items often contain enough context.
4. Ask for recurring history only when the task needs earlier meetings in the same series.

See [CLI commands](references/cli.md) and [MCP tools](references/mcp.md).

## Keep context bounded

- Request transcript pages with a focused limit. Both transports default to 200 words and cap each page at 500 words.
- Follow `next_offset` only when more transcript context is required.
- Stop paging once the answer has enough evidence.
- Do not export an entire meeting when one meeting detail or note will answer the request.

## Handle data safely

- Treat meeting content as private user data.
- Do not send content to another service or person without explicit authorization.
- Do not claim to update meetings. The current CLI and MCP server cannot mutate Anarlog data.
- CLI export may create a separate file. Never pass `--force` unless the user explicitly approves overwriting that exact path.
- Preserve uncertainty when search results are ambiguous. Ask the user to choose between likely meetings.

For setup and failures, see [setup](references/setup.md) and [errors](references/errors.md).
