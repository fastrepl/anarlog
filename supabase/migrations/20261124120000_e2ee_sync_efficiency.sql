BEGIN;

ALTER TABLE public.workspaces
  ADD COLUMN IF NOT EXISTS e2ee_compacted_through bigint
  CHECK (e2ee_compacted_through >= 0);

ALTER TABLE public.e2ee_replica_receipts
  ADD COLUMN IF NOT EXISTS created_at timestamptz NOT NULL DEFAULT now();

CREATE INDEX IF NOT EXISTS e2ee_replica_receipts_created_at_idx
  ON public.e2ee_replica_receipts (created_at);

CREATE OR REPLACE FUNCTION public.prune_e2ee_replica_receipts(
  p_older_than interval DEFAULT interval '7 days',
  p_batch integer DEFAULT 5000
)
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_deleted integer := 0;
BEGIN
  IF p_older_than IS NULL OR p_older_than < interval '1 minute'
     OR p_batch IS NULL OR p_batch NOT BETWEEN 1 AND 50000 THEN
    RAISE EXCEPTION 'Invalid replica receipt prune parameters' USING ERRCODE = '22023';
  END IF;

  WITH target AS (
    SELECT ctid
    FROM public.e2ee_replica_receipts
    WHERE created_at < now() - p_older_than
    ORDER BY created_at
    LIMIT p_batch
  )
  DELETE FROM public.e2ee_replica_receipts AS receipt
  USING target
  WHERE receipt.ctid = target.ctid;

  GET DIAGNOSTICS v_deleted = ROW_COUNT;
  RETURN v_deleted;
END;
$$;

REVOKE ALL ON FUNCTION public.prune_e2ee_replica_receipts(interval, integer)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.prune_e2ee_replica_receipts(interval, integer)
  TO service_role;

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
    OR (v_envelope ->> 'version') NOT IN ('1', '2')
    OR COALESCE(v_envelope ->> 'key_id', '') !~ '^[A-Za-z0-9_-]{22}$'
  THEN
    RETURN NULL;
  END IF;

  RETURN v_envelope ->> 'key_id';
END;
$$;

