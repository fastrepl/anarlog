BEGIN;

ALTER TABLE public.workspaces ADD COLUMN e2ee_cloud_authority_after bigint
  CHECK (e2ee_cloud_authority_after >= 0);
ALTER TABLE public.e2ee_freshness_events ADD COLUMN protocol integer NOT NULL DEFAULT 1
  CHECK (protocol IN (1, 2));
ALTER TABLE public.e2ee_freshness_events DROP CONSTRAINT e2ee_freshness_events_payload_key;
CREATE UNIQUE INDEX e2ee_freshness_legacy_payload_key
  ON public.e2ee_freshness_events(workspace_id, record_id, payload_hash) WHERE protocol = 1;

CREATE TABLE public.e2ee_replica_receipts (
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  mutation_id uuid NOT NULL,
  request_hash text NOT NULL,
  accepted_head bigint NOT NULL,
  events jsonb NOT NULL,
  PRIMARY KEY (workspace_id, mutation_id)
);
ALTER TABLE public.e2ee_replica_receipts ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON public.e2ee_replica_receipts FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT ON public.e2ee_replica_receipts TO service_role;

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

  IF EXISTS (SELECT 1 FROM public.workspaces WHERE id = p_workspace_id
             AND e2ee_cloud_authority_after IS NOT NULL) THEN
    RAISE EXCEPTION 'This workspace requires a cloud-authoritative sync client' USING ERRCODE = 'A0002';
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
  ON CONFLICT DO NOTHING;

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

-- The workspace lock serializes acceptance, including the legacy writer above.
-- An immutable request receipt survives lost responses and restored client backups.
CREATE FUNCTION public.accept_e2ee_replica_batch(
  p_actor_user_id uuid, p_workspace_id uuid, p_mutation_id uuid,
  p_base_sequence bigint, p_initialize boolean, p_events jsonb
) RETURNS TABLE (initialized_at timestamptz, head_sequence bigint,
                 cloud_authority_after bigint, mutation_id uuid, receipts jsonb)
LANGUAGE plpgsql SECURITY INVOKER SET search_path = '' AS $$
DECLARE
  v_workspace public.workspaces%ROWTYPE;
  v_key text;
  v_head bigint;
  v_hash text;
  v_receipt public.e2ee_replica_receipts%ROWTYPE;
  v_receipts jsonb;
BEGIN
  IF p_mutation_id IS NULL OR p_base_sequence IS NULL OR p_base_sequence < 0
     OR p_initialize IS NULL OR p_events IS NULL OR jsonb_typeof(p_events) <> 'array'
     OR jsonb_array_length(p_events) NOT BETWEEN 1 AND 1024 THEN
    RAISE EXCEPTION 'Invalid replica batch' USING ERRCODE = '22023';
  END IF;
  SELECT * INTO v_workspace FROM public.workspaces WHERE id = p_workspace_id FOR UPDATE;
  v_key := private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id);
  IF v_workspace.id IS NULL OR v_key IS NULL THEN
    RAISE EXCEPTION 'Replica access denied' USING ERRCODE = '42501';
  END IF;
  v_hash := encode(extensions.digest(jsonb_build_array(p_base_sequence, p_initialize, p_events)::text, 'sha256'), 'hex');
  SELECT * INTO v_receipt FROM public.e2ee_replica_receipts
    WHERE workspace_id = p_workspace_id AND e2ee_replica_receipts.mutation_id = p_mutation_id;
  IF FOUND THEN
    IF v_receipt.request_hash <> v_hash THEN
      RAISE EXCEPTION 'Mutation identity was reused' USING ERRCODE = '22023';
    END IF;
    RETURN QUERY SELECT v_workspace.e2ee_freshness_initialized_at, v_receipt.accepted_head,
      v_workspace.e2ee_cloud_authority_after, p_mutation_id, v_receipt.events;
    RETURN;
  END IF;
  SELECT COALESCE(max(sequence), 0) INTO v_head FROM public.e2ee_freshness_events WHERE workspace_id = p_workspace_id;
  IF v_head <> p_base_sequence THEN
    RAISE EXCEPTION 'Replica base changed; pull before retrying' USING ERRCODE = '40001';
  END IF;
  IF v_workspace.e2ee_freshness_initialized_at IS NULL AND NOT p_initialize THEN
    RAISE EXCEPTION 'Replica requires established state' USING ERRCODE = '55000';
  END IF;
  IF EXISTS (
    SELECT 1 FROM jsonb_array_elements(p_events) AS event(value)
    WHERE jsonb_typeof(event.value) <> 'object'
      OR COALESCE(event.value->>'record_id', '') !~ '^[A-Za-z0-9_-]{43}$'
      OR COALESCE(event.value->>'payload_hash', '') !~ '^[A-Za-z0-9_-]{43}$'
      OR octet_length(COALESCE(event.value->>'payload', '')) NOT BETWEEN 1 AND 16777216
      OR private.e2ee_freshness_payload_key_id(event.value->>'payload') IS NULL
      OR (v_workspace.kind::text = 'shared' AND private.e2ee_freshness_payload_key_id(event.value->>'payload') IS DISTINCT FROM v_key)
      OR event.value->>'payload_hash' <> rtrim(translate(encode(extensions.digest(event.value->>'payload', 'sha256'), 'base64'), '+/', '-_'), '=')
  ) OR (SELECT count(DISTINCT value->>'record_id') FROM jsonb_array_elements(p_events)) <> jsonb_array_length(p_events) THEN
    RAISE EXCEPTION 'Invalid replica event' USING ERRCODE = '22023';
  END IF;
  UPDATE public.workspaces SET
    e2ee_cloud_authority_after = COALESCE(e2ee_cloud_authority_after, v_head),
    e2ee_freshness_initialized_at = COALESCE(e2ee_freshness_initialized_at, now())
    WHERE id = p_workspace_id RETURNING * INTO v_workspace;
  WITH accepted AS (
    INSERT INTO public.e2ee_freshness_events(workspace_id, record_id, payload_hash, payload, created_by, protocol)
    SELECT p_workspace_id, value->>'record_id', value->>'payload_hash', value->>'payload', p_actor_user_id, 2
    FROM jsonb_array_elements(p_events)
    RETURNING sequence, record_id, payload_hash
  ) SELECT max(sequence), jsonb_agg(jsonb_build_object('sequence', sequence, 'recordId', record_id, 'payloadHash', payload_hash) ORDER BY sequence)
    INTO v_head, v_receipts FROM accepted;
  INSERT INTO public.e2ee_replica_receipts VALUES (p_workspace_id, p_mutation_id, v_hash, v_head, v_receipts);
  RETURN QUERY SELECT v_workspace.e2ee_freshness_initialized_at, v_head,
    v_workspace.e2ee_cloud_authority_after, p_mutation_id, v_receipts;
