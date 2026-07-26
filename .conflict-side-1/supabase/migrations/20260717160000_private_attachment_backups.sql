INSERT INTO storage.buckets (
  id,
  name,
  public,
  file_size_limit,
  allowed_mime_types
)
VALUES (
  'attachment-backups',
  'attachment-backups',
  false,
  545259520,
  ARRAY['application/octet-stream']::text[]
)
ON CONFLICT (id) DO UPDATE SET
  name = EXCLUDED.name,
  public = EXCLUDED.public,
  file_size_limit = EXCLUDED.file_size_limit,
  allowed_mime_types = EXCLUDED.allowed_mime_types;

DROP POLICY IF EXISTS attachment_backups_deny_client_select
  ON storage.objects;
CREATE POLICY attachment_backups_deny_client_select
  ON storage.objects
  AS RESTRICTIVE
  FOR SELECT
  TO anon, authenticated
  USING (bucket_id <> 'attachment-backups');

DROP POLICY IF EXISTS attachment_backups_deny_client_insert
  ON storage.objects;
CREATE POLICY attachment_backups_deny_client_insert
  ON storage.objects
  AS RESTRICTIVE
  FOR INSERT
  TO anon, authenticated
  WITH CHECK (bucket_id <> 'attachment-backups');

DROP POLICY IF EXISTS attachment_backups_deny_client_update
  ON storage.objects;
CREATE POLICY attachment_backups_deny_client_update
  ON storage.objects
  AS RESTRICTIVE
  FOR UPDATE
  TO anon, authenticated
  USING (bucket_id <> 'attachment-backups')
  WITH CHECK (bucket_id <> 'attachment-backups');

DROP POLICY IF EXISTS attachment_backups_deny_client_delete
  ON storage.objects;
CREATE POLICY attachment_backups_deny_client_delete
  ON storage.objects
  AS RESTRICTIVE
  FOR DELETE
  TO anon, authenticated
  USING (bucket_id <> 'attachment-backups');

