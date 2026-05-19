# Session Files And SQLite Migration Plan

## Target State

Anarlog should use one Markdown file per session as the canonical session
document. SQLite should own structured application data such as people,
organizations, calendars, events, and relationship mappings. TinyBase should be
treated as transitional UI/shadow state during migration and removed from
durable main data over time.

## Storage Boundaries

- Session Markdown files are canonical for session content: raw notes,
  generated summary, and readable transcript.
- SQLite is canonical for structured data: people, organizations, calendars,
  events, session participants, tags, and other queryable records.
- TinyBase remains only as a compatibility bridge while consumers are moved
  behind domain hooks.
- TipTap JSON is runtime editor state, not the long-term source of truth.

## Compatibility Rules

- New readers must load the new single-file format first and fall back to the
  legacy session directory format.
- Update-time migration scripts may convert legacy session folders and TinyBase
  data before the UI loads.
- Runtime readers must still tolerate unmigrated legacy data for at least one
  compatibility window.
- Migration scripts must be idempotent and safe to resume after interruption.
- Do not delete legacy source files in the same release that first creates the
  new format.
- Preserve existing session ids so tabs, recents, audio, search, and references
  continue to resolve.
- Writes for migrated sessions should go only to the new format to avoid stale
  legacy files resurrecting content.

## Session Markdown Contract

Each migrated session should be represented by a single `session.md` file with
versioned frontmatter and stable section headings:

```md
---
schema_version: 1
id: "session-id"
created_at: "2026-05-19T00:00:00.000Z"
updated_at: "2026-05-19T00:05:00.000Z"
title: "Weekly sync"
folder_id: "work"
event_id: "calendar-event-id"
event_json: {"tracking_id":"calendar-event-id"}
participants_json: [{"person_id":"person-id","name":"Jane Doe"}]
tags_json: ["work"]
---

# Notes

Raw notes written in the editor.

# Summary

Generated summary content.

# Transcript

Readable transcript text.
```

Detailed transcript timing or speaker metadata should stay in this file only if
it is user data. Use a fenced machine block later if timing metadata is needed
for playback alignment.

## Migration Flow

1. Define and test the Markdown parse/render contract.
2. Add a session file service with `loadSessionFile`, `saveSessionFile`, and
   legacy-folder fallback.
3. Add update-time migration scripts:
   - convert legacy `sessions/<id>/{_meta.json,_memo.md,_summary.md,transcript.json}`
     into `sessions/<id>/session.md`
   - import TinyBase structured data into SQLite
   - record per-domain and per-session migration markers
4. Move editor writes to the session file service.
5. Move structured domains to SQLite using the existing per-domain migration
   pattern:
   - put consumers behind stable hooks
   - add Rust migrations and Drizzle schema
   - import legacy data idempotently
   - shadow-hydrate TinyBase where needed
   - remove TinyBase tables and persisters after consumers move
6. Rename contact-domain code from `humans` toward `people` at hook/API
   boundaries first, then storage.
7. After a compatibility window, remove old session content persisters and
   legacy writers.

## SQLite Domain Order

1. `people` compatibility layer over existing `humans`
2. organizations
3. calendars
4. events
5. session participants
6. tags and mappings

Use text ids, safe defaults, minimal triggers, and idempotent legacy imports.

## Verification

- Markdown parse/render golden tests.
- Legacy session-folder migration fixtures.
- Delete and reopen tests proving deleted notes do not reappear.
- Update-time migration idempotency tests.
- SQLite import idempotency tests.
- `pnpm -F @hypr/desktop typecheck`
- `pnpm -F @hypr/desktop test`
- `cargo check` when Rust migrations or migration scripts change.
