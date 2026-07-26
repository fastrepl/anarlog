ALTER TABLE public.attachment_backup_objects
  ADD COLUMN delete_generation bigint NOT NULL DEFAULT 0,
  ADD COLUMN delete_request_id uuid,
  ADD COLUMN delete_fence_id uuid,
  ADD COLUMN delete_not_before timestamptz,
  ADD CONSTRAINT attachment_backup_objects_delete_generation_check CHECK (
    delete_generation >= 0
  ),
  ADD CONSTRAINT attachment_backup_objects_delete_fence_check CHECK (
    (
      delete_request_id IS NULL
      AND delete_fence_id IS NULL
      AND delete_not_before IS NULL
    )
    OR (
      state = 'current'
      AND delete_request_id IS NOT NULL
      AND delete_fence_id IS NOT NULL
      AND delete_not_before IS NOT NULL
      AND delete_not_before >= cleanup_not_before
      AND delete_not_before >= created_at
      AND deletion_requested_at IS NULL
      AND gc_lease_id IS NULL
      AND gc_lease_expires_at IS NULL
    )
  );

CREATE INDEX attachment_backup_objects_delete_due_idx
  ON public.attachment_backup_objects(delete_not_before, created_at, id)
  WHERE state = 'current' AND delete_fence_id IS NOT NULL;