REVOKE ALL ON FUNCTION private.e2ee_freshness_payload_key_id(text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.e2ee_freshness_payload_key_id(text)
  TO service_role;

CREATE OR REPLACE FUNCTION public.read_e2ee_witness_head(
  p_actor_user_id uuid,
  p_workspace_id uuid
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
  v_compacted_through bigint;
  v_head_sequence bigint;
BEGIN
  IF p_actor_user_id IS NULL OR p_workspace_id IS NULL THEN
    RAISE EXCEPTION 'E2EE witness head request is invalid' USING ERRCODE = '22023';
  END IF;

  IF private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id) IS NULL THEN
    RAISE EXCEPTION 'E2EE freshness read is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT workspace.e2ee_freshness_initialized_at, workspace.e2ee_compacted_through
  INTO v_initialized_at, v_compacted_through
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;

  SELECT GREATEST(
    COALESCE(v_compacted_through, 0::bigint),
    COALESCE((
      SELECT MAX(event.sequence)
      FROM public.e2ee_freshness_events AS event
      WHERE event.workspace_id = p_workspace_id
    ), 0::bigint)
  )::bigint
  INTO v_head_sequence;

  RETURN QUERY SELECT v_initialized_at, v_head_sequence;
END;
$$;

REVOKE ALL ON FUNCTION public.read_e2ee_witness_head(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_e2ee_witness_head(uuid, uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION public.read_e2ee_freshness_page_unchecked(
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
  v_compacted_through bigint;
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

  SELECT workspace.e2ee_freshness_initialized_at, workspace.e2ee_compacted_through
  INTO v_initialized_at, v_compacted_through
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;

  IF v_compacted_through IS NOT NULL
    AND p_after_sequence < v_compacted_through
    AND p_through_sequence IS DISTINCT FROM 0
  THEN
    RAISE SQLSTATE 'PGRST' USING
      MESSAGE = '{"code":"A0003","message":"E2EE witness history compacted","details":null,"hint":null}',
      DETAIL = '{"status":410,"headers":{}}';
  END IF;

  SELECT GREATEST(
    COALESCE(v_compacted_through, 0::bigint),
    COALESCE((
      SELECT MAX(event.sequence)
      FROM public.e2ee_freshness_events AS event
      WHERE event.workspace_id = p_workspace_id
    ), 0::bigint)
  )::bigint
  INTO v_head_sequence;

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
    WITH candidate AS (
      SELECT
        source.sequence,
        source.record_id,
        source.payload_hash,
        source.payload,
        row_number() OVER (ORDER BY source.sequence) AS row_num,
        sum(
          octet_length(source.record_id)
            + octet_length(source.payload_hash)
            + octet_length(source.payload)
            + 256
        ) OVER (ORDER BY source.sequence ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)::bigint AS cumulative_bytes
      FROM public.e2ee_freshness_events AS source
      WHERE source.workspace_id = p_workspace_id
        AND source.sequence > p_after_sequence
        AND source.sequence <= v_through_sequence
      ORDER BY source.sequence
      LIMIT p_limit
    )
    SELECT candidate.sequence, candidate.record_id, candidate.payload_hash, candidate.payload
    FROM candidate
    WHERE candidate.row_num = 1 OR candidate.cumulative_bytes <= p_max_bytes
    ORDER BY candidate.sequence
  ) AS event ON true;
END;
$$;

REVOKE ALL ON FUNCTION public.read_e2ee_freshness_page_unchecked(
  uuid, uuid, bigint, bigint, integer, integer
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_e2ee_freshness_page_unchecked(
  uuid, uuid, bigint, bigint, integer, integer
) TO service_role;

CREATE OR REPLACE FUNCTION public.accept_e2ee_replica_batch(
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
  v_key := private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id);
  IF v_key IS NULL THEN
    RAISE EXCEPTION 'Replica access denied' USING ERRCODE = '42501';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM public.e2ee_replica_receipts
                 WHERE workspace_id = p_workspace_id AND e2ee_replica_receipts.mutation_id = p_mutation_id)
     AND GREATEST(
       COALESCE((SELECT w.e2ee_compacted_through FROM public.workspaces AS w WHERE w.id = p_workspace_id), 0::bigint),
       COALESCE((SELECT max(sequence) FROM public.e2ee_freshness_events WHERE workspace_id = p_workspace_id), 0::bigint)
     ) <> p_base_sequence THEN
    RAISE SQLSTATE 'PGRST' USING
      MESSAGE = '{"code":"40001","message":"Replica base changed; pull before retrying","details":null,"hint":null}',
      DETAIL = '{"status":409,"headers":{}}';
  END IF;
  PERFORM private.admit_e2ee_witness_write();
  SELECT * INTO v_workspace FROM public.workspaces WHERE id = p_workspace_id FOR UPDATE NOWAIT;
  v_key := private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id);
  IF v_workspace.id IS NULL OR v_key IS NULL THEN
    RAISE EXCEPTION 'Replica access denied' USING ERRCODE = '42501';
  END IF;
  SELECT * INTO v_receipt FROM public.e2ee_replica_receipts
    WHERE workspace_id = p_workspace_id AND e2ee_replica_receipts.mutation_id = p_mutation_id;
  IF FOUND THEN
    v_hash := encode(extensions.digest(jsonb_build_array(p_base_sequence, p_initialize, p_events)::text, 'sha256'), 'hex');
    IF v_receipt.request_hash <> v_hash THEN
      RAISE EXCEPTION 'Mutation identity was reused' USING ERRCODE = '22023';
    END IF;
    RETURN QUERY SELECT v_workspace.e2ee_freshness_initialized_at, v_receipt.accepted_head,
      v_workspace.e2ee_cloud_authority_after, p_mutation_id, v_receipt.events;
    RETURN;
  END IF;
  v_head := GREATEST(
    COALESCE(v_workspace.e2ee_compacted_through, 0::bigint),
    COALESCE((SELECT max(sequence) FROM public.e2ee_freshness_events WHERE workspace_id = p_workspace_id), 0::bigint)
  );
  IF v_head <> p_base_sequence THEN
    RAISE SQLSTATE 'PGRST' USING
      MESSAGE = '{"code":"40001","message":"Replica base changed; pull before retrying","details":null,"hint":null}',
      DETAIL = '{"status":409,"headers":{}}';
  END IF;
  v_hash := encode(extensions.digest(jsonb_build_array(p_base_sequence, p_initialize, p_events)::text, 'sha256'), 'hex');
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
  INSERT INTO public.e2ee_replica_receipts (
    workspace_id, mutation_id, request_hash, accepted_head, events
  ) VALUES (p_workspace_id, p_mutation_id, v_hash, v_head, v_receipts);
  RETURN QUERY SELECT v_workspace.e2ee_freshness_initialized_at, v_head,
    v_workspace.e2ee_cloud_authority_after, p_mutation_id, v_receipts;
END;
$$;
REVOKE ALL ON FUNCTION public.accept_e2ee_replica_batch(uuid, uuid, uuid, bigint, boolean, jsonb) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.accept_e2ee_replica_batch(uuid, uuid, uuid, bigint, boolean, jsonb) TO service_role;

INSERT INTO storage.buckets (
  id,
  name,
  public,
  file_size_limit,
  allowed_mime_types
)
VALUES (
  'e2ee-snapshots',
  'e2ee-snapshots',
  false,
  545259520,
  ARRAY['application/octet-stream']::text[]
)
ON CONFLICT (id) DO UPDATE SET
  name = EXCLUDED.name,
  public = EXCLUDED.public,
  file_size_limit = EXCLUDED.file_size_limit,
  allowed_mime_types = EXCLUDED.allowed_mime_types;

DROP POLICY IF EXISTS e2ee_snapshots_deny_client_select
  ON storage.objects;
CREATE POLICY e2ee_snapshots_deny_client_select
  ON storage.objects
  AS RESTRICTIVE
  FOR SELECT
  TO anon, authenticated
  USING (bucket_id <> 'e2ee-snapshots');

DROP POLICY IF EXISTS e2ee_snapshots_deny_client_insert
  ON storage.objects;
CREATE POLICY e2ee_snapshots_deny_client_insert
  ON storage.objects
  AS RESTRICTIVE
  FOR INSERT
  TO anon, authenticated
  WITH CHECK (bucket_id <> 'e2ee-snapshots');

DROP POLICY IF EXISTS e2ee_snapshots_deny_client_update
  ON storage.objects;
CREATE POLICY e2ee_snapshots_deny_client_update
  ON storage.objects
  AS RESTRICTIVE
  FOR UPDATE
  TO anon, authenticated
  USING (bucket_id <> 'e2ee-snapshots')
  WITH CHECK (bucket_id <> 'e2ee-snapshots');

DROP POLICY IF EXISTS e2ee_snapshots_deny_client_delete
  ON storage.objects;
CREATE POLICY e2ee_snapshots_deny_client_delete
  ON storage.objects
  AS RESTRICTIVE
  FOR DELETE
  TO anon, authenticated
  USING (bucket_id <> 'e2ee-snapshots');

CREATE TABLE public.e2ee_workspace_snapshots (
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  snapshot_sequence bigint NOT NULL CHECK (snapshot_sequence > 0),
  object_key text NOT NULL,
  format_version smallint NOT NULL DEFAULT 1 CHECK (format_version = 1),
  payload_bytes bigint NOT NULL CHECK (payload_bytes BETWEEN 1 AND 545259520),
  payload_sha256 text NOT NULL CHECK (payload_sha256 ~ '^[0-9a-f]{64}$'),
  record_count integer NOT NULL CHECK (record_count >= 0),
  key_id text NOT NULL CHECK (key_id ~ '^[A-Za-z0-9_-]{22}$'),
  created_by uuid NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (workspace_id, snapshot_sequence),
  CONSTRAINT e2ee_workspace_snapshots_object_key_check CHECK (
    object_key ~ (
      '^'
      || workspace_id::text
      || '/[0-9]+-[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.ans1$'
    )
  )
);

ALTER TABLE public.e2ee_workspace_snapshots ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON public.e2ee_workspace_snapshots FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT, DELETE ON public.e2ee_workspace_snapshots TO service_role;

CREATE OR REPLACE FUNCTION public.read_latest_e2ee_snapshot(
  p_actor_user_id uuid,
  p_workspace_id uuid
)
RETURNS TABLE (
  snapshot_sequence bigint,
  object_key text,
  format_version smallint,
  payload_bytes bigint,
  payload_sha256 text,
  record_count integer,
  key_id text,
  head_sequence bigint,
  cloud_authority_after bigint
)
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = ''
AS $$
DECLARE
  v_compacted_through bigint;
  v_cloud_authority_after bigint;
  v_head_sequence bigint;
BEGIN
  IF p_actor_user_id IS NULL OR p_workspace_id IS NULL THEN
    RAISE EXCEPTION 'E2EE snapshot read request is invalid' USING ERRCODE = '22023';
  END IF;

  IF private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id) IS NULL THEN
    RAISE EXCEPTION 'E2EE snapshot access is not permitted' USING ERRCODE = '42501';
  END IF;

  SELECT workspace.e2ee_compacted_through, workspace.e2ee_cloud_authority_after
  INTO v_compacted_through, v_cloud_authority_after
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;

  SELECT GREATEST(
    COALESCE(v_compacted_through, 0::bigint),
    COALESCE((
      SELECT MAX(event.sequence)
      FROM public.e2ee_freshness_events AS event
      WHERE event.workspace_id = p_workspace_id
    ), 0::bigint)
  )::bigint
  INTO v_head_sequence;

  RETURN QUERY
  SELECT
    snap.snapshot_sequence,
    snap.object_key,
    snap.format_version,
    snap.payload_bytes,
    snap.payload_sha256,
    snap.record_count,
    snap.key_id,
    v_head_sequence,
    v_cloud_authority_after
  FROM public.e2ee_workspace_snapshots AS snap
  WHERE snap.workspace_id = p_workspace_id
  ORDER BY snap.snapshot_sequence DESC
  LIMIT 1;
END;
$$;

REVOKE ALL ON FUNCTION public.read_latest_e2ee_snapshot(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_latest_e2ee_snapshot(uuid, uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION public.commit_e2ee_snapshot(
  p_actor_user_id uuid,
  p_workspace_id uuid,
  p_snapshot_sequence bigint,
  p_object_key text,
  p_format_version smallint,
  p_payload_bytes bigint,
  p_payload_sha256 text,
  p_record_count integer,
  p_key_id text
)
RETURNS TABLE (
  committed boolean,
  retired_object_keys text[]
)
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = ''
AS $$
DECLARE
  v_workspace public.workspaces%ROWTYPE;
  v_active_key_id text;
  v_head_sequence bigint;
  v_retired text[];
BEGIN
  IF p_actor_user_id IS NULL
     OR p_workspace_id IS NULL
     OR p_snapshot_sequence IS NULL
     OR p_snapshot_sequence <= 0
     OR COALESCE(p_object_key, '') !~ (
       '^'
       || p_workspace_id::text
       || '/[0-9]+-[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.ans1$'
     )
     OR p_format_version IS NULL
     OR p_format_version <> 1
     OR p_payload_bytes IS NULL
     OR p_payload_bytes NOT BETWEEN 1 AND 545259520
     OR COALESCE(p_payload_sha256, '') !~ '^[0-9a-f]{64}$'
     OR p_record_count IS NULL
     OR p_record_count < 0
     OR COALESCE(p_key_id, '') !~ '^[A-Za-z0-9_-]{22}$'
  THEN
    RAISE EXCEPTION 'Invalid E2EE snapshot commit request' USING ERRCODE = '22023';
  END IF;

  SELECT * INTO v_workspace
  FROM public.workspaces
  WHERE id = p_workspace_id
  FOR UPDATE;

  v_active_key_id := private.active_e2ee_freshness_key_id(p_actor_user_id, p_workspace_id);
  IF v_workspace.id IS NULL OR v_active_key_id IS NULL THEN
    RAISE EXCEPTION 'E2EE snapshot publication is not permitted' USING ERRCODE = '42501';
  END IF;

  IF v_workspace.kind::text = 'shared' AND p_key_id IS DISTINCT FROM v_active_key_id THEN
    RAISE EXCEPTION 'Invalid E2EE snapshot key identity' USING ERRCODE = '22023';
  END IF;

  v_head_sequence := GREATEST(
    COALESCE(v_workspace.e2ee_compacted_through, 0::bigint),
    COALESCE((
      SELECT MAX(event.sequence)
      FROM public.e2ee_freshness_events AS event
      WHERE event.workspace_id = p_workspace_id
    ), 0::bigint)
  );

  IF v_workspace.e2ee_freshness_initialized_at IS NULL OR p_snapshot_sequence > v_head_sequence THEN
    RAISE EXCEPTION 'E2EE snapshot sequence exceeds witness head' USING ERRCODE = '22023';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public.e2ee_workspace_snapshots AS existing
    WHERE existing.workspace_id = p_workspace_id
      AND existing.snapshot_sequence >= p_snapshot_sequence
  ) THEN
    IF EXISTS (
      SELECT 1
      FROM public.e2ee_workspace_snapshots AS existing
      WHERE existing.workspace_id = p_workspace_id
        AND existing.object_key = p_object_key
    ) THEN
      RETURN QUERY SELECT false, ARRAY[]::text[];
    ELSE
      RETURN QUERY SELECT false, ARRAY[p_object_key];
    END IF;
    RETURN;
  END IF;

  INSERT INTO public.e2ee_workspace_snapshots (
    workspace_id,
    snapshot_sequence,
    object_key,
    format_version,
    payload_bytes,
    payload_sha256,
    record_count,
    key_id,
    created_by
  ) VALUES (
    p_workspace_id,
    p_snapshot_sequence,
    p_object_key,
    p_format_version,
    p_payload_bytes,
    p_payload_sha256,
    p_record_count,
    p_key_id,
    p_actor_user_id
  )
  ON CONFLICT DO NOTHING;

  WITH ranked AS (
    SELECT snap.snapshot_sequence
    FROM public.e2ee_workspace_snapshots AS snap
    WHERE snap.workspace_id = p_workspace_id
    ORDER BY snap.snapshot_sequence DESC
    OFFSET 2
  ),
  deleted AS (
    DELETE FROM public.e2ee_workspace_snapshots AS snap
    USING ranked
    WHERE snap.workspace_id = p_workspace_id
      AND snap.snapshot_sequence = ranked.snapshot_sequence
    RETURNING snap.object_key
  )
  SELECT COALESCE(array_agg(deleted.object_key), ARRAY[]::text[])
  INTO v_retired
  FROM deleted;

  RETURN QUERY SELECT true, v_retired;
END;
$$;

REVOKE ALL ON FUNCTION public.commit_e2ee_snapshot(
  uuid, uuid, bigint, text, smallint, bigint, text, integer, text
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.commit_e2ee_snapshot(
  uuid, uuid, bigint, text, smallint, bigint, text, integer, text
) TO service_role;

CREATE OR REPLACE FUNCTION public.prune_e2ee_compacted_events(
  p_workspace_id uuid,
  p_retain_tail_events bigint DEFAULT 2000,
  p_batch integer DEFAULT 5000
)
RETURNS TABLE (
  compacted_through bigint,
  deleted_rows integer
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_workspace public.workspaces%ROWTYPE;
  v_latest_snapshot bigint;
  v_head bigint;
  v_target bigint;
  v_compacted bigint;
  v_deleted integer := 0;
BEGIN
  IF p_workspace_id IS NULL
     OR p_retain_tail_events IS NULL
     OR p_retain_tail_events < 0
     OR p_batch IS NULL
     OR p_batch NOT BETWEEN 1 AND 50000
  THEN
    RAISE EXCEPTION 'Invalid E2EE event compaction parameters' USING ERRCODE = '22023';
  END IF;

  SELECT * INTO v_workspace
  FROM public.workspaces
  WHERE id = p_workspace_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'Workspace not found' USING ERRCODE = '22023';
  END IF;

  SELECT snap.snapshot_sequence
  INTO v_latest_snapshot
  FROM public.e2ee_workspace_snapshots AS snap
  WHERE snap.workspace_id = p_workspace_id
  ORDER BY snap.snapshot_sequence DESC
  LIMIT 1;

  IF v_latest_snapshot IS NULL THEN
    RETURN QUERY SELECT COALESCE(v_workspace.e2ee_compacted_through, 0::bigint), 0;
    RETURN;
  END IF;

  v_head := GREATEST(
    COALESCE(v_workspace.e2ee_compacted_through, 0::bigint),
    COALESCE((
      SELECT MAX(event.sequence)
      FROM public.e2ee_freshness_events AS event
      WHERE event.workspace_id = p_workspace_id
    ), 0::bigint)
  );

  v_target := LEAST(v_latest_snapshot, GREATEST(0::bigint, v_head - p_retain_tail_events));
  v_compacted := GREATEST(COALESCE(v_workspace.e2ee_compacted_through, 0::bigint), v_target);

  IF v_compacted > 0 AND v_compacted IS DISTINCT FROM v_workspace.e2ee_compacted_through THEN
    UPDATE public.workspaces
    SET e2ee_compacted_through = v_compacted,
        updated_at = now()
    WHERE id = p_workspace_id;
  END IF;

  IF v_compacted <= 0 THEN
    RETURN QUERY SELECT 0::bigint, 0;
    RETURN;
  END IF;

  WITH batch AS (
    SELECT event.sequence
    FROM public.e2ee_freshness_events AS event
    WHERE event.workspace_id = p_workspace_id
      AND event.sequence <= v_compacted
    ORDER BY event.sequence
    LIMIT p_batch
  )
  DELETE FROM public.e2ee_freshness_events AS event
  USING batch
  WHERE event.workspace_id = p_workspace_id
    AND event.sequence = batch.sequence;

  GET DIAGNOSTICS v_deleted = ROW_COUNT;
  RETURN QUERY SELECT v_compacted, v_deleted;
END;
$$;

REVOKE ALL ON FUNCTION public.prune_e2ee_compacted_events(uuid, bigint, integer)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.prune_e2ee_compacted_events(uuid, bigint, integer)
  TO service_role;

COMMIT;
