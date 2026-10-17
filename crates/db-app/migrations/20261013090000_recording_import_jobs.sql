-- Local execution state and paths must never sync to other devices.
CREATE TABLE IF NOT EXISTS local_recording_import_jobs (
 id TEXT PRIMARY KEY NOT NULL,
 meeting_id TEXT NOT NULL DEFAULT '',
 request_key TEXT UNIQUE,
 file_path TEXT NOT NULL DEFAULT '',
 processing_status TEXT NOT NULL DEFAULT 'queued',
 summary_status TEXT NOT NULL DEFAULT 'pending',
 error TEXT,
 worker_id TEXT,
 updated_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS local_recording_import_worker (
 id TEXT PRIMARY KEY NOT NULL,
 vault_path TEXT NOT NULL DEFAULT '',
 updated_at INTEGER NOT NULL DEFAULT 0
);
