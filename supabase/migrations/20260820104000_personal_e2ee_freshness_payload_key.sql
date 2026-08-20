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
  v_workspace_kind text;
  v_events jsonb := COALESCE(p_events, '[]'::jsonb);
BEGIN
  IF p_actor_user_id IS NULL OR p_workspace_id IS NULL OR p_initialize IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness request is invalid' USING ERRCODE = '22023';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at, workspace.kind::text
  INTO v_initialized_at, v_workspace_kind
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
      OR private.e2ee_freshness_payload_key_id(event.value->>'payload') IS NULL
      OR (
        v_workspace_kind = 'shared'
        AND private.e2ee_freshness_payload_key_id(event.value->>'payload')
          IS DISTINCT FROM v_active_key_id
      )
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
