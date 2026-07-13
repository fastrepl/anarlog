WITH normalized_documents AS (
  SELECT
    document.session_id,
    document.sort_order,
    document.created_at,
    document.id,
    document.body_format,
    document.body,
    ltrim(document.body, char(9) || char(10) || char(13) || ' ') AS normalized_body
  FROM session_documents AS document
  JOIN sessions AS session
    ON session.id = document.session_id
   AND session.deleted_at IS NULL
  WHERE trim(session.title) = ''
    AND document.kind IN ('summary', 'template_output')
    AND document.deleted_at IS NULL
),
document_titles AS (
  SELECT
    session_id,
    sort_order,
    created_at,
    id,
    CASE
      WHEN body_format = 'markdown'
        AND substr(normalized_body, 1, 2) = '# '
      THEN substr(
        normalized_body,
        3,
        instr(normalized_body || char(10), char(10)) - 3
      )
      WHEN body_format = 'prosemirror_json'
        AND json_valid(body)
        AND json_extract(body, '$.content[0].type') = 'heading'
        AND json_extract(body, '$.content[0].attrs.level') = 1
      THEN (
        SELECT group_concat(json_extract(node.value, '$.text'), '')
        FROM json_each(body, '$.content[0].content') AS node
        WHERE json_extract(node.value, '$.type') = 'text'
      )
      ELSE ''
    END AS candidate_title
  FROM normalized_documents
),
valid_titles AS (
  SELECT
    session_id,
    trim(candidate_title) AS title,
    sort_order,
    created_at,
    id
  FROM document_titles
  WHERE trim(candidate_title) <> ''
    AND lower(trim(candidate_title)) NOT IN ('summary', 'untitled', 'untitled note')
),
ranked_titles AS (
  SELECT
    session_id,
    title,
    row_number() OVER (
      PARTITION BY session_id
      ORDER BY sort_order, created_at, id
    ) AS title_rank
  FROM valid_titles
)
UPDATE sessions
SET
  title = (
    SELECT ranked_titles.title
    FROM ranked_titles
    WHERE ranked_titles.session_id = sessions.id
      AND ranked_titles.title_rank = 1
  ),
  updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE trim(title) = ''
  AND deleted_at IS NULL
  AND EXISTS (
    SELECT 1
    FROM ranked_titles
    WHERE ranked_titles.session_id = sessions.id
      AND ranked_titles.title_rank = 1
  );
