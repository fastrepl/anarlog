CREATE OR REPLACE FUNCTION public.read_e2ee_freshness_page_v2(
  p_actor_user_id uuid,
  p_workspace_id uuid,
  p_after_sequence bigint,
  p_through_sequence bigint,
  p_limit integer,
  p_max_bytes integer
)
RETURNS TABLE (
  initialized_at timestamptz,
  head_sequence bigint,
  through_sequence bigint,
  event_sequence bigint,
  record_id text,
  payload_hash text,
  payload text
)
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = ''
AS $$
DECLARE
  v_initialized_at timestamptz;
  v_head_sequence bigint;
  v_through_sequence bigint;
BEGIN
  IF p_actor_user_id IS NULL
    OR p_workspace_id IS NULL
    OR p_after_sequence IS NULL
    OR p_after_sequence < 0
    OR p_limit IS NULL
    OR p_limit NOT BETWEEN 1 AND 1024
    OR p_max_bytes IS NULL
    OR p_max_bytes NOT BETWEEN 1 AND 50331648
  THEN
    RAISE EXCEPTION 'E2EE freshness page is invalid' USING ERRCODE = '22023';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at
  INTO v_initialized_at
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id
    AND workspace.id = p_actor_user_id
    AND workspace.owner_user_id = p_actor_user_id
    AND workspace.kind = 'personal'
    AND workspace.deleted_at IS NULL
    AND workspace.e2ee_key_id IS NOT NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'E2EE freshness read is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT COALESCE(MAX(event.sequence), 0)::bigint
  INTO v_head_sequence
  FROM public.e2ee_freshness_events AS event
  WHERE event.workspace_id = p_workspace_id;

  v_through_sequence := COALESCE(p_through_sequence, v_head_sequence);
  IF v_through_sequence < p_after_sequence OR v_through_sequence > v_head_sequence THEN
    RAISE EXCEPTION 'E2EE freshness page is invalid' USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  SELECT
    v_initialized_at,
    v_head_sequence,
    v_through_sequence,
    event.sequence,
    event.record_id,
    event.payload_hash,
    event.payload
  FROM (SELECT 1) AS singleton
  LEFT JOIN LATERAL (
    -- Fetch one row at a time so oversized ciphertext tails are never materialized.
    WITH RECURSIVE bounded AS (
      SELECT
        source.sequence,
        source.record_id,
        source.payload_hash,
        source.payload,
        1 AS event_count,
        (
          octet_length(source.record_id)
            + octet_length(source.payload_hash)
            + octet_length(source.payload)
            + 256
        )::bigint AS cumulative_bytes
      FROM LATERAL (
        SELECT candidate.sequence, candidate.record_id, candidate.payload_hash, candidate.payload
        FROM public.e2ee_freshness_events AS candidate
        WHERE candidate.workspace_id = p_workspace_id
          AND candidate.sequence > p_after_sequence
          AND candidate.sequence <= v_through_sequence
        ORDER BY candidate.sequence
        LIMIT 1
      ) AS source

      UNION ALL

      SELECT
        source.sequence,
        source.record_id,
        source.payload_hash,
        source.payload,
        bounded.event_count + 1,
        bounded.cumulative_bytes
          + octet_length(source.record_id)
          + octet_length(source.payload_hash)
          + octet_length(source.payload)
          + 256
      FROM bounded
      CROSS JOIN LATERAL (
        SELECT candidate.sequence, candidate.record_id, candidate.payload_hash, candidate.payload
        FROM public.e2ee_freshness_events AS candidate
        WHERE candidate.workspace_id = p_workspace_id
          AND candidate.sequence > bounded.sequence
          AND candidate.sequence <= v_through_sequence
        ORDER BY candidate.sequence
        LIMIT 1
      ) AS source
      WHERE bounded.event_count < p_limit
        AND bounded.cumulative_bytes
          + octet_length(source.record_id)
          + octet_length(source.payload_hash)
          + octet_length(source.payload)
          + 256 <= p_max_bytes
    )
    SELECT bounded.sequence, bounded.record_id, bounded.payload_hash, bounded.payload
    FROM bounded
    ORDER BY bounded.sequence
  ) AS event ON true;
END;
$$;

REVOKE ALL ON FUNCTION public.read_e2ee_freshness_page_v2(
  uuid,
  uuid,
  bigint,
  bigint,
  integer,
  integer
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_e2ee_freshness_page_v2(
  uuid,
  uuid,
  bigint,
  bigint,
  integer,
  integer
) TO service_role;
