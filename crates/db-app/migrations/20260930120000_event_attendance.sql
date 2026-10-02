-- Persist normalized calendar attendance evidence for automatic meeting actions.
ALTER TABLE events ADD COLUMN attendance_json TEXT;
