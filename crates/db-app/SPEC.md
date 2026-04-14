# db-app Spec

`db-app` is the local desktop SQLite store.

## Tables

### `daily_notes`

One row per user per date for the canonical daily note/editor document.

Columns:
- `id`
- `date`
- `body`
- `user_id`
- `visibility`
- `created_at`
- `updated_at`

Unique:
- `(date, user_id)`

### `daily_summaries`

One derived summary artifact per daily note. This is what the daily summary UI
should render.

Columns:
- `id`
- `daily_note_id`
- `date`
- `content`
- `timeline_json`
- `topics_json`
- `status`
- `source_cursor_ms`
- `source_fingerprint`
- `generation_error`
- `generated_at`
- `created_at`
- `updated_at`

Unique:
- `daily_note_id`

### `prompt_overrides`

One local override row per editable built-in prompt task.

Columns:
- `task_type`
- `content`
- `created_at`
- `updated_at`

Unique:
- `task_type`

### `templates`

User-authored note templates mirrored into the desktop SQLite database.

Columns:
- `id`
- `title`
- `description`
- `pinned`
- `pin_order`
- `category`
- `targets_json`
- `sections_json`
- `created_at`
- `updated_at`

Unique:
- `id`

### `activity_observation_events`

Raw observation lifecycle events emitted by activity capture.

### `activity_screenshots`

Captured screenshots plus blob payloads and snapshot metadata.

### `activity_observation_analyses`

Per-screenshot analysis output used to build higher-level summaries.

## Model

- `daily_notes` is user-authored canonical content.
- `daily_summaries` is machine-generated durable output.
- `templates` is the durable local store for user templates.
- Activity capture tables are append-heavy local telemetry owned by the desktop app.
