CREATE VIEW IF NOT EXISTS timeline AS
  SELECT
    mp.human_id AS human_id,
    'meeting' AS source_type,
    m.id AS source_id,
    m.created_at AS happened_at,
    COALESCE(m.title, '') AS title
  FROM meeting_participants mp
  JOIN meetings m ON m.id = mp.meeting_id

  UNION ALL

  SELECT
    n.entity_id AS human_id,
    'note' AS source_type,
    n.id AS source_id,
    n.created_at AS happened_at,
    n.title AS title
  FROM notes n
  WHERE n.entity_type = 'human' AND n.entity_id != '';
