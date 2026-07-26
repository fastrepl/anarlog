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
  v_now timestamptz;
  v_share public.session_shares%ROWTYPE;
  v_owner_user_id uuid;
  v_object public.session_share_attachment_objects%ROWTYPE;
  v_object_id uuid;
  v_owner_bytes bigint;
  v_owner_object_count bigint;
  v_owner_active_reservations bigint;
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

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_owner_user_id::text, 170003)
  );
  v_now := clock_timestamp();

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

  SELECT
    COALESCE(sum(attachment.size_bytes), 0),
    count(*),
    count(*) FILTER (
      WHERE attachment.state = 'reserved'
        AND attachment.cleanup_not_before > v_now
    )
  INTO
    v_owner_bytes,
    v_owner_object_count,
    v_owner_active_reservations
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.owner_user_id = v_owner_user_id;

  IF v_owner_object_count >= 10000 THEN
    RAISE EXCEPTION 'shared attachment object limit exceeded'
      USING ERRCODE = '54000';
  END IF;

  IF p_size_bytes > 5368709120 - v_owner_bytes THEN
    RAISE EXCEPTION 'shared attachment storage quota exceeded'
      USING ERRCODE = '54000';
  END IF;

  IF v_owner_active_reservations >= 5 THEN
    RAISE EXCEPTION 'shared attachment reservation limit exceeded'
      USING ERRCODE = '55000';
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
