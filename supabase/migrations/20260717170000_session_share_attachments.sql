INSERT INTO storage.buckets (
  id,
  name,
  public,
  file_size_limit,
  allowed_mime_types
)
VALUES (
  'shared-note-attachments',
  'shared-note-attachments',
  false,
  536870912,
  NULL
)
ON CONFLICT (id) DO UPDATE SET
  name = excluded.name,
  public = excluded.public,
  file_size_limit = excluded.file_size_limit,
  allowed_mime_types = excluded.allowed_mime_types;

DROP POLICY IF EXISTS shared_note_attachments_deny_client_select
  ON storage.objects;
CREATE POLICY shared_note_attachments_deny_client_select
  ON storage.objects
  AS RESTRICTIVE
  FOR SELECT
  TO anon, authenticated
  USING (bucket_id <> 'shared-note-attachments');

DROP POLICY IF EXISTS shared_note_attachments_deny_client_insert
  ON storage.objects;
CREATE POLICY shared_note_attachments_deny_client_insert
  ON storage.objects
  AS RESTRICTIVE
  FOR INSERT
  TO anon, authenticated
  WITH CHECK (bucket_id <> 'shared-note-attachments');

DROP POLICY IF EXISTS shared_note_attachments_deny_client_update
  ON storage.objects;
CREATE POLICY shared_note_attachments_deny_client_update
  ON storage.objects
  AS RESTRICTIVE
  FOR UPDATE
  TO anon, authenticated
  USING (bucket_id <> 'shared-note-attachments')
  WITH CHECK (bucket_id <> 'shared-note-attachments');

DROP POLICY IF EXISTS shared_note_attachments_deny_client_delete
  ON storage.objects;
CREATE POLICY shared_note_attachments_deny_client_delete
  ON storage.objects
  AS RESTRICTIVE
  FOR DELETE
  TO anon, authenticated
  USING (bucket_id <> 'shared-note-attachments');

CREATE TABLE public.session_share_attachment_objects (
  id uuid PRIMARY KEY,
  share_id uuid NOT NULL,
  owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE RESTRICT,
  attachment_ref text NOT NULL,
  version_ref text NOT NULL,
  object_key text NOT NULL,
  filename text NOT NULL,
  content_type text NOT NULL,
  size_bytes bigint NOT NULL,
  sha256 text,
  state text NOT NULL DEFAULT 'reserved',
  reservation_expires_at timestamptz NOT NULL,
  last_signed_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz NOT NULL,
  finalized_at timestamptz,
  deletion_requested_at timestamptz,
  gc_lease_id uuid,
  gc_lease_expires_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT session_share_attachment_objects_share_fk
    FOREIGN KEY (share_id)
    REFERENCES public.session_shares(id)
    ON DELETE RESTRICT,
  CONSTRAINT session_share_attachment_objects_share_id_key
    UNIQUE (share_id, id),
  CONSTRAINT session_share_attachment_objects_share_version_key
    UNIQUE (share_id, version_ref),
  CONSTRAINT session_share_attachment_objects_object_key_key
    UNIQUE (object_key),
  CONSTRAINT session_share_attachment_objects_ref_check CHECK (
    attachment_ref ~ '^[A-Za-z0-9_-]{43}$'
    AND version_ref ~ '^[A-Za-z0-9_-]{43}$'
    AND version_ref <> attachment_ref
  ),
  CONSTRAINT session_share_attachment_objects_key_check CHECK (
    object_key = (
      owner_user_id::text || '/' || share_id::text || '/' || id::text || '.sna1'
    )
  ),
  CONSTRAINT session_share_attachment_objects_filename_check CHECK (
    filename = btrim(filename)
    AND filename <> ''
    AND filename !~ '[\\/[:cntrl:]]'
    AND octet_length(filename) <= 1024
  ),
  CONSTRAINT session_share_attachment_objects_content_type_check CHECK (
    content_type = lower(btrim(content_type))
    AND content_type ~ '^[a-z0-9][a-z0-9!#$&^_.+-]*/[a-z0-9][a-z0-9!#$&^_.+-]*$'
    AND octet_length(content_type) <= 255
  ),
  CONSTRAINT session_share_attachment_objects_size_check CHECK (
    size_bytes BETWEEN 1 AND 536870912
  ),
  CONSTRAINT session_share_attachment_objects_sha_check CHECK (
    sha256 IS NULL OR sha256 ~ '^[0-9a-f]{64}$'
  ),
  CONSTRAINT session_share_attachment_objects_state_check CHECK (
    state IN ('reserved', 'ready', 'deleting')
  ),
  CONSTRAINT session_share_attachment_objects_lifecycle_check CHECK (
    (
      state = 'reserved'
      AND finalized_at IS NULL
      AND deletion_requested_at IS NULL
    )
    OR (
      state = 'ready'
      AND finalized_at IS NOT NULL
      AND deletion_requested_at IS NULL
      AND sha256 IS NOT NULL
    )
    OR (
      state = 'deleting'
      AND deletion_requested_at IS NOT NULL
    )
  ),
  CONSTRAINT session_share_attachment_objects_signing_check CHECK (
    (last_signed_at IS NULL AND upload_expires_at IS NULL)
    OR (
      last_signed_at IS NOT NULL
      AND upload_expires_at IS NOT NULL
      AND upload_expires_at > last_signed_at
    )
  ),
  CONSTRAINT session_share_attachment_objects_lease_check CHECK (
    (gc_lease_id IS NULL AND gc_lease_expires_at IS NULL)
    OR (gc_lease_id IS NOT NULL AND gc_lease_expires_at IS NOT NULL)
  ),
  CONSTRAINT session_share_attachment_objects_time_check CHECK (
    reservation_expires_at > created_at
    AND cleanup_not_before >= reservation_expires_at
    AND (
      upload_expires_at IS NULL
      OR cleanup_not_before >= upload_expires_at + interval '24 hours 5 minutes'
    )
    AND updated_at >= created_at
  )
);

