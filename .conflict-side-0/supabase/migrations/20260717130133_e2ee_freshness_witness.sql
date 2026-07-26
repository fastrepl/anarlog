ALTER TABLE public.workspaces
  ADD COLUMN e2ee_freshness_initialized_at timestamptz;

CREATE TABLE public.e2ee_freshness_events (
  sequence bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  record_id text NOT NULL,
  payload_hash text NOT NULL,
  payload text NOT NULL,
  created_by uuid NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT e2ee_freshness_events_record_id_check CHECK (
    record_id ~ '^[A-Za-z0-9_-]{43}$'
  ),
  CONSTRAINT e2ee_freshness_events_payload_hash_check CHECK (
    payload_hash ~ '^[A-Za-z0-9_-]{43}$'
  ),
  CONSTRAINT e2ee_freshness_events_payload_size_check CHECK (
    octet_length(payload) BETWEEN 1 AND 16777216
  ),
  CONSTRAINT e2ee_freshness_events_payload_key UNIQUE (
    workspace_id,
    record_id,
    payload_hash
  )
);

CREATE INDEX e2ee_freshness_events_workspace_sequence_idx
  ON public.e2ee_freshness_events(workspace_id, sequence);

ALTER TABLE public.e2ee_freshness_events ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.e2ee_freshness_events
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON SEQUENCE public.e2ee_freshness_events_sequence_seq
  FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT ON TABLE public.e2ee_freshness_events TO service_role;
GRANT USAGE, SELECT ON SEQUENCE public.e2ee_freshness_events_sequence_seq TO service_role;

CREATE OR REPLACE FUNCTION public.claim_personal_workspace_e2ee_key(
  p_actor_user_id uuid,
  p_key_id text
)
RETURNS TABLE (key_id text)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_actor_user_id IS NULL
    OR p_key_id IS NULL
    OR p_key_id !~ '^[A-Za-z0-9_-]{22}$'
  THEN
    RAISE EXCEPTION 'E2EE key identity is invalid' USING ERRCODE = '22023';
  END IF;

  UPDATE public.workspaces AS workspace
  SET e2ee_key_id = p_key_id,
      e2ee_freshness_initialized_at = now(),
      updated_at = now()
  WHERE workspace.id = p_actor_user_id
    AND workspace.owner_user_id = p_actor_user_id
    AND workspace.kind = 'personal'
    AND workspace.deleted_at IS NULL
    AND workspace.e2ee_key_id IS NULL;

  RETURN QUERY
  SELECT workspace.e2ee_key_id
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_actor_user_id
    AND workspace.owner_user_id = p_actor_user_id
    AND workspace.kind = 'personal'
    AND workspace.deleted_at IS NULL;
END;
$$;

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
    AND workspace.id = p_actor_user_id
    AND workspace.owner_user_id = p_actor_user_id
    AND workspace.kind = 'personal'
    AND workspace.deleted_at IS NULL
    AND workspace.e2ee_key_id IS NOT NULL
  FOR UPDATE;

  IF NOT FOUND THEN
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

REVOKE ALL ON FUNCTION public.publish_e2ee_freshness_events(uuid, uuid, boolean, jsonb)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_e2ee_freshness_page(uuid, uuid, bigint, bigint, integer)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.publish_e2ee_freshness_events(uuid, uuid, boolean, jsonb)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.read_e2ee_freshness_page(uuid, uuid, bigint, bigint, integer)
  TO service_role;