CREATE TABLE private.attachment_backup_delete_requests (
  owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  delete_request_id uuid NOT NULL,
  object_id uuid,
  attachment_ref text NOT NULL,
  version_ref text NOT NULL,
  object_key text NOT NULL,
  fence_id uuid,
  fence_generation bigint,
  delete_not_before timestamptz,
  outcome text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (owner_user_id, delete_request_id),
  CONSTRAINT attachment_backup_delete_requests_attachment_ref_check CHECK (
    attachment_ref ~ '^[A-Za-z0-9_-]{43}$'
  ),
  CONSTRAINT attachment_backup_delete_requests_version_ref_check CHECK (
    version_ref ~ '^[A-Za-z0-9_-]{43}$'
    AND version_ref <> attachment_ref
  ),
  CONSTRAINT attachment_backup_delete_requests_object_key_check CHECK (
    object_key ~ (
      '^'
      || owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  ),
  CONSTRAINT attachment_backup_delete_requests_outcome_check CHECK (
    outcome IN ('scheduled', 'dependency_appeared', 'cancelled')
  ),
  CONSTRAINT attachment_backup_delete_requests_generation_check CHECK (
    fence_generation IS NULL OR fence_generation >= 0
  ),
  CONSTRAINT attachment_backup_delete_requests_shape_check CHECK (
    (
      object_id IS NOT NULL
      AND fence_id IS NOT NULL
      AND fence_generation IS NOT NULL
      AND delete_not_before IS NOT NULL
    )
    OR (
      outcome = 'cancelled'
      AND object_id IS NULL
      AND fence_id IS NULL
      AND fence_generation IS NULL
      AND delete_not_before IS NULL
    )
  ),
  CONSTRAINT attachment_backup_delete_requests_time_check CHECK (
    updated_at >= created_at
    AND (
      delete_not_before IS NULL
      OR delete_not_before >= created_at
    )
  ),
  CONSTRAINT attachment_backup_delete_requests_fence_key UNIQUE (fence_id)
);

ALTER TABLE private.attachment_backup_delete_requests ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE private.attachment_backup_delete_requests
  FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE
  ON TABLE private.attachment_backup_delete_requests TO service_role;

CREATE POLICY attachment_backup_delete_requests_service_all
  ON private.attachment_backup_delete_requests
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.invalidate_attachment_backup_delete(
  p_owner_user_id uuid,
  p_object_id uuid,
  p_delete_request_id uuid,
  p_delete_fence_id uuid,
  p_delete_generation bigint,
  p_now timestamptz
)
RETURNS void
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  UPDATE private.attachment_backup_delete_requests AS deletion
  SET
    outcome = 'dependency_appeared',
    updated_at = GREATEST(deletion.updated_at, p_now)
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.delete_request_id = p_delete_request_id
    AND deletion.object_id = p_object_id
    AND deletion.fence_id = p_delete_fence_id
    AND deletion.fence_generation = p_delete_generation
    AND deletion.outcome = 'scheduled';

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup delete request is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    delete_generation = backup.delete_generation + 1,
    delete_request_id = NULL,
    delete_fence_id = NULL,
    delete_not_before = NULL,
    updated_at = GREATEST(backup.updated_at, p_now)
  WHERE backup.id = p_object_id
    AND backup.owner_user_id = p_owner_user_id
    AND backup.state = 'current'
    AND backup.delete_request_id = p_delete_request_id
    AND backup.delete_fence_id = p_delete_fence_id
    AND backup.delete_generation = p_delete_generation;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup delete fence changed'
      USING ERRCODE = '40001';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.invalidate_attachment_backup_delete(
  uuid, uuid, uuid, uuid, bigint, timestamptz
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.invalidate_attachment_backup_delete(
  uuid, uuid, uuid, uuid, bigint, timestamptz
) TO service_role;

CREATE OR REPLACE FUNCTION public.schedule_attachment_backup_deletion(
  p_owner_user_id uuid,
  p_attachment_ref text,
  p_version_ref text,
  p_object_key text,
  p_delete_request_id uuid
)
RETURNS TABLE (
  outcome text,
  object_id uuid,
  object_key text,
  delete_request_id uuid,
  delete_fence_id uuid,
  delete_generation bigint,
  delete_not_before timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_request private.attachment_backup_delete_requests%ROWTYPE;
  v_backup public.attachment_backup_objects%ROWTYPE;
  v_fence_id uuid;
  v_generation bigint;
  v_not_before timestamptz;
BEGIN
  IF p_delete_request_id IS NULL
    OR p_attachment_ref IS NULL
    OR p_attachment_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref IS NULL
    OR p_version_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref = p_attachment_ref
    OR p_object_key IS NULL
    OR p_object_key !~ (
      '^'
      || p_owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  THEN
    RAISE EXCEPTION 'invalid attachment backup deletion request'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT deletion.*
  INTO v_request
  FROM private.attachment_backup_delete_requests AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.delete_request_id = p_delete_request_id;

  IF FOUND THEN
    IF v_request.attachment_ref <> p_attachment_ref
      OR v_request.version_ref <> p_version_ref
      OR v_request.object_key <> p_object_key
    THEN
      RAISE EXCEPTION 'attachment backup delete request conflicts with existing identity'
        USING ERRCODE = '40001';
    END IF;

    RETURN QUERY SELECT
      v_request.outcome,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.object_id
        ELSE NULL::uuid
      END,
      v_request.object_key,
      v_request.delete_request_id,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.fence_id
        ELSE NULL::uuid
      END,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.fence_generation
        ELSE NULL::bigint
      END,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.delete_not_before
        ELSE NULL::timestamptz
      END,
      false;
    RETURN;
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

  IF v_backup.attachment_ref <> p_attachment_ref
    OR v_backup.version_ref <> p_version_ref
  THEN
    RAISE EXCEPTION 'attachment backup deletion identity changed'
      USING ERRCODE = '40001';
  END IF;

  IF v_backup.delete_request_id IS NOT NULL THEN
    RAISE EXCEPTION 'attachment backup deletion is already pending'
      USING ERRCODE = '40001';
  END IF;

  v_fence_id := extensions.gen_random_uuid();
  v_generation := v_backup.delete_generation + 1;
  v_not_before := GREATEST(
    v_now + interval '24 hours',
    v_backup.cleanup_not_before
  );

  INSERT INTO private.attachment_backup_delete_requests (
    owner_user_id,
    delete_request_id,
    object_id,
    attachment_ref,
    version_ref,
    object_key,
    fence_id,
    fence_generation,
    delete_not_before,
    outcome,
    created_at,
    updated_at
  ) VALUES (
    p_owner_user_id,
    p_delete_request_id,
    v_backup.id,
    p_attachment_ref,
    p_version_ref,
    p_object_key,
    v_fence_id,
    v_generation,
    v_not_before,
    'scheduled',
    v_now,
    v_now
  )
  ON CONFLICT ON CONSTRAINT attachment_backup_delete_requests_pkey
    DO NOTHING
  RETURNING * INTO v_request;

  IF NOT FOUND THEN
    SELECT deletion.*
    INTO v_request
    FROM private.attachment_backup_delete_requests AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
      AND deletion.delete_request_id = p_delete_request_id;

    IF NOT FOUND
      OR v_request.attachment_ref <> p_attachment_ref
      OR v_request.version_ref <> p_version_ref
      OR v_request.object_key <> p_object_key
    THEN
      RAISE EXCEPTION 'attachment backup delete request conflicts with existing identity'
        USING ERRCODE = '40001';
    END IF;

    RETURN QUERY SELECT
      v_request.outcome,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.object_id
        ELSE NULL::uuid
      END,
      v_request.object_key,
      v_request.delete_request_id,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.fence_id
        ELSE NULL::uuid
      END,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.fence_generation
        ELSE NULL::bigint
      END,
      CASE
        WHEN v_request.outcome = 'scheduled' THEN v_request.delete_not_before
        ELSE NULL::timestamptz
      END,
      false;
    RETURN;
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    delete_generation = v_generation,
    delete_request_id = p_delete_request_id,
    delete_fence_id = v_fence_id,
    delete_not_before = v_not_before,
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.id = v_backup.id
    AND backup.state = 'current'
    AND backup.delete_request_id IS NULL
    AND backup.delete_generation = v_backup.delete_generation
  RETURNING backup.* INTO v_backup;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup delete fence changed'
      USING ERRCODE = '40001';
  END IF;

  RETURN QUERY SELECT
    v_request.outcome,
    v_request.object_id,
    v_request.object_key,
    v_request.delete_request_id,
    v_request.fence_id,
    v_request.fence_generation,
    v_request.delete_not_before,
    true;
END;
$$;

CREATE OR REPLACE FUNCTION public.cancel_attachment_backup_deletion(
  p_owner_user_id uuid,
  p_attachment_ref text,
  p_version_ref text,
  p_object_key text,
  p_delete_request_id uuid
)
RETURNS TABLE (
  outcome text,
  object_key text,
  was_cancelled boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_request private.attachment_backup_delete_requests%ROWTYPE;
  v_locked_request private.attachment_backup_delete_requests%ROWTYPE;
  v_backup public.attachment_backup_objects%ROWTYPE;
BEGIN
  IF p_delete_request_id IS NULL
    OR p_attachment_ref IS NULL
    OR p_attachment_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref IS NULL
    OR p_version_ref !~ '^[A-Za-z0-9_-]{43}$'
    OR p_version_ref = p_attachment_ref
    OR p_object_key IS NULL
    OR p_object_key !~ (
      '^'
      || p_owner_user_id::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
  THEN
    RAISE EXCEPTION 'invalid attachment backup deletion cancellation'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_attachment_backup_owner(p_owner_user_id);

  SELECT deletion.*
  INTO v_request
  FROM private.attachment_backup_delete_requests AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.delete_request_id = p_delete_request_id;

  IF NOT FOUND THEN
    INSERT INTO private.attachment_backup_delete_requests (
      owner_user_id,
      delete_request_id,
      attachment_ref,
      version_ref,
      object_key,
      outcome,
      created_at,
      updated_at
    ) VALUES (
      p_owner_user_id,
      p_delete_request_id,
      p_attachment_ref,
      p_version_ref,
      p_object_key,
      'cancelled',
      v_now,
      v_now
    )
    ON CONFLICT ON CONSTRAINT attachment_backup_delete_requests_pkey
      DO NOTHING
    RETURNING * INTO v_request;

    IF FOUND THEN
      RETURN QUERY SELECT 'cancelled'::text, p_object_key, false;
      RETURN;
    END IF;

    SELECT deletion.*
    INTO v_request
    FROM private.attachment_backup_delete_requests AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
      AND deletion.delete_request_id = p_delete_request_id;
  END IF;

  IF v_request.attachment_ref <> p_attachment_ref
    OR v_request.version_ref <> p_version_ref
    OR v_request.object_key <> p_object_key
  THEN
    RAISE EXCEPTION 'attachment backup delete request conflicts with existing identity'
      USING ERRCODE = '40001';
  END IF;

  IF v_request.outcome IN ('dependency_appeared', 'cancelled') THEN
    RETURN QUERY SELECT v_request.outcome, v_request.object_key, false;
    RETURN;
  END IF;

  SELECT backup.*
  INTO v_backup
  FROM public.attachment_backup_objects AS backup
  WHERE backup.id = v_request.object_id
    AND backup.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND OR v_backup.state = 'deleting' THEN
    RAISE EXCEPTION 'attachment backup deletion is too late to cancel'
      USING ERRCODE = '55006';
  END IF;

  SELECT deletion.*
  INTO v_locked_request
  FROM private.attachment_backup_delete_requests AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.delete_request_id = p_delete_request_id
  FOR UPDATE;

  IF v_locked_request.outcome IN ('dependency_appeared', 'cancelled') THEN
    RETURN QUERY SELECT v_locked_request.outcome, v_locked_request.object_key, false;
    RETURN;
  END IF;

  IF v_backup.state <> 'current'
    OR v_backup.attachment_ref <> p_attachment_ref
    OR v_backup.version_ref <> p_version_ref
    OR v_backup.object_key <> p_object_key
    OR v_backup.delete_request_id IS DISTINCT FROM p_delete_request_id
    OR v_backup.delete_fence_id IS DISTINCT FROM v_locked_request.fence_id
    OR v_backup.delete_generation IS DISTINCT FROM v_locked_request.fence_generation
  THEN
    RAISE EXCEPTION 'attachment backup delete fence changed'
      USING ERRCODE = '40001';
  END IF;

  UPDATE private.attachment_backup_delete_requests AS deletion
  SET
    outcome = 'cancelled',
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.delete_request_id = p_delete_request_id
    AND deletion.object_id = v_backup.id
    AND deletion.fence_id = v_locked_request.fence_id
    AND deletion.fence_generation = v_locked_request.fence_generation
    AND deletion.outcome = 'scheduled';

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup delete request is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    delete_generation = backup.delete_generation + 1,
    delete_request_id = NULL,
    delete_fence_id = NULL,
    delete_not_before = NULL,
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.id = v_backup.id
    AND backup.owner_user_id = p_owner_user_id
    AND backup.state = 'current'
    AND backup.delete_request_id = p_delete_request_id
    AND backup.delete_fence_id = v_locked_request.fence_id
    AND backup.delete_generation = v_locked_request.fence_generation;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'attachment backup delete fence changed'
      USING ERRCODE = '40001';
  END IF;

  RETURN QUERY SELECT 'cancelled'::text, p_object_key, true;
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

    IF v_existing.state = 'current'
      AND v_existing.delete_request_id IS NOT NULL
    THEN
      PERFORM private.invalidate_attachment_backup_delete(
        p_owner_user_id,
        v_existing.id,
        v_existing.delete_request_id,
        v_existing.delete_fence_id,
        v_existing.delete_generation,
        v_now
      );

      SELECT backup.*
      INTO v_existing
      FROM public.attachment_backup_objects AS backup
      WHERE backup.id = v_existing.id;
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

  IF v_backup.delete_request_id IS NOT NULL THEN
    PERFORM private.invalidate_attachment_backup_delete(
      p_owner_user_id,
      v_backup.id,
      v_backup.delete_request_id,
      v_backup.delete_fence_id,
      v_backup.delete_generation,
      v_now
    );
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    cleanup_not_before = GREATEST(
      backup.cleanup_not_before,
      p_download_expires_at + interval '5 minutes'
    ),
    updated_at = GREATEST(backup.updated_at, v_now)
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

  IF p_candidate_object_key IS NULL THEN
    RAISE EXCEPTION 'invalid attachment backup candidate key'
      USING ERRCODE = '22023';
  END IF;

  IF p_candidate_object_key <> v_candidate.object_key THEN
    RAISE EXCEPTION 'attachment backup candidate key does not match'
      USING ERRCODE = '40001';
  END IF;

  IF v_candidate.state = 'current' THEN
    IF v_candidate.delete_request_id IS NOT NULL THEN
      PERFORM private.invalidate_attachment_backup_delete(
        p_owner_user_id,
        v_candidate.id,
        v_candidate.delete_request_id,
        v_candidate.delete_fence_id,
        v_candidate.delete_generation,
        v_now
      );

      SELECT backup.*
      INTO v_candidate
      FROM public.attachment_backup_objects AS backup
      WHERE backup.id = v_candidate.id;
    END IF;

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

    IF v_current.delete_request_id IS NOT NULL THEN
      PERFORM private.invalidate_attachment_backup_delete(
        p_owner_user_id,
        v_current.id,
        v_current.delete_request_id,
        v_current.delete_fence_id,
        v_current.delete_generation,
        v_now
      );
    END IF;

    UPDATE public.attachment_backup_objects AS backup
    SET
      state = 'deleting',
      deletion_requested_at = v_now,
      delete_request_id = NULL,
      delete_fence_id = NULL,
      delete_not_before = NULL,
      gc_lease_id = NULL,
      gc_lease_expires_at = NULL,
      updated_at = GREATEST(backup.updated_at, v_now)
    WHERE backup.id = v_current.id;
  END IF;

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'current',
    delete_request_id = NULL,
    delete_fence_id = NULL,
    delete_not_before = NULL,
    updated_at = GREATEST(backup.updated_at, v_now)
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
    WHERE (
        (
          backup.state IN ('reserved', 'ready', 'deleting')
          AND backup.cleanup_not_before <= v_now
        )
        OR (
          backup.state = 'current'
          AND backup.delete_request_id IS NOT NULL
          AND backup.delete_fence_id IS NOT NULL
          AND backup.delete_not_before <= v_now
          AND backup.cleanup_not_before <= v_now
          AND EXISTS (
            SELECT 1
            FROM private.attachment_backup_delete_requests AS deletion
            WHERE deletion.owner_user_id = backup.owner_user_id
              AND deletion.delete_request_id = backup.delete_request_id
              AND deletion.object_id = backup.id
              AND deletion.fence_id = backup.delete_fence_id
              AND deletion.fence_generation = backup.delete_generation
              AND deletion.delete_not_before = backup.delete_not_before
              AND deletion.outcome = 'scheduled'
          )
        )
      )
      AND (
        backup.gc_lease_expires_at IS NULL
        OR backup.gc_lease_expires_at <= v_now
      )
    ORDER BY
      COALESCE(backup.delete_not_before, backup.cleanup_not_before),
      backup.created_at,
      backup.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE public.attachment_backup_objects AS backup
    SET
      state = 'deleting',
      deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
      delete_generation = CASE
        WHEN backup.delete_request_id IS NOT NULL
          THEN backup.delete_generation + 1
        ELSE backup.delete_generation
      END,
      delete_request_id = NULL,
      delete_fence_id = NULL,
      delete_not_before = NULL,
      gc_lease_id = p_lease_id,
      gc_lease_expires_at = v_now + make_interval(secs => p_lease_seconds),
      updated_at = GREATEST(backup.updated_at, v_now)
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
  v_extension_horizon timestamptz;
  v_horizon timestamptz;
  v_was_created boolean;
BEGIN
  IF p_owner_user_id IS NULL THEN
    RAISE EXCEPTION 'invalid account deletion owner'
      USING ERRCODE = '22023';
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_owner_user_id::text, 170001)
  );

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

  IF NOT FOUND THEN
    IF v_existing_horizon IS NOT NULL THEN
      owner_user_id := p_owner_user_id;
      final_sweep_not_before := v_existing_horizon;
      was_created := false;
      RETURN NEXT;
      RETURN;
    END IF;

    RAISE EXCEPTION 'account deletion owner is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.workspaces AS workspace
  SET
    deleted_at = COALESCE(workspace.deleted_at, v_now),
    updated_at = GREATEST(workspace.updated_at, v_now)
  WHERE workspace.owner_user_id = p_owner_user_id;

  UPDATE public.workspace_memberships AS membership
  SET
    deleted_at = COALESCE(membership.deleted_at, v_now),
    updated_at = GREATEST(membership.updated_at, v_now)
  WHERE membership.user_id = p_owner_user_id
    OR EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = membership.workspace_id
        AND workspace.owner_user_id = p_owner_user_id
    );

  UPDATE private.attachment_backup_delete_requests AS deletion
  SET
    outcome = 'dependency_appeared',
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id
    AND deletion.outcome = 'scheduled'
    AND EXISTS (
      SELECT 1
      FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
        AND backup.id = deletion.object_id
        AND backup.delete_request_id = deletion.delete_request_id
        AND backup.delete_fence_id = deletion.fence_id
        AND backup.delete_generation = deletion.fence_generation
    );

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
    delete_generation = CASE
      WHEN backup.delete_request_id IS NOT NULL
        THEN backup.delete_generation + 1
      ELSE backup.delete_generation
    END,
    delete_request_id = NULL,
    delete_fence_id = NULL,
    delete_not_before = NULL,
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT max(backup.cleanup_not_before)
  INTO v_backup_horizon
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT private.prepare_account_deletion_extension(p_owner_user_id, v_now)
  INTO v_extension_horizon;

  v_horizon := GREATEST(
    COALESCE(
      v_existing_horizon,
      v_now + interval '24 hours 5 minutes'
    ),
    COALESCE(v_backup_horizon, v_existing_horizon, v_now),
    COALESCE(v_extension_horizon, v_existing_horizon, v_now)
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

REVOKE ALL ON FUNCTION public.schedule_attachment_backup_deletion(
  uuid, text, text, text, uuid
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.cancel_attachment_backup_deletion(
  uuid, text, text, text, uuid
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.schedule_attachment_backup_deletion(
  uuid, text, text, text, uuid
) TO service_role;
GRANT EXECUTE ON FUNCTION public.cancel_attachment_backup_deletion(
  uuid, text, text, text, uuid
) TO service_role;