CREATE INDEX session_share_attachment_objects_gc_idx
  ON public.session_share_attachment_objects(
    cleanup_not_before,
    gc_lease_expires_at,
    created_at
  );

CREATE INDEX session_share_attachment_objects_owner_idx
  ON public.session_share_attachment_objects(owner_user_id, share_id, state);

CREATE TABLE public.session_share_snapshot_attachments (
  share_id uuid NOT NULL,
  attachment_id uuid NOT NULL,
  position smallint NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (share_id, attachment_id),
  CONSTRAINT session_share_snapshot_attachments_position_key
    UNIQUE (share_id, position),
  CONSTRAINT session_share_snapshot_attachments_snapshot_fk
    FOREIGN KEY (share_id)
    REFERENCES public.session_share_snapshots(share_id)
    ON DELETE RESTRICT,
  CONSTRAINT session_share_snapshot_attachments_object_fk
    FOREIGN KEY (share_id, attachment_id)
    REFERENCES public.session_share_attachment_objects(share_id, id)
    ON DELETE RESTRICT,
  CONSTRAINT session_share_snapshot_attachments_position_check CHECK (
    position BETWEEN 0 AND 63
  )
);

ALTER TABLE public.session_share_attachment_objects ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_share_snapshot_attachments ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_share_attachment_objects
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_share_snapshot_attachments
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.session_share_attachment_objects TO service_role;
GRANT ALL ON TABLE public.session_share_snapshot_attachments TO service_role;

CREATE POLICY session_share_attachment_objects_service_all
  ON public.session_share_attachment_objects
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_share_snapshot_attachments_service_all
  ON public.session_share_snapshot_attachments
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.require_session_share_attachment_manager(
  p_share_id uuid,
  p_actor_user_id uuid
)
RETURNS public.session_shares
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF p_share_id IS NULL OR p_actor_user_id IS NULL THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND membership.user_id = p_actor_user_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE OF share, workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_share;
END;
$$;

