CREATE OR REPLACE FUNCTION private.active_e2ee_freshness_key_id(
  p_actor_user_id uuid,
  p_workspace_id uuid
)
RETURNS text
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT workspace.e2ee_key_id
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id
    AND workspace.id = p_actor_user_id
    AND workspace.owner_user_id = p_actor_user_id
    AND workspace.kind = 'personal'
    AND workspace.deleted_at IS NULL
    AND workspace.e2ee_key_id IS NOT NULL

  UNION ALL

  SELECT workspace_key.key_id
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  JOIN public.workspace_e2ee_keys AS workspace_key
    ON workspace_key.workspace_id = workspace.id
    AND workspace_key.retired_at IS NULL
  WHERE workspace.id = p_workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND membership.user_id = p_actor_user_id
    AND membership.deleted_at IS NULL;
$$;

CREATE OR REPLACE FUNCTION private.e2ee_freshness_payload_key_id(
  p_payload text
)
RETURNS text
LANGUAGE plpgsql
IMMUTABLE
STRICT
SECURITY INVOKER
SET search_path = ''
AS $$
DECLARE
  v_envelope jsonb;
BEGIN
  BEGIN
    v_envelope := p_payload::jsonb;
  EXCEPTION WHEN invalid_text_representation THEN
    RETURN NULL;
  END;

  IF jsonb_typeof(v_envelope) <> 'object'
    OR v_envelope ->> 'version' <> '1'
    OR COALESCE(v_envelope ->> 'key_id', '') !~ '^[A-Za-z0-9_-]{22}$'
  THEN
    RETURN NULL;
  END IF;

  RETURN v_envelope ->> 'key_id';
END;
$$;

REVOKE ALL ON FUNCTION private.active_e2ee_freshness_key_id(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.e2ee_freshness_payload_key_id(text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.active_e2ee_freshness_key_id(uuid, uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.e2ee_freshness_payload_key_id(text)
  TO service_role;

CREATE OR REPLACE FUNCTION private.initialize_shared_workspace_e2ee_witness()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  UPDATE public.workspaces AS workspace
  SET e2ee_freshness_initialized_at = COALESCE(
    workspace.e2ee_freshness_initialized_at,
    now()
  )
  WHERE workspace.id = NEW.workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.initialize_shared_workspace_e2ee_witness()
  FROM PUBLIC, anon, authenticated;

CREATE TRIGGER on_shared_workspace_e2ee_key_witness_initialized
  AFTER INSERT ON public.workspace_e2ee_keys
  FOR EACH ROW EXECUTE FUNCTION private.initialize_shared_workspace_e2ee_witness();

UPDATE public.workspaces AS workspace
SET e2ee_freshness_initialized_at = COALESCE(
  workspace.e2ee_freshness_initialized_at,
  workspace_key.created_at
)
FROM public.workspace_e2ee_keys AS workspace_key
WHERE workspace.id = workspace_key.workspace_id
  AND workspace.kind = 'shared'
  AND workspace.deleted_at IS NULL
  AND workspace_key.retired_at IS NULL;

CREATE OR REPLACE FUNCTION public.publish_e2ee_freshness_events(
  p_actor_user_id uuid,
  p_workspace_id uuid,
  p_initialize boolean,
  p_events jsonb
)
RETURNS TABLE (
  initialized_at timestamptz,
  head_sequence bigint
)
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = ''
AS $$
DECLARE
  v_active_key_id text;
  v_initialized_at timestamptz;
  v_events jsonb := COALESCE(p_events, '[]'::jsonb);
BEGIN
  IF p_actor_user_id IS NULL OR p_workspace_id IS NULL OR p_initialize IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness request is invalid' USING ERRCODE = '22023';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at
  INTO v_initialized_at
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'E2EE freshness publication is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id)
  INTO v_active_key_id;

  IF v_active_key_id IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness publication is not permitted' USING ERRCODE = '42501';
  END IF;

  IF jsonb_typeof(v_events) <> 'array' OR jsonb_array_length(v_events) > 64 THEN
    RAISE EXCEPTION 'E2EE freshness event batch is invalid' USING ERRCODE = '22023';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM jsonb_array_elements(v_events) AS event(value)
    WHERE jsonb_typeof(event.value) <> 'object'
      OR COALESCE(event.value->>'record_id', '') !~ '^[A-Za-z0-9_-]{43}$'
      OR COALESCE(event.value->>'payload_hash', '') !~ '^[A-Za-z0-9_-]{43}$'
      OR octet_length(COALESCE(event.value->>'payload', '')) NOT BETWEEN 1 AND 16777216
      OR private.e2ee_freshness_payload_key_id(event.value->>'payload')
        IS DISTINCT FROM v_active_key_id
      OR event.value->>'payload_hash' <> rtrim(
        translate(
          encode(extensions.digest(event.value->>'payload', 'sha256'), 'base64'),
          '+/',
          '-_'
        ),
        '='
      )
  ) THEN
    RAISE EXCEPTION 'E2EE freshness event is invalid' USING ERRCODE = '22023';
  END IF;

  IF v_initialized_at IS NULL AND NOT p_initialize THEN
    RAISE EXCEPTION 'E2EE freshness witness is not initialized' USING ERRCODE = '55000';
  END IF;

  IF v_initialized_at IS NULL AND jsonb_array_length(v_events) = 0 THEN
    RAISE EXCEPTION 'E2EE freshness initialization requires established state'
      USING ERRCODE = '55000';
  END IF;

  INSERT INTO public.e2ee_freshness_events (
    workspace_id,
    record_id,
    payload_hash,
    payload,
    created_by
  )
  SELECT
    p_workspace_id,
    event.value->>'record_id',
    event.value->>'payload_hash',
    event.value->>'payload',
    p_actor_user_id
  FROM jsonb_array_elements(v_events) AS event(value)
  ON CONFLICT (workspace_id, record_id, payload_hash) DO NOTHING;

  IF v_initialized_at IS NULL THEN
    UPDATE public.workspaces AS workspace
    SET e2ee_freshness_initialized_at = now(),
        updated_at = now()
    WHERE workspace.id = p_workspace_id
    RETURNING workspace.e2ee_freshness_initialized_at
    INTO v_initialized_at;
  END IF;

  RETURN QUERY
  SELECT
    v_initialized_at,
    COALESCE(MAX(event.sequence), 0)::bigint
  FROM public.e2ee_freshness_events AS event
  WHERE event.workspace_id = p_workspace_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.read_e2ee_freshness_page(
  p_actor_user_id uuid,
  p_workspace_id uuid,
  p_after_sequence bigint,
  p_through_sequence bigint,
  p_limit integer
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
    OR p_limit NOT BETWEEN 1 AND 64
  THEN
    RAISE EXCEPTION 'E2EE freshness page is invalid' USING ERRCODE = '22023';
  END IF;

  IF private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id) IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness read is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at
  INTO v_initialized_at
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;

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
    SELECT candidate.sequence, candidate.record_id, candidate.payload_hash, candidate.payload
    FROM public.e2ee_freshness_events AS candidate
    WHERE candidate.workspace_id = p_workspace_id
      AND candidate.sequence > p_after_sequence
      AND candidate.sequence <= v_through_sequence
    ORDER BY candidate.sequence
    LIMIT p_limit
  ) AS event ON true;
END;
$$;

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

  IF private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id) IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness read is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at
  INTO v_initialized_at
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;

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
