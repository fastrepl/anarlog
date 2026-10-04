-- Device-owned policy and crash-recoverable local purge, never synced.
CREATE TABLE managed_app_settings (
 id TEXT PRIMARY KEY NOT NULL,
 value_json TEXT NOT NULL DEFAULT 'null'
) STRICT;
CREATE TRIGGER managed_app_settings_insert BEFORE INSERT ON app_settings
WHEN EXISTS(SELECT 1 FROM managed_app_settings m WHERE m.id=NEW.id AND m.value_json<>NEW.value_json)
BEGIN SELECT RAISE(ABORT, 'setting is managed'); END;
CREATE TRIGGER managed_app_settings_update BEFORE UPDATE ON app_settings
WHEN EXISTS(SELECT 1 FROM managed_app_settings m WHERE m.id=OLD.id AND (m.value_json<>NEW.value_json OR NEW.id<>OLD.id))
BEGIN SELECT RAISE(ABORT, 'setting is managed'); END;
CREATE TRIGGER managed_app_settings_delete BEFORE DELETE ON app_settings
WHEN EXISTS(SELECT 1 FROM managed_app_settings m WHERE m.id=OLD.id)
BEGIN SELECT RAISE(ABORT, 'setting is managed'); END;
CREATE TABLE local_meeting_purges (
 meeting_id TEXT PRIMARY KEY NOT NULL,
 export_sha256 TEXT NOT NULL DEFAULT '',
 paths_json TEXT NOT NULL DEFAULT '[]'
) STRICT;
