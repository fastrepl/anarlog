# db-app Spec

## Core Model

Task is the core entity. Daily is the organizing unit.

```
daily (one per day)
 └── tasks (many per daily)
      ├── type=todo      → plain checkbox
      ├── type=meeting   → has session artifacts (transcript, summaries, participants)
      ├── type=gmail     → linked to Gmail thread
      ├── type=linear    → linked to Linear issue
      ├── type=github    → linked to GitHub issue
      └── type=agent     → background AI task
```

Every editor checkbox = a task row. When a user types `- [ ] buy milk`, that immediately creates a task row.

Sessions (meetings) are artifacts of meeting-type tasks, not standalone entities.

## Editor Relationship

ProseMirror/TipTap is a thin view layer. Task nodes reference DB rows via `taskId` attr. The task row is canonical for metadata (status, title, source); the editor is a projection.

Task deletion in the editor is a domain command, not character-by-character text editing. Behavior varies by type:
- `todo` → delete row, remove node
- `agent` (running) → prompt to cancel
- `linear` / `gmail` → alert, remove from today's list only
- `meeting` (with artifacts) → warn about artifacts

## Tables

Common columns omitted from listings: `id TEXT PK`, `created_at`, `user_id`, `visibility` appear on most synced tables.

### Daily & Tasks

**daily** — One row per user per day. The organizing unit for tasks.
Columns: date, body (TipTap JSON), updated_at
Unique: (date, user_id)

**tasks** — Core entity. Every checkbox, meeting, integration item is a task row.
Columns: daily_id, parent_task_id, sort_key, type (todo|meeting|gmail|linear|github|agent), title, status, source_id, source_url, event_id, metadata_json, updated_at, updated_by
FK: daily_id → daily, parent_task_id → tasks, event_id → events
Unique: event_id (where not null — one task per event)

### Task Children

**task_notes** — Memos, agent notes, imported snippets. Append-only with soft delete.
Columns: task_id, author_type, author_id, body, deleted_at
FK: task_id → tasks

**task_events** — Audit trail. Who changed what, why.
Columns: task_id, actor_type (user|agent|sync), actor_id, event_type, payload_json
FK: task_id → tasks

**task_participants** — People associated with a task.
Columns: task_id, human_id, source
FK: task_id → tasks, human_id → humans
Unique: (task_id, human_id)

**meeting_artifacts** — Transcript and note body for meeting-type tasks. One per task.
Columns: task_id, transcript_md, note_body, updated_at
FK: task_id → tasks
Unique: task_id

**meeting_summaries** — Templated summary sections for a meeting task.
Columns: task_id, template_id, content, position, title, updated_at
FK: task_id → tasks

**task_words** — Transcript words scoped to a task.
Columns: task_id, text, start_ms, end_ms, channel, state
FK: task_id → tasks

**task_speaker_hints** — Speaker diarization hints for task transcript.
Columns: task_id, word_id, kind, speaker_index, provider, channel, human_id
FK: task_id → tasks, word_id → task_words

### Calendar & Events

**connections** — OAuth/API connections to external providers.
Columns: provider_type, provider_id, base_url, api_key

**calendars** — Calendar sources from connected providers.
Columns: provider, connection_id, tracking_id, name, color, source, enabled, raw_json
FK: connection_id → connections

**events** — Calendar events synced from providers.
Columns: calendar_id, tracking_id, title, started_at, ended_at, location, meeting_link, description, note, recurrence_series_id, has_recurrence_rules, is_all_day, participants_json, raw_json, sync_status, deleted_at
FK: calendar_id → calendars

**event_participants** — Parsed participants for an event.
Columns: event_id, human_id, email, name, is_organizer, is_current_user
FK: event_id → events (ON DELETE CASCADE), human_id → humans (ON DELETE SET NULL)
Unique: (event_id, email)

### People

**organizations** — Teams/companies that humans belong to.
Columns: name, pinned, pin_order

**humans** — People known to the user.
Columns: name, email, org_id, job_title, linkedin_username, memo, pinned, pin_order, linked_user_id
FK: org_id → organizations

