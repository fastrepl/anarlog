# MCP tools and resources

Read tools are idempotent. Proposal tools insert or decline staged edits; they never apply those edits to the meeting.

| Tool                            | Use                                                                                                     |
| ------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `list_meetings`                 | Find recent meetings by title, ID fragment, or recurring series.                                        |
| `get_meeting`                   | Read metadata, canonical note, summaries, participants, and action items.                               |
| `get_meeting_transcript`        | Read a transcript page. Start with `limit: 200`; continue from `pagination.next_offset` only as needed. |
| `get_recurring_meeting_history` | Find meetings from the same recurring series as a known meeting.                                        |
| `propose_summary_edit`          | Stage a complete summary replacement. Pass `target_id` when multiple summaries exist.                   |
| `propose_memo_edit`             | Stage a complete memo replacement.                                                                      |
| `list_proposals`                | List staged proposals. Defaults to `status: pending`.                                                   |
| `get_proposal`                  | Read one proposal and its unified `diff`.                                                               |
| `decline_proposal`              | Discard a pending proposal without changing the meeting.                                                |

Transcript limits are measured in words. The default is 200 and the maximum is 500.

Available resources:

- `anarlog://meetings/{meeting_id}`
- `anarlog://meetings/{meeting_id}/transcript{?offset,limit}`
- `anarlog://series/{series_id}`

Prefer tools when the workflow needs structured JSON. Use resources when the client needs concise Markdown or plain-text context.