REVOKE ALL ON FUNCTION private.require_session_share_attachment_manager(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.require_session_share_attachment_manager(uuid, uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION public.reserve_session_share_attachment(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_attachment_ref text,
  p_version_ref text,
  p_filename text,
  p_content_type text,
  p_size_bytes bigint
)
RETURNS TABLE (
  attachment_id uuid,
  object_key text,
  object_state text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  reservation_expires_at timestamptz,
  cleanup_not_before timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_share public.session_shares%ROWTYPE;
  v_owner_user_id uuid;
  v_object public.session_share_attachment_objects%ROWTYPE;
  v_object_id uuid;
BEGIN
  IF p_attachment_ref IS NULL
    OR p_attachment_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref IS NULL
    OR p_version_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref = p_attachment_ref
    OR p_filename IS NULL
    OR p_filename <> btrim(p_filename)
    OR p_filename = ''
    OR p_filename ~ '[\\/[:cntrl:]]'
    OR octet_length(p_filename) > 1024
    OR p_content_type IS NULL
    OR p_content_type <> lower(btrim(p_content_type))
    OR p_content_type !~ '^[a-z0-9][a-z0-9!#$&^_.+-]*/[a-z0-9][a-z0-9!#$&^_.+-]*$'
    OR octet_length(p_content_type) > 255
    OR p_size_bytes IS NULL
    OR p_size_bytes NOT BETWEEN 1 AND 536870912
  THEN
    RAISE EXCEPTION 'invalid shared attachment metadata'
      USING ERRCODE = '22023';
  END IF;

  v_share := private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  SELECT workspace.owner_user_id
  INTO STRICT v_owner_user_id
  FROM public.workspaces AS workspace
  WHERE workspace.id = v_share.workspace_id
    AND workspace.deleted_at IS NULL;

  SELECT attachment.*
  INTO v_object
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.share_id = v_share.id
    AND attachment.version_ref = p_version_ref
  FOR UPDATE;

  IF FOUND THEN
    IF v_object.owner_user_id <> v_owner_user_id
      OR v_object.attachment_ref <> p_attachment_ref
      OR v_object.filename <> p_filename
      OR v_object.content_type <> p_content_type
      OR v_object.size_bytes <> p_size_bytes
    THEN
      RAISE EXCEPTION 'shared attachment reservation conflicts'
        USING ERRCODE = '40001';
    END IF;

    RETURN QUERY SELECT
      v_object.id,
      v_object.object_key,
      v_object.state,
      v_object.filename,
      v_object.content_type,
      v_object.size_bytes,
      v_object.sha256,
      v_object.reservation_expires_at,
      v_object.cleanup_not_before,
      false;
    RETURN;
  END IF;

  IF (
    SELECT count(*)
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.share_id = v_share.id
  ) >= 256
    OR COALESCE((
      SELECT sum(attachment.size_bytes)
      FROM public.session_share_attachment_objects AS attachment
      WHERE attachment.share_id = v_share.id
    ), 0) + p_size_bytes > 2147483648
  THEN
    RAISE EXCEPTION 'shared attachment quota exhausted'
      USING ERRCODE = '54000';
  END IF;

  v_object_id := gen_random_uuid();
  INSERT INTO public.session_share_attachment_objects (
    id,
    share_id,
    owner_user_id,
    attachment_ref,
    version_ref,
    object_key,
    filename,
    content_type,
    size_bytes,
    reservation_expires_at,
    cleanup_not_before,
    created_at,
    updated_at
  ) VALUES (
    v_object_id,
    v_share.id,
    v_owner_user_id,
    p_attachment_ref,
    p_version_ref,
    v_owner_user_id::text || '/' || v_share.id::text || '/' || v_object_id::text || '.sna1',
    p_filename,
    p_content_type,
    p_size_bytes,
    v_now + interval '15 minutes',
    v_now + interval '15 minutes',
    v_now,
    v_now
  )
  RETURNING * INTO v_object;

  RETURN QUERY SELECT
    v_object.id,
    v_object.object_key,
    v_object.state,
    v_object.filename,
    v_object.content_type,
    v_object.size_bytes,
    v_object.sha256,
    v_object.reservation_expires_at,
    v_object.cleanup_not_before,
    true;
END;
$$;

CREATE OR REPLACE FUNCTION public.read_session_share_attachment_by_key(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_object_key text
)
RETURNS TABLE (
  attachment_id uuid,
  object_key text,
  object_state text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  reservation_expires_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_owner_user_id uuid;
BEGIN
  v_share := private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  SELECT workspace.owner_user_id
  INTO STRICT v_owner_user_id
  FROM public.workspaces AS workspace
  WHERE workspace.id = v_share.workspace_id
    AND workspace.deleted_at IS NULL;

  IF p_object_key IS NULL
    OR p_object_key !~ (
      '^' || v_owner_user_id::text || '/' || v_share.id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.sna1$'
    )
  THEN
    RAISE EXCEPTION 'invalid shared attachment object key'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY SELECT
    attachment.id,
    attachment.object_key,
    attachment.state,
    attachment.filename,
    attachment.content_type,
    attachment.size_bytes,
    attachment.sha256,
    attachment.reservation_expires_at,
    attachment.upload_expires_at,
    attachment.cleanup_not_before
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.share_id = v_share.id
    AND attachment.owner_user_id = v_owner_user_id
    AND attachment.object_key = p_object_key;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_session_share_attachment_signed(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_attachment_id uuid,
  p_upload_expires_at timestamptz,
  p_sha256 text
)
RETURNS TABLE (
  attachment_id uuid,
  object_key text,
  object_state text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_object public.session_share_attachment_objects%ROWTYPE;
BEGIN
  PERFORM private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  IF p_upload_expires_at IS NULL
    OR p_upload_expires_at <= v_now
    OR p_upload_expires_at > v_now + interval '2 hours 5 minutes'
    OR p_sha256 IS NULL
    OR p_sha256 !~ '^[0-9a-f]{64}$'
  THEN
    RAISE EXCEPTION 'invalid shared attachment upload grant'
      USING ERRCODE = '22023';
  END IF;

  SELECT attachment.*
  INTO v_object
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = p_attachment_id
    AND attachment.share_id = p_share_id
  FOR UPDATE;

  IF NOT FOUND
    OR v_object.state <> 'reserved'
    OR v_object.reservation_expires_at <= v_now
    OR (v_object.sha256 IS NOT NULL AND v_object.sha256 <> p_sha256)
  THEN
    RAISE EXCEPTION 'shared attachment reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    sha256 = p_sha256,
    last_signed_at = v_now,
    upload_expires_at = p_upload_expires_at,
    cleanup_not_before = GREATEST(
      attachment.cleanup_not_before,
      p_upload_expires_at + interval '24 hours 5 minutes'
    ),
    updated_at = v_now
  WHERE attachment.id = v_object.id
  RETURNING * INTO v_object;

  RETURN QUERY SELECT
    v_object.id,
    v_object.object_key,
    v_object.state,
    v_object.filename,
    v_object.content_type,
    v_object.size_bytes,
    v_object.sha256,
    v_object.upload_expires_at,
    v_object.cleanup_not_before;
END;
$$;

CREATE OR REPLACE FUNCTION public.finalize_session_share_attachment(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_attachment_id uuid,
  p_object_key text,
  p_observed_size_bytes bigint,
  p_observed_content_type text
)
RETURNS TABLE (
  attachment_id uuid,
  object_key text,
  object_state text,
  was_finalized boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_object public.session_share_attachment_objects%ROWTYPE;
BEGIN
  PERFORM private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  SELECT attachment.*
  INTO v_object
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = p_attachment_id
    AND attachment.share_id = p_share_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'shared attachment reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  IF p_object_key IS DISTINCT FROM v_object.object_key
    OR p_observed_size_bytes IS DISTINCT FROM v_object.size_bytes
    OR p_observed_content_type IS DISTINCT FROM v_object.content_type
  THEN
    RAISE EXCEPTION 'shared attachment object does not match reservation'
      USING ERRCODE = '22023';
  END IF;

  IF v_object.state = 'ready' THEN
    RETURN QUERY SELECT v_object.id, v_object.object_key, v_object.state, false;
    RETURN;
  END IF;

  IF v_object.state <> 'reserved'
    OR v_object.sha256 IS NULL
    OR v_object.last_signed_at IS NULL
    OR v_object.cleanup_not_before <= v_now
  THEN
    RAISE EXCEPTION 'shared attachment reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'ready',
    finalized_at = v_now,
    cleanup_not_before = GREATEST(
      attachment.cleanup_not_before,
      v_now + interval '24 hours'
    ),
    updated_at = v_now
  WHERE attachment.id = v_object.id
  RETURNING * INTO v_object;

  RETURN QUERY SELECT v_object.id, v_object.object_key, v_object.state, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.session_share_attachment_manifest(
  p_share_id uuid
)
RETURNS jsonb
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT COALESCE(
    jsonb_agg(
      jsonb_build_object(
        'id', attachment.id,
        'filename', attachment.filename,
        'contentType', attachment.content_type,
        'sizeBytes', attachment.size_bytes,
        'sha256', attachment.sha256
      ) ORDER BY binding.position
    ),
    '[]'::jsonb
  )
  FROM public.session_share_snapshot_attachments AS binding
  JOIN public.session_share_attachment_objects AS attachment
    ON attachment.share_id = binding.share_id
    AND attachment.id = binding.attachment_id
  WHERE binding.share_id = p_share_id
    AND attachment.state = 'ready';
$$;

CREATE OR REPLACE FUNCTION private.publish_session_share_snapshot_with_attachments(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[]
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_title text := btrim(p_title);
  v_attachment_ids uuid[] := COALESCE(p_attachment_ids, ARRAY[]::uuid[]);
  v_retired_ids uuid[];
  v_snapshot public.session_share_snapshots%ROWTYPE;
BEGIN
  BEGIN
    PERFORM private.require_session_share_attachment_manager(
      p_share_id,
      p_actor_user_id
    );
  EXCEPTION WHEN insufficient_privilege THEN
    RAISE EXCEPTION 'session snapshot publication not permitted'
      USING ERRCODE = '42501';
  END;

  IF v_title IS NULL
    OR octet_length(v_title) > 4096
    OR p_body_json IS NULL
    OR jsonb_typeof(p_body_json) <> 'object'
    OR p_body_json ->> 'type' <> 'doc'
    OR octet_length(p_body_json::text) > 2097152
    OR cardinality(v_attachment_ids) > 64
    OR EXISTS (
      SELECT 1
      FROM unnest(v_attachment_ids) AS requested(id)
      WHERE requested.id IS NULL
    )
    OR cardinality(v_attachment_ids) <> (
      SELECT count(DISTINCT requested.id)
      FROM unnest(v_attachment_ids) AS requested(id)
    )
  THEN
    RAISE EXCEPTION 'invalid session share snapshot'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = ANY(v_attachment_ids)
  FOR UPDATE;

  IF cardinality(v_attachment_ids) <> (
    SELECT count(*)
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.id = ANY(v_attachment_ids)
      AND attachment.share_id = p_share_id
      AND attachment.state = 'ready'
      AND attachment.sha256 IS NOT NULL
  ) THEN
    RAISE EXCEPTION 'shared snapshot attachment is unavailable'
      USING ERRCODE = '55000';
  END IF;

  SELECT array_agg(binding.attachment_id)
  INTO v_retired_ids
  FROM public.session_share_snapshot_attachments AS binding
  WHERE binding.share_id = p_share_id
    AND NOT (binding.attachment_id = ANY(v_attachment_ids));

  INSERT INTO public.session_share_snapshots (
    share_id,
    schema_version,
    content_revision,
    title,
    body_json,
    published_by_user_id,
    published_at,
    updated_at
  ) VALUES (
    p_share_id,
    1,
    1,
    v_title,
    p_body_json,
    p_actor_user_id,
    v_now,
    v_now
  )
  ON CONFLICT ON CONSTRAINT session_share_snapshots_pkey DO UPDATE SET
    schema_version = 1,
    content_revision = public.session_share_snapshots.content_revision + 1,
    title = excluded.title,
    body_json = excluded.body_json,
    published_by_user_id = excluded.published_by_user_id,
    published_at = excluded.published_at,
    updated_at = excluded.updated_at
  RETURNING * INTO v_snapshot;

  DELETE FROM public.session_share_snapshot_attachments AS binding
  WHERE binding.share_id = p_share_id;

  INSERT INTO public.session_share_snapshot_attachments (
    share_id,
    attachment_id,
    position,
    created_at
  )
  SELECT
    p_share_id,
    requested.id,
    (requested.ordinality - 1)::smallint,
    v_now
  FROM unnest(v_attachment_ids) WITH ORDINALITY AS requested(id, ordinality);

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
    gc_lease_id = NULL,
    gc_lease_expires_at = NULL,
    updated_at = v_now
  WHERE attachment.id = ANY(COALESCE(v_retired_ids, ARRAY[]::uuid[]))
    AND attachment.share_id = p_share_id
    AND attachment.state <> 'deleting';

  PERFORM private.write_session_access_event(
    p_share_id,
    'snapshot_published',
    p_actor_user_id,
    NULL,
    p_share_id,
    CASE
      WHEN v_snapshot.content_revision > 1
        THEN (v_snapshot.content_revision - 1)::text
      ELSE NULL
    END,
    v_snapshot.content_revision::text
  );

  RETURN QUERY SELECT
    v_snapshot.share_id,
    v_snapshot.schema_version,
    v_snapshot.content_revision,
    v_snapshot.title,
    v_snapshot.body_json,
    private.session_share_attachment_manifest(v_snapshot.share_id),
    v_snapshot.published_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.publish_session_share_snapshot(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_title text,
  p_body_json jsonb
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    published.share_id,
    published.schema_version,
    published.content_revision,
    published.title,
    published.body_json,
    published.published_at
  FROM private.publish_session_share_snapshot_with_attachments(
    p_share_id,
    p_actor_user_id,
    p_title,
    p_body_json,
    ARRAY[]::uuid[]
  ) AS published;
$$;

CREATE OR REPLACE FUNCTION public.publish_session_share_snapshot_with_attachments(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[]
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.publish_session_share_snapshot_with_attachments(
    p_share_id,
    p_actor_user_id,
    p_title,
    p_body_json,
    p_attachment_ids
  );
$$;

REVOKE ALL ON FUNCTION private.session_share_attachment_manifest(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.publish_session_share_snapshot_with_attachments(
  uuid, uuid, text, jsonb, uuid[]
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.publish_session_share_snapshot_with_attachments(
  uuid, uuid, text, jsonb, uuid[]
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.session_share_attachment_manifest(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.publish_session_share_snapshot_with_attachments(
  uuid, uuid, text, jsonb, uuid[]
) TO service_role;
GRANT EXECUTE ON FUNCTION public.publish_session_share_snapshot_with_attachments(
  uuid, uuid, text, jsonb, uuid[]
) TO service_role;

CREATE OR REPLACE FUNCTION private.gateway_read_session_share_link_snapshot_v2(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.published_at
  FROM private.gateway_read_session_share_link_snapshot(
    p_share_id,
    p_link_token
  ) AS snapshot;
$$;

CREATE OR REPLACE FUNCTION private.gateway_read_public_session_share_snapshot_v2(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.published_at
  FROM private.gateway_read_public_session_share_snapshot(p_public_slug) AS snapshot;
$$;

CREATE OR REPLACE FUNCTION private.gateway_claim_session_share_handoff_v2(
  p_request_id text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  attachment_downloads_json jsonb,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_claimed record;
  v_now timestamptz := clock_timestamp();
  v_downloads jsonb;
BEGIN
  SELECT snapshot.*
  INTO v_claimed
  FROM private.gateway_claim_session_share_handoff(p_request_id) AS snapshot;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    cleanup_not_before = GREATEST(
      attachment.cleanup_not_before,
      v_now + interval '6 minutes 10 seconds'
    ),
    updated_at = v_now
  FROM public.session_share_snapshot_attachments AS binding
  WHERE binding.share_id = v_claimed.share_id
    AND attachment.share_id = binding.share_id
    AND attachment.id = binding.attachment_id
    AND attachment.state = 'ready';

  SELECT COALESCE(
    jsonb_agg(
      jsonb_build_object(
        'share_id', attachment.share_id,
        'attachment_id', attachment.id,
        'object_key', attachment.object_key,
        'filename', attachment.filename,
        'content_type', attachment.content_type,
        'size_bytes', attachment.size_bytes,
        'sha256', attachment.sha256,
        'access_version', share.access_version,
        'cleanup_not_before', attachment.cleanup_not_before
      ) ORDER BY binding.position
    ),
    '[]'::jsonb
  )
  INTO v_downloads
  FROM public.session_share_snapshot_attachments AS binding
  JOIN public.session_share_attachment_objects AS attachment
    ON attachment.share_id = binding.share_id
    AND attachment.id = binding.attachment_id
  JOIN public.session_shares AS share
    ON share.id = binding.share_id
  WHERE binding.share_id = v_claimed.share_id
    AND attachment.state = 'ready';

  RETURN QUERY SELECT
    v_claimed.share_id,
    v_claimed.schema_version,
    v_claimed.content_revision,
    v_claimed.title,
    v_claimed.body_json,
    private.session_share_attachment_manifest(v_claimed.share_id),
    v_downloads,
    v_claimed.published_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_session_share_link_snapshot_v2(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_session_share_link_snapshot_v2(
    p_share_id,
    p_link_token
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_public_session_share_snapshot_v2(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_public_session_share_snapshot_v2(p_public_slug);
$$;

CREATE OR REPLACE FUNCTION public.gateway_claim_session_share_handoff_v2(
  p_request_id text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  attachment_downloads_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_claim_session_share_handoff_v2(p_request_id);
$$;

REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_snapshot_v2(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.gateway_read_public_session_share_snapshot_v2(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.gateway_claim_session_share_handoff_v2(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_snapshot_v2(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_read_public_session_share_snapshot_v2(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_claim_session_share_handoff_v2(text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_snapshot_v2(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_public_session_share_snapshot_v2(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_claim_session_share_handoff_v2(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_snapshot_v2(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_public_session_share_snapshot_v2(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_claim_session_share_handoff_v2(text)
  TO service_role;

CREATE OR REPLACE FUNCTION private.prepare_session_share_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_share public.session_shares%ROWTYPE;
  v_object public.session_share_attachment_objects%ROWTYPE;
BEGIN
  IF p_download_expires_at IS NULL
    OR p_download_expires_at <= v_now
    OR p_download_expires_at > v_now + interval '65 seconds'
  THEN
    RAISE EXCEPTION 'invalid shared attachment download request'
      USING ERRCODE = '22023';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT attachment.*
  INTO v_object
  FROM public.session_share_attachment_objects AS attachment
  JOIN public.session_share_snapshot_attachments AS binding
    ON binding.share_id = attachment.share_id
    AND binding.attachment_id = attachment.id
  WHERE attachment.share_id = v_share.id
    AND attachment.id = p_attachment_id
    AND attachment.state = 'ready'
  FOR UPDATE OF attachment;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    cleanup_not_before = GREATEST(
      attachment.cleanup_not_before,
      p_download_expires_at + interval '5 minutes'
    ),
    updated_at = v_now
  WHERE attachment.id = v_object.id
  RETURNING * INTO v_object;

  RETURN QUERY SELECT
    v_share.id,
    v_object.id,
    v_object.object_key,
    v_object.filename,
    v_object.content_type,
    v_object.size_bytes,
    v_object.sha256,
    v_share.access_version,
    v_object.cleanup_not_before;
END;
$$;

CREATE OR REPLACE FUNCTION private.prepare_my_session_share_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_actor_user_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS actor
    WHERE actor.id = p_actor_user_id
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) OR NOT EXISTS (
    SELECT 1
    FROM public.session_shares AS share
    JOIN public.workspaces AS source_workspace
      ON source_workspace.id = share.workspace_id
    WHERE share.id = p_share_id
      AND share.deleted_at IS NULL
      AND source_workspace.deleted_at IS NULL
      AND (
        EXISTS (
          SELECT 1
          FROM public.workspace_memberships AS source_membership
          WHERE source_membership.workspace_id = share.workspace_id
            AND source_membership.user_id = p_actor_user_id
            AND source_membership.role IN ('owner', 'admin')
            AND source_membership.deleted_at IS NULL
        )
        OR EXISTS (
          SELECT 1
          FROM public.session_access_grants AS access_grant
          WHERE access_grant.share_id = share.id
            AND access_grant.grantee_user_id = p_actor_user_id
            AND access_grant.revoked_at IS NULL
        )
        OR (
          share.general_scope = 'workspace'
          AND EXISTS (
            SELECT 1
            FROM public.workspaces AS target_workspace
            JOIN public.workspace_memberships AS target_membership
              ON target_membership.workspace_id = target_workspace.id
            WHERE target_workspace.id = share.general_workspace_id
              AND target_workspace.deleted_at IS NULL
              AND target_membership.user_id = p_actor_user_id
              AND target_membership.deleted_at IS NULL
          )
        )
        OR share.general_scope = 'public'
      )
  ) THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT *
  FROM private.prepare_session_share_attachment_download(
    p_share_id,
    p_attachment_id,
    p_download_expires_at
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.prepare_my_session_share_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_actor_user_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.prepare_my_session_share_attachment_download(
    p_share_id,
    p_attachment_id,
    p_actor_user_id,
    p_download_expires_at
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_prepare_session_share_link_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_link_token text,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  IF p_link_token IS NULL
    OR p_link_token !~ '^[A-Za-z0-9_-]{43}$'
    OR NOT EXISTS (
      SELECT 1
      FROM public.session_shares AS share
      JOIN public.session_share_links AS link
        ON link.share_id = share.id
      WHERE share.id = p_share_id
        AND share.general_scope = 'link'
        AND share.deleted_at IS NULL
        AND link.revoked_at IS NULL
        AND link.token_hash = extensions.digest(p_link_token, 'sha256')
    )
  THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT *
  FROM private.prepare_session_share_attachment_download(
    p_share_id,
    p_attachment_id,
    p_download_expires_at
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_prepare_public_session_share_attachment_download(
  p_public_slug text,
  p_attachment_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_share_id uuid;
BEGIN
  IF p_public_slug IS NULL OR p_public_slug !~ '^s_[0-9a-f]{32}$' THEN
    RETURN;
  END IF;

  SELECT share.id
  INTO v_share_id
  FROM public.session_shares AS share
  WHERE share.public_slug = p_public_slug
    AND share.general_scope = 'public'
    AND share.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT *
  FROM private.prepare_session_share_attachment_download(
    v_share_id,
    p_attachment_id,
    p_download_expires_at
  );
END;
$$;

REVOKE ALL ON FUNCTION private.prepare_session_share_attachment_download(uuid, uuid, timestamptz)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_prepare_session_share_link_attachment_download(
  uuid, uuid, text, timestamptz
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_prepare_public_session_share_attachment_download(
  text, uuid, timestamptz
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.prepare_session_share_attachment_download(uuid, uuid, timestamptz)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION public.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_prepare_session_share_link_attachment_download(
  uuid, uuid, text, timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_prepare_public_session_share_attachment_download(
  text, uuid, timestamptz
) TO service_role;

REVOKE ALL ON FUNCTION public.reserve_session_share_attachment(
  uuid, uuid, text, text, text, text, bigint
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_session_share_attachment_by_key(uuid, uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_session_share_attachment_signed(
  uuid, uuid, uuid, timestamptz, text
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finalize_session_share_attachment(
  uuid, uuid, uuid, text, bigint, text
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.reserve_session_share_attachment(
  uuid, uuid, text, text, text, text, bigint
) TO service_role;
GRANT EXECUTE ON FUNCTION public.read_session_share_attachment_by_key(uuid, uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_session_share_attachment_signed(
  uuid, uuid, uuid, timestamptz, text
) TO service_role;
GRANT EXECUTE ON FUNCTION public.finalize_session_share_attachment(
  uuid, uuid, uuid, text, bigint, text
) TO service_role;

CREATE OR REPLACE FUNCTION public.claim_session_share_attachment_gc_leases(
  p_lease_id uuid,
  p_limit integer DEFAULT 32,
  p_lease_seconds integer DEFAULT 300
)
RETURNS TABLE (
  attachment_id uuid,
  owner_user_id uuid,
  share_id uuid,
  object_key text,
  size_bytes bigint,
  gc_lease_id uuid,
  gc_lease_expires_at timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  IF p_lease_id IS NULL
    OR p_limit IS NULL
    OR p_limit NOT BETWEEN 1 AND 100
    OR p_lease_seconds IS NULL
    OR p_lease_seconds NOT BETWEEN 30 AND 3600
  THEN
    RAISE EXCEPTION 'invalid shared attachment GC lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT attachment.id
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.cleanup_not_before <= v_now
      AND (
        attachment.gc_lease_expires_at IS NULL
        OR attachment.gc_lease_expires_at <= v_now
      )
      AND (
        attachment.state = 'deleting'
        OR (
          attachment.state = 'reserved'
          AND attachment.reservation_expires_at <= v_now
        )
        OR (
          attachment.state = 'ready'
          AND NOT EXISTS (
            SELECT 1
            FROM public.session_share_snapshot_attachments AS binding
            WHERE binding.share_id = attachment.share_id
              AND binding.attachment_id = attachment.id
          )
        )
      )
    ORDER BY attachment.cleanup_not_before, attachment.created_at, attachment.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE public.session_share_attachment_objects AS attachment
    SET
      state = 'deleting',
      deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
      gc_lease_id = p_lease_id,
      gc_lease_expires_at = v_now + make_interval(secs => p_lease_seconds),
      updated_at = v_now
    FROM candidates
    WHERE attachment.id = candidates.id
    RETURNING attachment.*
  )
  SELECT
    leased.id,
    leased.owner_user_id,
    leased.share_id,
    leased.object_key,
    leased.size_bytes,
    leased.gc_lease_id,
    leased.gc_lease_expires_at
  FROM leased
  ORDER BY leased.cleanup_not_before, leased.created_at, leased.id;
END;
$$;

CREATE OR REPLACE FUNCTION public.finish_session_share_attachment_deletion(
  p_attachment_id uuid,
  p_object_key text,
  p_gc_lease_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_object public.session_share_attachment_objects%ROWTYPE;
BEGIN
  SELECT attachment.*
  INTO v_object
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = p_attachment_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF p_object_key IS DISTINCT FROM v_object.object_key
    OR p_gc_lease_id IS NULL
    OR v_object.gc_lease_id IS DISTINCT FROM p_gc_lease_id
    OR v_object.gc_lease_expires_at IS NULL
    OR v_object.gc_lease_expires_at <= v_now
    OR v_object.cleanup_not_before > v_now
    OR EXISTS (
      SELECT 1
      FROM public.session_share_snapshot_attachments AS binding
      WHERE binding.share_id = v_object.share_id
        AND binding.attachment_id = v_object.id
    )
  THEN
    RAISE EXCEPTION 'shared attachment deletion is unavailable'
      USING ERRCODE = '55000';
  END IF;

  DELETE FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = v_object.id;
  RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION public.claim_session_share_attachment_gc_leases(
  uuid, integer, integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finish_session_share_attachment_deletion(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.claim_session_share_attachment_gc_leases(
  uuid, integer, integer
) TO service_role;
GRANT EXECUTE ON FUNCTION public.finish_session_share_attachment_deletion(uuid, text, uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION private.retire_session_share_attachments()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL THEN
    DELETE FROM public.session_share_snapshot_attachments AS binding
    WHERE binding.share_id = NEW.id;

    UPDATE public.session_share_attachment_objects AS attachment
    SET
      state = 'deleting',
      deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
      gc_lease_id = NULL,
      gc_lease_expires_at = NULL,
      updated_at = v_now
    WHERE attachment.share_id = NEW.id;
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.retire_workspace_session_share_attachments()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL THEN
    DELETE FROM public.session_share_snapshot_attachments AS binding
    USING public.session_shares AS share
    WHERE share.workspace_id = NEW.id
      AND binding.share_id = share.id;

    UPDATE public.session_share_attachment_objects AS attachment
    SET
      state = 'deleting',
      deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
      gc_lease_id = NULL,
      gc_lease_expires_at = NULL,
      updated_at = v_now
    FROM public.session_shares AS share
    WHERE share.workspace_id = NEW.id
      AND attachment.share_id = share.id;
  END IF;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.retire_session_share_attachments()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.retire_workspace_session_share_attachments()
  FROM PUBLIC, anon, authenticated;

CREATE TRIGGER on_session_share_attachments_deleted
  AFTER UPDATE OF deleted_at ON public.session_shares
  FOR EACH ROW EXECUTE FUNCTION private.retire_session_share_attachments();

CREATE TRIGGER on_workspace_session_share_attachments_deleted
  AFTER UPDATE OF deleted_at ON public.workspaces
  FOR EACH ROW EXECUTE FUNCTION private.retire_workspace_session_share_attachments();

CREATE OR REPLACE FUNCTION public.begin_account_deletion(
  p_owner_user_id uuid
)
RETURNS TABLE (
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_existing_horizon timestamptz;
  v_backup_horizon timestamptz;
  v_shared_horizon timestamptz;
  v_horizon timestamptz;
  v_was_created boolean;
BEGIN
  IF p_owner_user_id IS NULL THEN
    RAISE EXCEPTION 'invalid account deletion owner'
      USING ERRCODE = '22023';
  END IF;

  SELECT deletion.final_sweep_not_before
  INTO v_existing_horizon
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal'
  FOR UPDATE;

  UPDATE public.workspaces AS workspace
  SET
    deleted_at = COALESCE(workspace.deleted_at, v_now),
    updated_at = GREATEST(workspace.updated_at, v_now)
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal';

  UPDATE public.workspace_memberships AS membership
  SET
    deleted_at = COALESCE(membership.deleted_at, v_now),
    updated_at = GREATEST(membership.updated_at, v_now)
  WHERE membership.workspace_id = p_owner_user_id
    AND membership.user_id = p_owner_user_id;

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.owner_user_id = p_owner_user_id;

  DELETE FROM public.session_share_snapshot_attachments AS binding
  USING public.session_share_attachment_objects AS attachment
  WHERE attachment.owner_user_id = p_owner_user_id
    AND binding.share_id = attachment.share_id
    AND binding.attachment_id = attachment.id;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
    gc_lease_id = NULL,
    gc_lease_expires_at = NULL,
    updated_at = GREATEST(attachment.updated_at, v_now)
  WHERE attachment.owner_user_id = p_owner_user_id;

  SELECT max(backup.cleanup_not_before)
  INTO v_backup_horizon
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT max(attachment.cleanup_not_before)
  INTO v_shared_horizon
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.owner_user_id = p_owner_user_id;

  v_horizon := GREATEST(
    COALESCE(
      v_existing_horizon,
      v_now + interval '24 hours 5 minutes'
    ),
    COALESCE(v_backup_horizon, v_existing_horizon, v_now),
    COALESCE(v_shared_horizon, v_existing_horizon, v_now)
  );

  SELECT NOT EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
  ) INTO v_was_created;

  INSERT INTO private.account_deletion_jobs (
    owner_user_id,
    requested_at,
    final_sweep_not_before,
    updated_at
  ) VALUES (
    p_owner_user_id,
    LEAST(v_now, v_horizon),
    v_horizon,
    v_now
  )
  ON CONFLICT ON CONSTRAINT account_deletion_jobs_pkey DO UPDATE SET
    final_sweep_not_before = GREATEST(
      private.account_deletion_jobs.final_sweep_not_before,
      excluded.final_sweep_not_before
    ),
    prefix_swept_at = CASE
      WHEN excluded.final_sweep_not_before
        > private.account_deletion_jobs.final_sweep_not_before
        THEN NULL
      ELSE private.account_deletion_jobs.prefix_swept_at
    END,
    updated_at = GREATEST(
      private.account_deletion_jobs.updated_at,
      excluded.updated_at
    )
  RETURNING
    account_deletion_jobs.owner_user_id,
    account_deletion_jobs.final_sweep_not_before
  INTO owner_user_id, final_sweep_not_before;

  was_created := v_was_created;
  RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_account_deletion_leases(
  p_lease_id uuid,
  p_limit integer DEFAULT 10,
  p_lease_seconds integer DEFAULT 900
)
RETURNS TABLE (
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  prefix_swept boolean,
  lease_id uuid,
  lease_expires_at timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  IF p_lease_id IS NULL
    OR p_limit IS NULL
    OR p_limit NOT BETWEEN 1 AND 100
    OR p_lease_seconds IS NULL
    OR p_lease_seconds NOT BETWEEN 30 AND 3600
  THEN
    RAISE EXCEPTION 'invalid account deletion lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT deletion.owner_user_id
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.final_sweep_not_before <= v_now
      AND (
        deletion.lease_expires_at IS NULL
        OR deletion.lease_expires_at <= v_now
      )
      AND NOT EXISTS (
        SELECT 1
        FROM public.attachment_backup_objects AS backup
        WHERE backup.owner_user_id = deletion.owner_user_id
      )
      AND NOT EXISTS (
        SELECT 1
        FROM public.session_share_attachment_objects AS attachment
        WHERE attachment.owner_user_id = deletion.owner_user_id
      )
    ORDER BY deletion.final_sweep_not_before, deletion.requested_at, deletion.owner_user_id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE private.account_deletion_jobs AS deletion
    SET
      lease_id = p_lease_id,
      lease_expires_at = v_now + make_interval(secs => p_lease_seconds),
      updated_at = v_now
    FROM candidates
    WHERE deletion.owner_user_id = candidates.owner_user_id
    RETURNING deletion.*
  )
  SELECT
    leased.owner_user_id,
    leased.final_sweep_not_before,
    leased.prefix_swept_at IS NOT NULL,
    leased.lease_id,
    leased.lease_expires_at
  FROM leased
  ORDER BY leased.final_sweep_not_before, leased.requested_at, leased.owner_user_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_account_deletion_prefix_swept(
  p_owner_user_id uuid,
  p_lease_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_deletion private.account_deletion_jobs%ROWTYPE;
BEGIN
  SELECT deletion.*
  INTO v_deletion
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF p_lease_id IS NULL
    OR v_deletion.lease_id IS DISTINCT FROM p_lease_id
    OR v_deletion.lease_expires_at IS NULL
    OR v_deletion.lease_expires_at <= v_now
    OR v_deletion.final_sweep_not_before > v_now
    OR EXISTS (
      SELECT 1 FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR EXISTS (
      SELECT 1 FROM public.session_share_attachment_objects AS attachment
      WHERE attachment.owner_user_id = p_owner_user_id
    )
  THEN
    RAISE EXCEPTION 'account deletion sweep is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE private.account_deletion_jobs AS deletion
  SET
    prefix_swept_at = COALESCE(deletion.prefix_swept_at, v_now),
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id;
  RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION public.finish_account_deletion(
  p_owner_user_id uuid,
  p_lease_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_deletion private.account_deletion_jobs%ROWTYPE;
BEGIN
  SELECT deletion.*
  INTO v_deletion
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF p_lease_id IS NULL
    OR v_deletion.lease_id IS DISTINCT FROM p_lease_id
    OR v_deletion.lease_expires_at IS NULL
    OR v_deletion.lease_expires_at <= v_now
    OR v_deletion.prefix_swept_at IS NULL
    OR EXISTS (
      SELECT 1 FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR EXISTS (
      SELECT 1 FROM public.session_share_attachment_objects AS attachment
      WHERE attachment.owner_user_id = p_owner_user_id
    )
  THEN
    RAISE EXCEPTION 'account deletion completion is unavailable'
      USING ERRCODE = '55000';
  END IF;

  DELETE FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id;
  RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION public.begin_account_deletion(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.claim_account_deletion_leases(uuid, integer, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_account_deletion_prefix_swept(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finish_account_deletion(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.begin_account_deletion(uuid) TO service_role;
GRANT EXECUTE ON FUNCTION public.claim_account_deletion_leases(uuid, integer, integer)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_account_deletion_prefix_swept(uuid, uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.finish_account_deletion(uuid, uuid)
  TO service_role;
