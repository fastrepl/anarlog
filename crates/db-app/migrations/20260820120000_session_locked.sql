-- Add a per-note privacy lock. Content stays on disk; the desktop UI
-- requires device authentication before revealing a locked note.
ALTER TABLE sessions ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