**aliases** — External identity mappings for humans (Slack, Google, etc.).
Columns: human_id, provider, external_id, workspace_id, display_name, confidence
FK: human_id → humans
Unique: (provider, external_id, workspace_id)

### Legacy Session

These tables predate the task-centric model. Still in use during migration.

**meetings** — A recorded session. Being replaced by task type=meeting + meeting_artifacts.
Columns: title, summary, memo, folder_id, event_id
FK: event_id → events

**words** — Transcript words for a meeting.
Columns: meeting_id, text, start_ms, end_ms, channel, state, task_id
FK: meeting_id → meetings, task_id → tasks

**speaker_hints** — Speaker diarization hints for meeting transcript.
Columns: meeting_id, word_id, kind, speaker_index, provider, channel, human_id, task_id
FK: meeting_id → meetings, word_id → words, task_id → tasks

**meeting_participants** — People who attended a meeting.
Columns: meeting_id, human_id, source
FK: meeting_id → meetings, human_id → humans
Unique: (meeting_id, human_id)

**chat_messages** — Chat history within a meeting session.
Columns: meeting_id, role, content
FK: meeting_id → meetings

**notes** — Freeform notes attached to a meeting or entity.
Columns: meeting_id, kind, title, content, entity_type, entity_id

**meetings_fts** — FTS5 virtual table over meetings(title, summary, memo). Maintained by triggers.

### Threads

**threads** — Conversation threads, optionally linked to a meeting.
Columns: meeting_id, title
FK: meeting_id → meetings

**messages** — Messages within a thread.
Columns: thread_id, role, parts
FK: thread_id → threads

### System

**users** — App users.
Columns: name

**folders** — Hierarchical folder structure for organizing meetings.
Columns: name, parent_id
FK: parent_id → folders

**settings** — Key-value app settings. Not synced.
Columns: key (PK), value

### Views

**timeline** — Union of meeting_participants and entity-linked notes. Returns: human_id, source_type, source_id, happened_at, title. Used to show activity history for a human.

## Multi-User

One SQLite DB per team. Personal use = 1-person team. No `team_id`; the DB boundary is the team boundary.

Every synced table carries `user_id` (row owner) and `visibility` (`'public'` or `'private'`).

SQLite Cloud RLS policy for every table: `user_id = auth_userid() OR visibility = 'public'`

`daily.date` is `UNIQUE(date, user_id)`, not globally unique — multiple users share the same team DB.

Child table visibility (task_notes, task_events, meeting_artifacts, etc.) is always inherited from the parent task. `set_task_visibility` propagates atomically.

Personal DB uses the same schema but does not enable RLS.

## Sync

OffSync (SQLite Cloud CRDT) for task tables. Row/column-level sync.

Document body (`daily.body`, TipTap JSON) uses LWW at cell level. No ProseMirror collab. No Yjs/Yrs. Acceptable because live co-editing is not a requirement.

Domain-specific merge rules (status transitions, user-vs-agent precedence) are handled in app logic, not DB config. OffSync provides generic row/column merge; the app interprets `task_events` to resolve semantic conflicts.

## Invariants

- All NOT NULL non-PK columns must have defaults (OffSync/cloudsync compatibility).
- TEXT primary keys (UUIDs). Required by SQLite Sync.
- No ON DELETE CASCADE. App-level cascade only. SQLite Sync warns about trigger/cascade interactions during sync.
- No DB triggers on synced tables. Triggers can fire multiple times during column-by-column sync.
- `sort_key` uses fractional indexing, managed in app code. DB stores and sorts lexicographically.

## Provenance

`task_notes` for memos, agent notes, imported snippets. Append-only with soft delete.
`task_events` for audit trail. Who changed what, why. Actor types: user, agent, sync.

Agent writes to task rows and task_notes/task_events, never directly to the ProseMirror document.

## Task Lifecycle

A task belongs to exactly one daily at a time. Rescheduling = changing `daily_id`.
"Remove from today, bring back tomorrow" = update `tasks.daily_id` to tomorrow's daily row.
