ALTER TABLE search_index_dirty
ADD COLUMN acknowledged_generation INTEGER NOT NULL DEFAULT 0
  CHECK (acknowledged_generation >= 0 AND acknowledged_generation <= generation);
