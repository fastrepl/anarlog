-- Per-folder chat instructions. Older builds ignore the new column.
ALTER TABLE folders ADD COLUMN instructions TEXT NOT NULL DEFAULT '';