END;
$$;
REVOKE ALL ON FUNCTION public.accept_e2ee_replica_batch(uuid, uuid, uuid, bigint, boolean, jsonb) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.accept_e2ee_replica_batch(uuid, uuid, uuid, bigint, boolean, jsonb) TO service_role;

-- Old clients must stop consuming history as well as stop writing it. Otherwise
-- their revision-based receiver could retire unsent edits while rejecting uploads.
ALTER FUNCTION public.read_e2ee_freshness_page_v2(uuid, uuid, bigint, bigint, integer, integer)
  RENAME TO read_e2ee_freshness_page_unchecked;
CREATE FUNCTION public.read_e2ee_freshness_page_v2(
  p_actor_user_id uuid, p_workspace_id uuid, p_after_sequence bigint,
  p_through_sequence bigint, p_limit integer, p_max_bytes integer
) RETURNS TABLE (initialized_at timestamptz, head_sequence bigint, through_sequence bigint,
                 event_sequence bigint, record_id text, payload_hash text, payload text)
LANGUAGE plpgsql SECURITY INVOKER SET search_path = '' AS $$
DECLARE v_boundary bigint;
BEGIN
  IF private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id) IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness read is not permitted' USING ERRCODE = '42501';
  END IF;
  SELECT e2ee_cloud_authority_after INTO v_boundary FROM public.workspaces WHERE id = p_workspace_id FOR SHARE;
  IF (p_after_sequence <> 0 OR p_through_sequence IS DISTINCT FROM 0)
     AND v_boundary IS NOT NULL THEN
    RAISE EXCEPTION 'This workspace requires a cloud-authoritative sync client' USING ERRCODE = 'A0002';
  END IF;
  RETURN QUERY SELECT * FROM public.read_e2ee_freshness_page_unchecked(p_actor_user_id, p_workspace_id,
    p_after_sequence, p_through_sequence, p_limit, p_max_bytes);
END;
$$;
REVOKE ALL ON FUNCTION public.read_e2ee_freshness_page_v2(uuid, uuid, bigint, bigint, integer, integer) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_e2ee_freshness_page_v2(uuid, uuid, bigint, bigint, integer, integer) TO service_role;
CREATE OR REPLACE FUNCTION public.read_e2ee_freshness_page(
  p_actor_user_id uuid, p_workspace_id uuid, p_after_sequence bigint,
  p_through_sequence bigint, p_limit integer
) RETURNS TABLE (initialized_at timestamptz, head_sequence bigint, through_sequence bigint,
                 event_sequence bigint, record_id text, payload_hash text, payload text)
LANGUAGE plpgsql SECURITY INVOKER SET search_path = '' AS $$
BEGIN
  IF p_limit IS NULL OR p_limit NOT BETWEEN 1 AND 64 THEN
    RAISE EXCEPTION 'E2EE freshness page is invalid' USING ERRCODE = '22023';
  END IF;
  RETURN QUERY SELECT * FROM public.read_e2ee_freshness_page_v2(p_actor_user_id, p_workspace_id,
    p_after_sequence, p_through_sequence, p_limit, 50331648);
END;
$$;
CREATE FUNCTION public.read_e2ee_replica_page(
  p_actor_user_id uuid, p_workspace_id uuid, p_after_sequence bigint,
  p_through_sequence bigint, p_limit integer, p_max_bytes integer
) RETURNS TABLE (initialized_at timestamptz, head_sequence bigint, through_sequence bigint,
                 event_sequence bigint, record_id text, payload_hash text, payload text,
                 cloud_authority_after bigint)
LANGUAGE sql SECURITY INVOKER SET search_path = '' AS $$
  SELECT page.*, workspace.e2ee_cloud_authority_after
  FROM public.read_e2ee_freshness_page_unchecked(p_actor_user_id, p_workspace_id, p_after_sequence,
                                        p_through_sequence, p_limit, p_max_bytes) AS page
  JOIN public.workspaces AS workspace ON workspace.id = p_workspace_id;
$$;

REVOKE ALL ON FUNCTION public.read_e2ee_replica_page(uuid, uuid, bigint, bigint, integer, integer) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_e2ee_replica_page(uuid, uuid, bigint, bigint, integer, integer) TO service_role;


COMMIT;