CREATE TABLE public.attachment_backup_objects (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE RESTRICT,
  attachment_ref text NOT NULL,
  version_ref text NOT NULL,
  object_key text NOT NULL,
  ciphertext_size_bytes bigint NOT NULL,
  ciphertext_sha256 text,
  format_version smallint NOT NULL DEFAULT 1,
  state text NOT NULL DEFAULT 'reserved',
  reservation_expires_at timestamptz NOT NULL DEFAULT (now() + interval '15 minutes'),
  last_signed_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz NOT NULL DEFAULT (now() + interval '15 minutes'),
  finalized_at timestamptz,
  deletion_requested_at timestamptz,
  gc_lease_id uuid,
  gc_lease_expires_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT attachment_backup_objects_attachment_ref_check CHECK (
    attachment_ref ~ '^[A-Za-z0-9_-]{43}$'
  ),
  CONSTRAINT attachment_backup_objects_version_ref_check CHECK (
    version_ref ~ '^[A-Za-z0-9_-]{43}$'
    AND version_ref <> attachment_ref
  ),
  CONSTRAINT attachment_backup_objects_object_key_check CHECK (
    object_key ~ (
      '^'
      || owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  ),
  CONSTRAINT attachment_backup_objects_size_check CHECK (
    ciphertext_size_bytes BETWEEN 1 AND 545259520
  ),
  CONSTRAINT attachment_backup_objects_sha256_check CHECK (
    ciphertext_sha256 IS NULL
    OR ciphertext_sha256 ~ '^[0-9a-f]{64}$'
  ),
  CONSTRAINT attachment_backup_objects_format_check CHECK (format_version = 1),
  CONSTRAINT attachment_backup_objects_state_check CHECK (
    state IN ('reserved', 'ready', 'current', 'deleting')
  ),
  CONSTRAINT attachment_backup_objects_signing_check CHECK (
    (last_signed_at IS NULL AND upload_expires_at IS NULL)
    OR (
      last_signed_at IS NOT NULL
      AND upload_expires_at IS NOT NULL
      AND upload_expires_at > last_signed_at
    )
  ),
  CONSTRAINT attachment_backup_objects_lifecycle_check CHECK (
    (
      state = 'reserved'
      AND finalized_at IS NULL
      AND deletion_requested_at IS NULL
    )
    OR (
      state IN ('ready', 'current')
      AND finalized_at IS NOT NULL
      AND deletion_requested_at IS NULL
      AND ciphertext_sha256 IS NOT NULL
    )
    OR (
      state = 'deleting'
      AND deletion_requested_at IS NOT NULL
    )
  ),
  CONSTRAINT attachment_backup_objects_lease_check CHECK (
    (gc_lease_id IS NULL AND gc_lease_expires_at IS NULL)
    OR (
      gc_lease_id IS NOT NULL
      AND gc_lease_expires_at IS NOT NULL
    )
  ),
  CONSTRAINT attachment_backup_objects_time_check CHECK (
    reservation_expires_at > created_at
    AND cleanup_not_before >= reservation_expires_at
    AND (
      upload_expires_at IS NULL
      OR cleanup_not_before >= upload_expires_at + interval '24 hours 5 minutes'
    )
    AND updated_at >= created_at
  ),
  CONSTRAINT attachment_backup_objects_owner_version_key UNIQUE (
    owner_user_id,
    version_ref
  ),
  CONSTRAINT attachment_backup_objects_object_key_key UNIQUE (object_key)
);

CREATE INDEX attachment_backup_objects_owner_ready_idx
  ON public.attachment_backup_objects(
    owner_user_id,
    attachment_ref,
    finalized_at DESC
  )
  WHERE state = 'ready';

CREATE UNIQUE INDEX attachment_backup_objects_owner_current_idx
  ON public.attachment_backup_objects(owner_user_id, attachment_ref)
  WHERE state = 'current';

CREATE INDEX attachment_backup_objects_owner_state_idx
  ON public.attachment_backup_objects(owner_user_id, state, created_at);

CREATE INDEX attachment_backup_objects_gc_idx
  ON public.attachment_backup_objects(
    cleanup_not_before,
    gc_lease_expires_at,
    created_at
  )
  WHERE state IN ('reserved', 'ready', 'deleting');

ALTER TABLE public.attachment_backup_objects ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.attachment_backup_objects
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.attachment_backup_objects TO service_role;

CREATE POLICY attachment_backup_objects_service_all
  ON public.attachment_backup_objects
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

GRANT USAGE ON SCHEMA private TO service_role;

CREATE OR REPLACE FUNCTION private.require_attachment_backup_owner(
  p_owner_user_id uuid
)
RETURNS void
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal'
    AND workspace.e2ee_key_id IS NOT NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'active personal E2EE workspace required'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.require_attachment_backup_owner(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.require_attachment_backup_owner(uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION private.enforce_attachment_backup_identity()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  IF NEW.id IS DISTINCT FROM OLD.id
    OR NEW.owner_user_id IS DISTINCT FROM OLD.owner_user_id
    OR NEW.attachment_ref IS DISTINCT FROM OLD.attachment_ref
    OR NEW.version_ref IS DISTINCT FROM OLD.version_ref
    OR NEW.object_key IS DISTINCT FROM OLD.object_key
    OR NEW.ciphertext_size_bytes IS DISTINCT FROM OLD.ciphertext_size_bytes
    OR NEW.format_version IS DISTINCT FROM OLD.format_version
    OR (
      OLD.ciphertext_sha256 IS NOT NULL
      AND NEW.ciphertext_sha256 IS DISTINCT FROM OLD.ciphertext_sha256
    )
  THEN
    RAISE EXCEPTION 'attachment backup identity is immutable'
      USING ERRCODE = '22023';
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.enforce_attachment_backup_identity()
  FROM PUBLIC, anon, authenticated;

CREATE TRIGGER attachment_backup_objects_immutable_identity
  BEFORE UPDATE ON public.attachment_backup_objects
  FOR EACH ROW
  EXECUTE FUNCTION private.enforce_attachment_backup_identity();

CREATE OR REPLACE FUNCTION public.read_attachment_backup_by_key(
  p_owner_user_id uuid,
  p_object_key text
)
RETURNS TABLE (
  object_id uuid,
  attachment_ref text,
  version_ref text,
  object_key text,
  object_state text,
  ciphertext_sha256 text,
  ciphertext_size_bytes bigint,
  format_version smallint,
  reservation_expires_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  IF p_object_key IS NULL
    OR p_object_key !~ (
      '^'
      || p_owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  THEN
    RAISE EXCEPTION 'invalid attachment backup object key'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  SELECT
    backup.id,
    backup.attachment_ref,
    backup.version_ref,
    backup.object_key,
    backup.state,
    backup.ciphertext_sha256,
    backup.ciphertext_size_bytes,
    backup.format_version,
    backup.reservation_expires_at,
    backup.upload_expires_at,
    backup.cleanup_not_before
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.object_key = p_object_key;
END;
$$;

CREATE OR REPLACE FUNCTION public.read_current_attachment_backup(
  p_owner_user_id uuid,
  p_attachment_ref text
)
RETURNS TABLE (
  object_id uuid,
  version_ref text,
  object_key text,
  ciphertext_sha256 text,
  ciphertext_size_bytes bigint,
  format_version smallint
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  IF p_attachment_ref IS NULL
    OR p_attachment_ref !~ '^[A-Za-z0-9_-]{43}$'
  THEN
    RAISE EXCEPTION 'invalid attachment backup reference'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  SELECT
    backup.id,
    backup.version_ref,
    backup.object_key,
    backup.ciphertext_sha256,
    backup.ciphertext_size_bytes,
    backup.format_version
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.attachment_ref = p_attachment_ref
    AND backup.state = 'current';
END;
$$;

CREATE OR REPLACE FUNCTION public.reserve_attachment_backup(
  p_owner_user_id uuid,
  p_attachment_ref text,
  p_version_ref text,
  p_ciphertext_size_bytes bigint,
  p_format_version smallint DEFAULT 1
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  object_state text,
  ciphertext_sha256 text,
  ciphertext_size_bytes bigint,
  format_version smallint,
  reservation_expires_at timestamptz,
  cleanup_not_before timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_existing public.attachment_backup_objects%ROWTYPE;
  v_created public.attachment_backup_objects%ROWTYPE;
  v_account_bytes bigint;
  v_object_count bigint;
  v_active_reservations bigint;
BEGIN
  IF p_attachment_ref IS NULL
    OR p_attachment_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref IS NULL
    OR p_version_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_attachment_ref = p_version_ref
    OR p_ciphertext_size_bytes IS NULL
    OR p_ciphertext_size_bytes < 1
    OR p_ciphertext_size_bytes > 545259520
    OR p_format_version <> 1
  THEN
    RAISE EXCEPTION 'invalid attachment backup reservation'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT backup.*
  INTO v_existing
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.version_ref = p_version_ref
  FOR UPDATE;

  IF FOUND THEN
    IF v_existing.attachment_ref <> p_attachment_ref
      OR v_existing.ciphertext_size_bytes <> p_ciphertext_size_bytes
      OR v_existing.format_version <> p_format_version
    THEN
      RAISE EXCEPTION 'attachment backup version conflicts with existing reservation'
        USING ERRCODE = '40001';
    END IF;

    IF v_existing.state = 'deleting'
      OR (
        v_existing.state = 'reserved'
        AND v_existing.cleanup_not_before <= v_now
      )
    THEN
      RAISE EXCEPTION 'attachment backup reservation is no longer reusable'
        USING ERRCODE = '55000';
    END IF;

    RETURN QUERY
    SELECT
      v_existing.id,
      v_existing.object_key,
      v_existing.state,
      v_existing.ciphertext_sha256,
      v_existing.ciphertext_size_bytes,
      v_existing.format_version,
      v_existing.reservation_expires_at,
      v_existing.cleanup_not_before,
      false;
    RETURN;
  END IF;

  SELECT
    COALESCE(sum(backup.ciphertext_size_bytes), 0),
    count(*)
  INTO v_account_bytes, v_object_count
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id;

  IF v_object_count >= 10000 THEN
    RAISE EXCEPTION 'attachment backup object limit exceeded'
      USING ERRCODE = '54000';
  END IF;

  IF p_ciphertext_size_bytes > 5368709120 - v_account_bytes THEN
    RAISE EXCEPTION 'attachment backup storage quota exceeded'
      USING ERRCODE = '54000';
  END IF;

  SELECT count(*)
  INTO v_active_reservations
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.state = 'reserved'
    AND backup.cleanup_not_before > v_now;

  IF v_active_reservations >= 5 THEN
    RAISE EXCEPTION 'attachment backup reservation limit exceeded'
      USING ERRCODE = '55000';
  END IF;

  INSERT INTO public.attachment_backup_objects (
    owner_user_id,
    attachment_ref,
    version_ref,
    object_key,
    ciphertext_size_bytes,
    format_version,
    reservation_expires_at,
    cleanup_not_before,
    created_at,
    updated_at
  ) VALUES (
    p_owner_user_id,
    p_attachment_ref,
    p_version_ref,
    p_owner_user_id::text || '/' || extensions.gen_random_uuid()::text || '.anb1',
    p_ciphertext_size_bytes,
    p_format_version,
    v_now + interval '15 minutes',
    v_now + interval '15 minutes',
    v_now,
    v_now
  )
  RETURNING * INTO v_created;

  RETURN QUERY
  SELECT
    v_created.id,
    v_created.object_key,
    v_created.state,
    v_created.ciphertext_sha256,
    v_created.ciphertext_size_bytes,
    v_created.format_version,
    v_created.reservation_expires_at,
    v_created.cleanup_not_before,
    true;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_attachment_backup_signed(
  p_owner_user_id uuid,
  p_object_id uuid,
  p_ciphertext_sha256 text,
  p_upload_expires_at timestamptz
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  ciphertext_sha256 text,
  last_signed_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup public.attachment_backup_objects%ROWTYPE;
BEGIN
  IF p_ciphertext_sha256 IS NULL
    OR p_ciphertext_sha256 !~ '^[0-9a-f]{64}$'
  THEN
    RAISE EXCEPTION 'invalid attachment backup ciphertext hash'
      USING ERRCODE = '22023';
  END IF;

  IF p_object_id IS NULL
    OR p_upload_expires_at IS NULL
    OR p_upload_expires_at <= v_now
    OR p_upload_expires_at > v_now + interval '2 hours 5 minutes'
  THEN
    RAISE EXCEPTION 'invalid attachment backup upload expiry'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = p_object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND
    OR v_backup.state <> 'reserved'
    OR v_backup.cleanup_not_before <= v_now
  THEN
    RAISE EXCEPTION 'attachment backup reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  IF v_backup.ciphertext_sha256 IS NOT NULL
    AND v_backup.ciphertext_sha256 <> p_ciphertext_sha256
  THEN
    RAISE EXCEPTION 'attachment backup ciphertext hash conflicts with reservation'
      USING ERRCODE = '40001';
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    ciphertext_sha256 = COALESCE(
      backup.ciphertext_sha256,
      p_ciphertext_sha256
    ),
    last_signed_at = v_now,
    upload_expires_at = GREATEST(
      COALESCE(backup.upload_expires_at, p_upload_expires_at),
      p_upload_expires_at
    ),
    cleanup_not_before = GREATEST(
      backup.cleanup_not_before,
      p_upload_expires_at + interval '24 hours 5 minutes'
    ),
    updated_at = v_now
  WHERE backup.id = v_backup.id
  RETURNING backup.* INTO v_backup;

  RETURN QUERY
  SELECT
    v_backup.id,
    v_backup.object_key,
    v_backup.ciphertext_sha256,
    v_backup.last_signed_at,
    v_backup.upload_expires_at,
    v_backup.cleanup_not_before;
END;
$$;

CREATE OR REPLACE FUNCTION public.finalize_attachment_backup(
  p_owner_user_id uuid,
  p_object_id uuid,
  p_object_key text,
  p_observed_ciphertext_size_bytes bigint
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  object_state text,
  was_finalized boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup public.attachment_backup_objects%ROWTYPE;
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = p_object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  IF p_object_key IS NULL
    OR p_observed_ciphertext_size_bytes IS NULL
  THEN
    RAISE EXCEPTION 'invalid attachment backup finalization'
      USING ERRCODE = '22023';
  END IF;

  IF p_object_key <> v_backup.object_key
    OR p_observed_ciphertext_size_bytes <> v_backup.ciphertext_size_bytes
  THEN
    RAISE EXCEPTION 'attachment backup object does not match reservation'
      USING ERRCODE = '40001';
  END IF;

  IF v_backup.ciphertext_sha256 IS NULL THEN
    RAISE EXCEPTION 'attachment backup ciphertext hash is unavailable'
      USING ERRCODE = '55000';
  END IF;

  IF v_backup.state IN ('ready', 'current') THEN
    RETURN QUERY
    SELECT v_backup.id, v_backup.object_key, v_backup.state, false;
    RETURN;
  END IF;

  IF v_backup.state <> 'reserved'
    OR v_backup.last_signed_at IS NULL
    OR v_backup.cleanup_not_before <= v_now
  THEN
    RAISE EXCEPTION 'attachment backup reservation is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'ready',
    cleanup_not_before = GREATEST(
      backup.cleanup_not_before,
      v_now + interval '24 hours'
    ),
    finalized_at = v_now,
    updated_at = v_now
  WHERE backup.id = v_backup.id
  RETURNING backup.* INTO v_backup;

  RETURN QUERY
  SELECT
    v_backup.id,
    v_backup.object_key,
    v_backup.state,
    true;
END;
$$;

CREATE OR REPLACE FUNCTION public.promote_attachment_backup(
  p_owner_user_id uuid,
  p_candidate_object_id uuid,
  p_candidate_object_key text,
  p_expected_current_object_key text DEFAULT NULL
)
RETURNS TABLE (
  current_object_id uuid,
  current_object_key text,
  current_version_ref text,
  current_ciphertext_sha256 text,
  displaced_object_id uuid,
  displaced_object_key text,
  was_promoted boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_candidate public.attachment_backup_objects%ROWTYPE;
  v_current public.attachment_backup_objects%ROWTYPE;
  v_displaced public.attachment_backup_objects%ROWTYPE;
  v_has_current boolean := false;
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT backup.*
  INTO v_candidate
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = p_candidate_object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup candidate is unavailable'
      USING ERRCODE = '55000';
  END IF;

  IF p_candidate_object_key IS NULL
  THEN
    RAISE EXCEPTION 'invalid attachment backup candidate key'
      USING ERRCODE = '22023';
  END IF;

  IF p_candidate_object_key <> v_candidate.object_key
  THEN
    RAISE EXCEPTION 'attachment backup candidate key does not match'
      USING ERRCODE = '40001';
  END IF;

  IF v_candidate.state = 'current' THEN
    RETURN QUERY
    SELECT
      v_candidate.id,
      v_candidate.object_key,
      v_candidate.version_ref,
      v_candidate.ciphertext_sha256,
      NULL::uuid,
      NULL::text,
      false;
    RETURN;
  END IF;

  IF v_candidate.state <> 'ready'
    OR v_candidate.cleanup_not_before <= v_now
  THEN
    RAISE EXCEPTION 'attachment backup candidate is unavailable'
      USING ERRCODE = '55000';
  END IF;

  SELECT backup.*
  INTO v_current
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.attachment_ref = v_candidate.attachment_ref
    AND backup.state = 'current'
  FOR UPDATE;

  v_has_current := FOUND;

  IF (p_expected_current_object_key IS NULL AND v_has_current)
    OR (
      p_expected_current_object_key IS NOT NULL
      AND (
        NOT v_has_current
        OR v_current.object_key <> p_expected_current_object_key
      )
    )
  THEN
    RAISE EXCEPTION 'attachment backup head changed'
      USING ERRCODE = '40001';
  END IF;

  IF v_has_current THEN
    v_displaced := v_current;

    UPDATE public.attachment_backup_objects AS backup
    SET
      state = 'deleting',
      deletion_requested_at = v_now,
      gc_lease_id = NULL,
      gc_lease_expires_at = NULL,
      updated_at = v_now
    WHERE backup.id = v_current.id;
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'current',
    updated_at = v_now
  WHERE backup.id = v_candidate.id
  RETURNING backup.* INTO v_candidate;

  RETURN QUERY
  SELECT
    v_candidate.id,
    v_candidate.object_key,
    v_candidate.version_ref,
    v_candidate.ciphertext_sha256,
    v_displaced.id,
    v_displaced.object_key,
    true;
END;
$$;

CREATE OR REPLACE FUNCTION public.prepare_attachment_backup_download(
  p_owner_user_id uuid,
  p_object_key text,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  ciphertext_sha256 text,
  ciphertext_size_bytes bigint,
  format_version smallint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup public.attachment_backup_objects%ROWTYPE;
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  IF p_object_key IS NULL
    OR p_object_key !~ (
      '^'
      || p_owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
    OR p_download_expires_at IS NULL
    OR p_download_expires_at <= v_now
    OR p_download_expires_at > v_now + interval '2 hours 5 minutes'
  THEN
    RAISE EXCEPTION 'invalid attachment backup download request'
      USING ERRCODE = '22023';
  END IF;

  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.object_key = p_object_key
  FOR UPDATE;

  IF NOT FOUND OR v_backup.state <> 'current' THEN
    RAISE EXCEPTION 'attachment backup current object is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    cleanup_not_before = GREATEST(
      backup.cleanup_not_before,
      p_download_expires_at + interval '5 minutes'
    ),
    updated_at = v_now
  WHERE backup.id = v_backup.id
  RETURNING backup.* INTO v_backup;

  RETURN QUERY
  SELECT
    v_backup.id,
    v_backup.object_key,
    v_backup.ciphertext_sha256,
    v_backup.ciphertext_size_bytes,
    v_backup.format_version,
    v_backup.cleanup_not_before;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_attachment_backup_deleting(
  p_owner_user_id uuid,
  p_object_id uuid
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  ciphertext_size_bytes bigint,
  cleanup_not_before timestamptz,
  was_marked boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup public.attachment_backup_objects%ROWTYPE;
  v_was_marked boolean;
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = p_object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup object is unavailable'
      USING ERRCODE = '55000';
  END IF;

  v_was_marked := v_backup.state <> 'deleting';
  IF v_was_marked THEN
    UPDATE public.attachment_backup_objects AS backup
    SET
      state = 'deleting',
      deletion_requested_at = v_now,
      gc_lease_id = NULL,
      gc_lease_expires_at = NULL,
      updated_at = v_now
    WHERE backup.id = v_backup.id
    RETURNING backup.* INTO v_backup;
  END IF;

  RETURN QUERY
  SELECT
    v_backup.id,
    v_backup.object_key,
    v_backup.ciphertext_size_bytes,
    v_backup.cleanup_not_before,
    v_was_marked;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_attachment_backup_deleting_by_key(
  p_owner_user_id uuid,
  p_object_key text
)
RETURNS TABLE (
  object_id uuid,
  object_key text,
  ciphertext_size_bytes bigint,
  cleanup_not_before timestamptz,
  was_marked boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_object_id uuid;
BEGIN
  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  IF p_object_key IS NULL
    OR p_object_key !~ (
      '^'
      || p_owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  THEN
    RAISE EXCEPTION 'invalid attachment backup object key'
      USING ERRCODE = '22023';
  END IF;

  SELECT backup.id
  INTO v_object_id
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id
    AND backup.object_key = p_object_key
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup object is unavailable'
      USING ERRCODE = '55000';
  END IF;

  RETURN QUERY
  SELECT
    marked.object_id,
    marked.object_key,
    marked.ciphertext_size_bytes,
    marked.cleanup_not_before,
    marked.was_marked
  FROM public.mark_attachment_backup_deleting(
    p_owner_user_id,
    v_object_id
  ) AS marked;
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_attachment_backup_gc_leases(
  p_lease_id uuid,
  p_limit integer DEFAULT 100,
  p_lease_seconds integer DEFAULT 300
)
RETURNS TABLE (
  object_id uuid,
  owner_user_id uuid,
  object_key text,
  ciphertext_size_bytes bigint,
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
    OR p_limit < 1
    OR p_limit > 1000
    OR p_lease_seconds IS NULL
    OR p_lease_seconds < 30
    OR p_lease_seconds > 3600
  THEN
    RAISE EXCEPTION 'invalid attachment backup GC lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT backup.id
    FROM public.attachment_backup_objects AS backup
    WHERE backup.state IN ('reserved', 'ready', 'deleting')
      AND backup.cleanup_not_before <= v_now
      AND (
        backup.gc_lease_expires_at IS NULL
        OR backup.gc_lease_expires_at <= v_now
      )
    ORDER BY backup.cleanup_not_before, backup.created_at, backup.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE public.attachment_backup_objects AS backup
    SET
      state = 'deleting',
      deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
      gc_lease_id = p_lease_id,
      gc_lease_expires_at = v_now + make_interval(secs => p_lease_seconds),
      updated_at = v_now
    FROM candidates
    WHERE backup.id = candidates.id
    RETURNING backup.*
  )
  SELECT
    leased.id,
    leased.owner_user_id,
    leased.object_key,
    leased.ciphertext_size_bytes,
    leased.gc_lease_id,
    leased.gc_lease_expires_at
  FROM leased
  ORDER BY leased.cleanup_not_before, leased.created_at, leased.id;
END;
$$;

CREATE OR REPLACE FUNCTION public.finish_attachment_backup_deletion(
  p_owner_user_id uuid,
  p_object_id uuid,
  p_object_key text,
  p_gc_lease_id uuid DEFAULT NULL
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup public.attachment_backup_objects%ROWTYPE;
BEGIN
  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = p_object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF v_backup.state <> 'deleting'
    OR v_backup.object_key <> p_object_key
    OR v_backup.cleanup_not_before > v_now
    OR (
      v_backup.gc_lease_id IS NOT NULL
      AND v_backup.gc_lease_id IS DISTINCT FROM p_gc_lease_id
    )
  THEN
    RAISE EXCEPTION 'attachment backup deletion is unavailable'
      USING ERRCODE = '55000';
  END IF;

  DELETE FROM public.attachment_backup_objects AS backup
  WHERE backup.id = v_backup.id;

  RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION public.read_attachment_backup_by_key(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_current_attachment_backup(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.reserve_attachment_backup(uuid, text, text, bigint, smallint)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_attachment_backup_signed(uuid, uuid, text, timestamptz)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finalize_attachment_backup(uuid, uuid, text, bigint)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.promote_attachment_backup(uuid, uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.prepare_attachment_backup_download(uuid, text, timestamptz)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_attachment_backup_deleting(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_attachment_backup_deleting_by_key(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.claim_attachment_backup_gc_leases(uuid, integer, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finish_attachment_backup_deletion(uuid, uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.read_attachment_backup_by_key(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.read_current_attachment_backup(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.reserve_attachment_backup(uuid, text, text, bigint, smallint)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_attachment_backup_signed(uuid, uuid, text, timestamptz)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.finalize_attachment_backup(uuid, uuid, text, bigint)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.promote_attachment_backup(uuid, uuid, text, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.prepare_attachment_backup_download(uuid, text, timestamptz)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_attachment_backup_deleting(uuid, uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_attachment_backup_deleting_by_key(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.claim_attachment_backup_gc_leases(uuid, integer, integer)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.finish_attachment_backup_deletion(uuid, uuid, text, uuid)
  TO service_role;
