ALTER TABLE shared_session_cache
ADD COLUMN web_editable INTEGER NOT NULL DEFAULT 0
CHECK (web_editable IN (0, 1));

ALTER TABLE shared_session_cache
ADD COLUMN web_edit_base_content_revision INTEGER
CHECK (
  web_edit_base_content_revision IS NULL
  OR web_edit_base_content_revision > 0
);

ALTER TABLE shared_session_cache
ADD COLUMN web_edit_base_title TEXT
CHECK (
  web_edit_base_title IS NULL
  OR (
    web_edit_base_title = trim(web_edit_base_title)
    AND length(CAST(web_edit_base_title AS BLOB)) <= 4096
  )
);

ALTER TABLE shared_session_cache
ADD COLUMN web_edit_base_body_json TEXT
CHECK (
  CASE
    WHEN web_edit_base_body_json IS NULL THEN 1
    WHEN json_valid(web_edit_base_body_json) THEN
      json_type(web_edit_base_body_json) = 'object'
      AND json_extract(web_edit_base_body_json, '$.type') = 'doc'
      AND length(CAST(web_edit_base_body_json AS BLOB)) <= 2097152
    ELSE 0
  END
);

CREATE TRIGGER IF NOT EXISTS validate_shared_session_cache_web_edit_base_insert
BEFORE INSERT ON shared_session_cache
WHEN
  (
    NEW.web_edit_base_content_revision IS NULL
    OR NEW.web_edit_base_title IS NULL
    OR NEW.web_edit_base_body_json IS NULL
  )
  <> (
    NEW.web_edit_base_content_revision IS NULL
    AND NEW.web_edit_base_title IS NULL
    AND NEW.web_edit_base_body_json IS NULL
  )
  OR (
    NEW.web_edit_base_content_revision IS NOT NULL
    AND (
      NEW.manage_access <> 1
      OR NEW.web_edit_base_content_revision >= NEW.content_revision
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'invalid shared-session web edit base');
END;

CREATE TRIGGER IF NOT EXISTS validate_shared_session_cache_web_edit_base_update
BEFORE UPDATE OF
  content_revision,
  manage_access,
  web_edit_base_content_revision,
  web_edit_base_title,
  web_edit_base_body_json
ON shared_session_cache
WHEN
  (
    NEW.web_edit_base_content_revision IS NULL
    OR NEW.web_edit_base_title IS NULL
    OR NEW.web_edit_base_body_json IS NULL
  )
  <> (
    NEW.web_edit_base_content_revision IS NULL
    AND NEW.web_edit_base_title IS NULL
    AND NEW.web_edit_base_body_json IS NULL
  )
  OR (
    NEW.web_edit_base_content_revision IS NOT NULL
    AND (
      NEW.manage_access <> 1
      OR NEW.web_edit_base_content_revision >= NEW.content_revision
    )
  )
BEGIN
  SELECT RAISE(ABORT, 'invalid shared-session web edit base');
END;
