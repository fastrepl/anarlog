ALTER TABLE shared_session_cache
ADD COLUMN attachments_json TEXT NOT NULL DEFAULT '[]'
CHECK (
  CASE
    WHEN json_valid(attachments_json) THEN
      json_type(attachments_json) = 'array'
      AND json_array_length(attachments_json) <= 64
      AND length(CAST(attachments_json AS BLOB)) <= 262144
    ELSE 0
  END
);
