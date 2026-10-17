# MCP tools and resources

Cloud MCP is read-only. Local MCP adds `list_folders` and proposal tools, which insert or decline staged edits; they never apply those edits to the meeting.

| Tool                            | Use                                                                                                                        |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `list_meetings`                 | Find recent meetings by title, ID fragment, or recurring series. An empty list means none matched; do not invent meetings. |
| `list_folders`                  | List local meeting folder paths. Pass a returned `path` as `list_meetings` `folder_path` to include its subfolders.        |
| `get_meeting`                   | Read metadata, canonical note, summaries, participants, and action items.                                                  |
| `get_meeting_transcript`        | Read a transcript page. Start with `limit: 200`; continue from `pagination.next_offset` only as needed.                    |
| `get_recurring_meeting_history` | Find meetings from the same recurring series as a known meeting.                                                           |
| `export_meeting`                | Read the complete meeting record including transcripts. Use only when smaller reads are insufficient.                      |
| `propose_summary_edit`          | Stage a complete summary replacement. Pass `target_id` when multiple summaries exist.                                      |
| `propose_memo_edit`             | Stage a complete memo replacement.                                                                                         |
| `list_proposals`                | List staged proposals. Defaults to `status: pending`.                                                                      |
| `get_proposal`                  | Read one proposal and its unified `diff`.                                                                                  |
| `decline_proposal`              | Discard a pending proposal without changing the meeting. Applied proposals cannot be declined.                             |

Local meeting objects include `folder_path`; it is `null` for meetings outside a folder.

Meeting exports can include optional `speakers` and `speaker_context` fields for transcript rendering. Older or trimmed Cloud snapshots may omit them; do not infer a speaker name when the export lacks evidence.

Transcript limits are measured in words. The default is 200 and the maximum is 500.

Available resources:

- `anarlog://meetings/{meeting_id}`
- `anarlog://meetings/{meeting_id}/transcript{?offset,limit}`
- `anarlog://series/{series_id}`

Prefer tools when the workflow needs structured JSON. Use resources when the client needs concise Markdown or plain-text context. Transcript resources default to 200 words (cap 500) and return plain text only.

## Local recording import

For an explicitly requested audio import, use local `import_recording` with `file_path`, optional `title`, an existing `folder_path`, and a stable `request_key` for submission retries. The updated desktop app must be open on the same machine and database. It creates a real meeting and runs configured transcription and summary providers, which may incur charges and receive audio. Hosted MCP cannot read a local file path or import it.

Persist `meeting_id` and `job_id`, then poll `get_recording_import` with `job_id`. Do not equate submission with completion. Check both `processing_status` and `summary_status`; `skipped_no_model` and `skipped_too_short` mean no summary was generated. Failure preserves completed meeting data. Reusing a request key returns the existing job, even after failure; do not silently resubmit with a new key. Local `get_meeting` includes the same status in `recording_import`.
