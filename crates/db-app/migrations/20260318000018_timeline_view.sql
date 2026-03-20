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
    a.human_id AS human_id,
    'slack' AS source_type,
    st.id AS source_id,
    st.started_at AS happened_at,
    sc.name AS title
  FROM slack_thread_participants stp
  JOIN slack_threads st ON st.id = stp.thread_id
  JOIN slack_channels sc ON sc.id = st.channel_id
  JOIN aliases a ON a.id = stp.alias_id

  UNION ALL

  SELECT
    n.entity_id AS human_id,
    'note' AS source_type,
    n.id AS source_id,
    n.created_at AS happened_at,
    n.title AS title
  FROM notes n
  WHERE n.entity_type = 'human' AND n.entity_id != '';
