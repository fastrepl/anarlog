BEGIN;

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

CREATE UNIQUE INDEX session_share_attachment_objects_active_version_key
  ON public.session_share_attachment_objects(share_id, version_ref)
  WHERE state <> 'deleting';

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
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );

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
    AND attachment.state <> 'deleting'
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

  IF EXISTS (
    SELECT 1
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.share_id = v_share.id
      AND attachment.version_ref = p_version_ref
      AND (
        attachment.owner_user_id IS DISTINCT FROM v_owner_user_id
        OR attachment.attachment_ref IS DISTINCT FROM p_attachment_ref
        OR attachment.filename IS DISTINCT FROM p_filename
        OR attachment.content_type IS DISTINCT FROM p_content_type
        OR attachment.size_bytes IS DISTINCT FROM p_size_bytes
      )
  ) THEN
    RAISE EXCEPTION 'shared attachment reservation conflicts'
      USING ERRCODE = '40001';
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

CREATE OR REPLACE FUNCTION private.read_my_session_share_snapshot_with_attachments(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    snapshot.share_id,
    snapshot.workspace_id,
    snapshot.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.capability,
    snapshot.manage_access,
    snapshot.access_version,
    snapshot.published_at
  FROM private.read_my_session_share_snapshot(p_share_id) AS snapshot;
$$;

CREATE OR REPLACE FUNCTION private.list_my_session_share_snapshots_with_attachments()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    snapshot.share_id,
    snapshot.workspace_id,
    snapshot.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.capability,
    snapshot.manage_access,
    snapshot.access_version,
    snapshot.published_at
  FROM private.list_my_session_share_snapshots() AS snapshot;
$$;

CREATE OR REPLACE FUNCTION public.read_my_session_share_snapshot_with_attachments(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.read_my_session_share_snapshot_with_attachments(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_snapshots_with_attachments()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT snapshot.*
  FROM private.list_my_session_share_snapshots_with_attachments() AS snapshot
  ORDER BY snapshot.share_id;
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_snapshot_page_with_attachments(
  p_after_share_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT snapshot.*
  FROM private.list_my_session_share_snapshots_with_attachments() AS snapshot
  WHERE p_after_share_id IS NULL OR snapshot.share_id > p_after_share_id
  ORDER BY snapshot.share_id
  LIMIT LEAST(GREATEST(COALESCE(p_limit, 100), 1), 100);
$$;

REVOKE ALL ON FUNCTION private.read_my_session_share_snapshot_with_attachments(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_my_session_share_snapshots_with_attachments()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_my_session_share_snapshot_with_attachments(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_session_share_snapshots_with_attachments()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_session_share_snapshot_page_with_attachments(uuid, integer)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.read_my_session_share_snapshot_with_attachments(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_session_share_snapshots_with_attachments()
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.read_my_session_share_snapshot_with_attachments(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_snapshots_with_attachments()
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_snapshot_page_with_attachments(uuid, integer)
  TO authenticated;

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
  v_replace_attachments boolean := p_attachment_ids IS NOT NULL;
  v_attachment_ids uuid[] := p_attachment_ids;
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

  IF NOT v_replace_attachments THEN
    SELECT COALESCE(
      array_agg(binding.attachment_id ORDER BY binding.position),
      ARRAY[]::uuid[]
    )
    INTO v_attachment_ids
    FROM public.session_share_snapshot_attachments AS binding
    WHERE binding.share_id = p_share_id;
  END IF;

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

  IF v_replace_attachments THEN
    SELECT array_agg(binding.attachment_id)
    INTO v_retired_ids
    FROM public.session_share_snapshot_attachments AS binding
    WHERE binding.share_id = p_share_id
      AND NOT (binding.attachment_id = ANY(v_attachment_ids));
  END IF;

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

  IF v_replace_attachments THEN
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
  END IF;

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
    NULL::uuid[]
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

DROP FUNCTION public.gateway_create_session_share_link_handoff(uuid, text);
DROP FUNCTION public.gateway_create_public_session_share_handoff(text);
DROP FUNCTION private.gateway_create_session_share_link_handoff(uuid, text);
DROP FUNCTION private.gateway_create_public_session_share_handoff(text);
DROP FUNCTION private.issue_session_share_handoff(uuid, text, uuid, bigint);

ALTER TABLE private.session_share_handoffs
  DROP CONSTRAINT session_share_handoffs_slot_check,
  ADD COLUMN lease_hash bytea,
  ADD COLUMN leased_at timestamptz,
  ADD COLUMN lease_expires_at timestamptz,
  ADD COLUMN source_hash bytea NOT NULL DEFAULT decode(repeat('0', 64), 'hex'),
  ADD CONSTRAINT session_share_handoffs_slot_check CHECK (
    slot BETWEEN 0 AND 31
  ),
  ADD CONSTRAINT session_share_handoffs_source_hash_check CHECK (
    octet_length(source_hash) = 32
  ),
  ADD CONSTRAINT session_share_handoffs_lease_check CHECK (
    (lease_hash IS NULL AND leased_at IS NULL AND lease_expires_at IS NULL)
    OR (
      octet_length(lease_hash) = 32
      AND leased_at IS NOT NULL
      AND leased_at >= created_at
      AND leased_at < expires_at
      AND lease_expires_at = leased_at + interval '20 minutes'
    )
  );

ALTER TABLE private.session_share_handoffs
  ALTER COLUMN source_hash DROP DEFAULT;

CREATE UNIQUE INDEX session_share_handoffs_lease_hash_key
  ON private.session_share_handoffs (lease_hash)
  WHERE lease_hash IS NOT NULL;

CREATE OR REPLACE FUNCTION private.issue_session_share_handoff(
  p_share_id uuid,
  p_access_kind text,
  p_link_id uuid,
  p_access_version bigint,
  p_source_hash bytea
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_created_at timestamptz := clock_timestamp();
  v_request_id text;
  v_slot smallint;
  v_rows bigint;
  v_source_active_count integer;
BEGIN
  IF p_source_hash IS NULL OR octet_length(p_source_hash) <> 32 THEN
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_share_id::text, 0)
  );
  v_created_at := clock_timestamp();

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND share.access_version = p_access_version
    AND (
      (
        p_access_kind = 'public'
        AND p_link_id IS NULL
        AND share.general_scope = 'public'
      )
      OR (
        p_access_kind = 'link'
        AND share.general_scope = 'link'
        AND EXISTS (
          SELECT 1
          FROM public.session_share_links AS link
          WHERE link.id = p_link_id
            AND link.share_id = share.id
            AND link.revoked_at IS NULL
        )
      )
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT count(*)
  INTO v_source_active_count
  FROM private.session_share_handoffs AS handoff
  WHERE handoff.share_id = p_share_id
    AND handoff.source_hash = p_source_hash
    AND COALESCE(handoff.lease_expires_at, handoff.expires_at) > v_created_at;

  IF v_source_active_count >= 4 THEN
    RETURN;
  END IF;

  SELECT candidate.slot::smallint
  INTO v_slot
  FROM pg_catalog.generate_series(0, 31) AS candidate(slot)
  LEFT JOIN private.session_share_handoffs AS handoff
    ON handoff.share_id = p_share_id
    AND handoff.slot = candidate.slot
  WHERE handoff.slot IS NULL
    OR COALESCE(handoff.lease_expires_at, handoff.expires_at) <= v_created_at
  ORDER BY
    (handoff.slot IS NULL) DESC,
    handoff.created_at ASC NULLS FIRST,
    candidate.slot
  LIMIT 1;

  IF v_slot IS NULL THEN
    RETURN;
  END IF;

  v_request_id := gen_random_uuid()::text;

  INSERT INTO private.session_share_handoffs AS current_handoff (
    request_hash,
    share_id,
    slot,
    access_kind,
    link_id,
    access_version,
    source_hash,
    created_at,
    lease_hash,
    leased_at,
    lease_expires_at,
    expires_at
  ) VALUES (
    extensions.digest(v_request_id, 'sha256'),
    p_share_id,
    v_slot,
    p_access_kind,
    p_link_id,
    p_access_version,
    p_source_hash,
    v_created_at,
    NULL,
    NULL,
    NULL,
    v_created_at + interval '60 seconds'
  )
  ON CONFLICT (share_id, slot) DO UPDATE SET
    request_hash = excluded.request_hash,
    access_kind = excluded.access_kind,
    link_id = excluded.link_id,
    access_version = excluded.access_version,
    source_hash = excluded.source_hash,
    created_at = excluded.created_at,
    lease_hash = NULL,
    leased_at = NULL,
    lease_expires_at = NULL,
    expires_at = excluded.expires_at
  WHERE COALESCE(
    current_handoff.lease_expires_at,
    current_handoff.expires_at
  ) <= v_created_at;

  GET DIAGNOSTICS v_rows = ROW_COUNT;
  IF v_rows <> 1 THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT v_request_id, v_created_at + interval '60 seconds';
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_create_session_share_link_handoff(
  p_share_id uuid,
  p_link_token text,
  p_source_hash text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_link public.session_share_links%ROWTYPE;
BEGIN
  IF p_link_token IS NULL OR octet_length(p_link_token) <> 43 THEN
    RETURN;
  END IF;
  IF p_link_token !~ '^[A-Za-z0-9_-]{43}$' THEN
    RETURN;
  END IF;
  IF p_source_hash IS NULL OR octet_length(p_source_hash) <> 64 THEN
    RETURN;
  END IF;
  IF p_source_hash !~ '^[0-9a-f]{64}$' THEN
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.id = p_share_id
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT link.*
  INTO v_link
  FROM public.session_share_links AS link
  WHERE link.share_id = v_share.id
    AND link.revoked_at IS NULL
    AND link.token_hash = extensions.digest(p_link_token, 'sha256');

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT handoff.request_id, handoff.expires_at
  FROM private.issue_session_share_handoff(
    v_share.id,
    'link',
    v_link.id,
    v_share.access_version,
    decode(p_source_hash, 'hex')
  ) AS handoff;
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_create_public_session_share_handoff(
  p_public_slug text,
  p_source_hash text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF p_public_slug IS NULL OR octet_length(p_public_slug) <> 34 THEN
    RETURN;
  END IF;
  IF p_public_slug !~ '^s_[0-9a-f]{32}$' THEN
    RETURN;
  END IF;
  IF p_source_hash IS NULL OR octet_length(p_source_hash) <> 64 THEN
    RETURN;
  END IF;
  IF p_source_hash !~ '^[0-9a-f]{64}$' THEN
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.public_slug = p_public_slug
    AND share.general_scope = 'public'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT handoff.request_id, handoff.expires_at
  FROM private.issue_session_share_handoff(
    v_share.id,
    'public',
    NULL,
    v_share.access_version,
    decode(p_source_hash, 'hex')
  ) AS handoff;
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_create_session_share_link_handoff(
  p_share_id uuid,
  p_link_token text,
  p_source_hash text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_create_session_share_link_handoff(
    p_share_id,
    p_link_token,
    p_source_hash
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_create_public_session_share_handoff(
  p_public_slug text,
  p_source_hash text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_create_public_session_share_handoff(
    p_public_slug,
    p_source_hash
  );
$$;

CREATE OR REPLACE FUNCTION private.gateway_lease_session_share_handoff(
  p_request_id text,
  p_lease_id text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  lease_expires_at timestamptz,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_handoff private.session_share_handoffs%ROWTYPE;
  v_snapshot record;
  v_lease_hash bytea;
  v_share_id uuid;
BEGIN
  IF p_request_id IS NULL OR octet_length(p_request_id) <> 36 THEN
    RETURN;
  END IF;
  IF p_request_id !~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN
    RETURN;
  END IF;
  IF p_lease_id IS NULL OR octet_length(p_lease_id) <> 36 THEN
    RETURN;
  END IF;
  IF p_lease_id !~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN
    RETURN;
  END IF;

  v_lease_hash := extensions.digest(p_lease_id, 'sha256');

  SELECT handoff.share_id
  INTO v_share_id
  FROM private.session_share_handoffs AS handoff
  WHERE handoff.request_hash = extensions.digest(p_request_id, 'sha256');

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_share_id::text, 0)
  );
  v_now := clock_timestamp();

  SELECT handoff.*
  INTO v_handoff
  FROM private.session_share_handoffs AS handoff
  WHERE handoff.request_hash = extensions.digest(p_request_id, 'sha256')
    AND handoff.share_id = v_share_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF v_handoff.lease_hash IS NULL AND v_handoff.expires_at <= v_now THEN
    RETURN;
  END IF;
  IF v_handoff.lease_hash IS NOT NULL AND (
    v_handoff.lease_hash <> v_lease_hash
    OR v_handoff.lease_expires_at <= v_now
  ) THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = v_handoff.share_id
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF v_handoff.access_kind = 'public' THEN
    SELECT
      share.id AS share_id,
      snapshot.schema_version,
      snapshot.content_revision,
      snapshot.title,
      snapshot.body_json,
      private.session_share_attachment_manifest(share.id) AS attachments_json,
      snapshot.published_at
    INTO v_snapshot
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    JOIN public.session_share_snapshots AS snapshot
      ON snapshot.share_id = share.id
    WHERE share.id = v_handoff.share_id
      AND share.general_scope = 'public'
      AND share.access_version = v_handoff.access_version
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL;
  ELSE
    SELECT
      share.id AS share_id,
      snapshot.schema_version,
      snapshot.content_revision,
      snapshot.title,
      snapshot.body_json,
      private.session_share_attachment_manifest(share.id) AS attachments_json,
      snapshot.published_at
    INTO v_snapshot
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    JOIN public.session_share_links AS link
      ON link.id = v_handoff.link_id
      AND link.share_id = share.id
    JOIN public.session_share_snapshots AS snapshot
      ON snapshot.share_id = share.id
    WHERE share.id = v_handoff.share_id
      AND share.general_scope = 'link'
      AND share.access_version = v_handoff.access_version
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
      AND link.revoked_at IS NULL;
  END IF;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF v_handoff.lease_hash IS NULL THEN
    UPDATE private.session_share_handoffs AS handoff
    SET
      lease_hash = v_lease_hash,
      leased_at = v_now,
      lease_expires_at = v_now + interval '20 minutes'
    WHERE handoff.request_hash = v_handoff.request_hash
    RETURNING * INTO v_handoff;
  END IF;

  RETURN QUERY SELECT
    v_snapshot.share_id::uuid,
    v_snapshot.schema_version::smallint,
    v_snapshot.content_revision::bigint,
    v_snapshot.title::text,
    v_snapshot.body_json::jsonb,
    v_snapshot.attachments_json::jsonb,
    v_handoff.lease_expires_at,
    v_snapshot.published_at::timestamptz;
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_lease_session_share_handoff(
  p_request_id text,
  p_lease_id text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  lease_expires_at timestamptz,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_lease_session_share_handoff(
    p_request_id,
    p_lease_id
  );
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

DROP FUNCTION IF EXISTS public.gateway_claim_session_share_handoff_v2(text);
DROP FUNCTION IF EXISTS private.gateway_claim_session_share_handoff_v2(text);
DROP FUNCTION IF EXISTS public.gateway_claim_session_share_handoff(text);
DROP FUNCTION IF EXISTS private.gateway_claim_session_share_handoff(text);

REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_snapshot_v2(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.gateway_read_public_session_share_snapshot_v2(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.issue_session_share_handoff(uuid, text, uuid, bigint, bytea)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_create_session_share_link_handoff(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.gateway_create_public_session_share_handoff(text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.gateway_lease_session_share_handoff(text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_snapshot_v2(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_read_public_session_share_snapshot_v2(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_create_session_share_link_handoff(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_create_public_session_share_handoff(text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_lease_session_share_handoff(text, text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_snapshot_v2(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_public_session_share_snapshot_v2(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_create_session_share_link_handoff(uuid, text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_create_public_session_share_handoff(text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_lease_session_share_handoff(text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_snapshot_v2(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_public_session_share_snapshot_v2(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_create_session_share_link_handoff(uuid, text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_create_public_session_share_handoff(text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_lease_session_share_handoff(text, text)
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

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
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

CREATE OR REPLACE FUNCTION private.prepare_session_share_handoff_attachment_download(
  p_lease_id text,
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
  v_handoff private.session_share_handoffs%ROWTYPE;
  v_share_id uuid;
BEGIN
  IF p_lease_id IS NULL OR octet_length(p_lease_id) <> 36 THEN
    RETURN;
  END IF;
  IF p_lease_id !~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN
    RETURN;
  END IF;

  SELECT handoff.share_id
  INTO v_share_id
  FROM private.session_share_handoffs AS handoff
  WHERE handoff.lease_hash = extensions.digest(p_lease_id, 'sha256');

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_share_id::text, 0)
  );
  v_now := clock_timestamp();

  SELECT handoff.*
  INTO v_handoff
  FROM private.session_share_handoffs AS handoff
  WHERE handoff.lease_hash = extensions.digest(p_lease_id, 'sha256')
    AND handoff.share_id = v_share_id
    AND handoff.lease_expires_at > v_now
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = v_handoff.share_id
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.id = v_handoff.share_id
    AND share.access_version = v_handoff.access_version
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND (
      (
        v_handoff.access_kind = 'public'
        AND share.general_scope = 'public'
      )
      OR (
        v_handoff.access_kind = 'link'
        AND share.general_scope = 'link'
        AND EXISTS (
          SELECT 1
          FROM public.session_share_links AS link
          WHERE link.id = v_handoff.link_id
            AND link.share_id = share.id
            AND link.revoked_at IS NULL
        )
      )
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT *
  FROM private.prepare_session_share_attachment_download(
    v_handoff.share_id,
    p_attachment_id,
    p_download_expires_at
  );
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_prepare_session_share_handoff_attachment_download(
  p_lease_id text,
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.prepare_session_share_handoff_attachment_download(
    p_lease_id,
    p_attachment_id,
    p_download_expires_at
  );
$$;

REVOKE ALL ON FUNCTION private.prepare_session_share_handoff_attachment_download(
  text, uuid, timestamptz
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.gateway_prepare_session_share_handoff_attachment_download(
  text, uuid, timestamptz
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.prepare_session_share_handoff_attachment_download(
  text, uuid, timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_prepare_session_share_handoff_attachment_download(
  text, uuid, timestamptz
) TO service_role;

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
  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
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
    );

  IF NOT FOUND THEN
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
  IF p_link_token IS NULL OR p_link_token !~ '^[A-Za-z0-9_-]{43}$' THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.session_share_links AS link
    ON link.share_id = share.id
  WHERE share.id = p_share_id
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND link.revoked_at IS NULL
    AND link.token_hash = extensions.digest(p_link_token, 'sha256');

  IF NOT FOUND THEN
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
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = v_share_id
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

DO $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  LOCK TABLE
    private.account_deletion_jobs,
    public.session_share_snapshot_attachments,
    public.session_share_attachment_objects
    IN SHARE ROW EXCLUSIVE MODE;

  DELETE FROM public.session_share_snapshot_attachments AS binding
  USING
    public.session_share_attachment_objects AS attachment,
    private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = attachment.owner_user_id
    AND binding.share_id = attachment.share_id
    AND binding.attachment_id = attachment.id;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
    gc_lease_id = NULL,
    gc_lease_expires_at = NULL,
    updated_at = GREATEST(attachment.updated_at, v_now)
  WHERE EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = attachment.owner_user_id
  );

  UPDATE private.account_deletion_jobs AS deletion
  SET
    final_sweep_not_before = GREATEST(
      deletion.final_sweep_not_before,
      COALESCE(
        (
          SELECT max(attachment.cleanup_not_before)
          FROM public.session_share_attachment_objects AS attachment
          WHERE attachment.owner_user_id = deletion.owner_user_id
        ),
        deletion.final_sweep_not_before
      )
    ),
    prefix_swept_at = NULL,
    lease_id = NULL,
    lease_expires_at = NULL,
    updated_at = GREATEST(deletion.updated_at, v_now);
END;
$$;

CREATE OR REPLACE FUNCTION private.prepare_account_deletion_extension(
  p_owner_user_id uuid,
  p_now timestamptz
)
RETURNS timestamptz
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_cleanup_not_before timestamptz;
BEGIN
  DELETE FROM public.session_share_snapshot_attachments AS binding
  USING public.session_share_attachment_objects AS attachment
  WHERE attachment.owner_user_id = p_owner_user_id
    AND binding.share_id = attachment.share_id
    AND binding.attachment_id = attachment.id;

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(attachment.deletion_requested_at, p_now),
    gc_lease_id = NULL,
    gc_lease_expires_at = NULL,
    updated_at = GREATEST(attachment.updated_at, p_now)
  WHERE attachment.owner_user_id = p_owner_user_id;

  SELECT max(attachment.cleanup_not_before)
  INTO v_cleanup_not_before
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.owner_user_id = p_owner_user_id;

  RETURN v_cleanup_not_before;
END;
$$;

CREATE OR REPLACE FUNCTION private.account_deletion_extension_ledgers_empty(
  p_owner_user_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
BEGIN
  RETURN NOT EXISTS (
    SELECT 1
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.owner_user_id = p_owner_user_id
  );
END;
$$;

CREATE OR REPLACE FUNCTION private.account_deletion_extension_prefix_empty(
  p_owner_user_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
BEGIN
  RETURN NOT EXISTS (
    SELECT 1
    FROM storage.objects AS object
    WHERE object.bucket_id = 'shared-note-attachments'
      AND left(object.name, length(p_owner_user_id::text) + 1)
        = p_owner_user_id::text || '/'
  );
END;
$$;

REVOKE ALL ON FUNCTION private.prepare_account_deletion_extension(uuid, timestamptz)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.account_deletion_extension_ledgers_empty(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.account_deletion_extension_prefix_empty(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.prepare_account_deletion_extension(uuid, timestamptz)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.account_deletion_extension_ledgers_empty(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.account_deletion_extension_prefix_empty(uuid)
  TO service_role;

COMMIT;
