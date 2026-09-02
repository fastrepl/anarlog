-- Per-folder icon. Older builds ignore the new column.
ALTER TABLE folders
ADD COLUMN icon_json TEXT NOT NULL DEFAULT '{"type":"icon","value":"folder","color":"#9ca3af"}';
