BEGIN;

SET LOCAL lock_timeout = '30s';

CREATE OR REPLACE FUNCTION private.sync_device_limit(p_actor_user_id uuid)
RETURNS integer
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM public.workspace_memberships AS membership
    WHERE membership.user_id = p_actor_user_id
      AND membership.deleted_at IS NULL
      AND private.workspace_capabilities(membership.workspace_id)
        @> ARRAY['team.shared_notes']::text[]
  ) THEN 5 ELSE 3 END;
$$;

REVOKE ALL ON FUNCTION private.sync_device_limit(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.sync_device_limit(uuid) TO service_role;

CREATE OR REPLACE FUNCTION public.get_sync_device_limit(p_actor_user_id uuid)
RETURNS integer
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT private.sync_device_limit(p_actor_user_id);
$$;

REVOKE ALL ON FUNCTION public.get_sync_device_limit(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.get_sync_device_limit(uuid) TO service_role;

CREATE OR REPLACE FUNCTION public.register_e2ee_device_enrollment(
  p_actor_user_id uuid,
  p_device_fingerprint text,
  p_device_name text,
  p_recipient_public_key text,
  p_replace_device_fingerprint text DEFAULT NULL
)
RETURNS TABLE (
  allowed boolean,
  requires_existing_key boolean,
  request_id uuid,
  expires_at timestamptz,
  enrollment_status text,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  device_count bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_enrollment public.e2ee_device_enrollment_requests%ROWTYPE;
  v_max_devices integer := private.sync_device_limit(p_actor_user_id);
  v_reuses_active_slot boolean;
BEGIN
  IF p_actor_user_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
    OR p_recipient_public_key IS NULL
    OR p_recipient_public_key !~ '^[A-Za-z0-9_-]{43}$'
    OR (
      p_device_name IS NOT NULL
      AND octet_length(NULLIF(TRIM(p_device_name), '')) NOT BETWEEN 1 AND 128
    )
    OR (
      p_replace_device_fingerprint IS NOT NULL
      AND (
        p_replace_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
        OR p_replace_device_fingerprint = p_device_fingerprint
      )
    )
  THEN
    RAISE EXCEPTION 'E2EE device enrollment is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.expires_at <= now();

  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    WHERE workspace.id = p_actor_user_id
      AND workspace.owner_user_id = p_actor_user_id
      AND workspace.kind = 'personal'
      AND workspace.deleted_at IS NULL
      AND workspace.e2ee_key_id IS NOT NULL
  ) THEN
    RETURN QUERY
    SELECT
      false,
      true,
      NULL::uuid,
      NULL::timestamptz,
      NULL::text,
      NULL::text,
      NULL::text,
      NULL::text,
      private.sync_device_slot_count(p_actor_user_id);
    RETURN;
  END IF;

  SELECT enrollment.*
  INTO v_enrollment
  FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.device_fingerprint = p_device_fingerprint
    AND enrollment.recipient_public_key = p_recipient_public_key
    AND enrollment.expires_at > now()
  FOR UPDATE;

  IF FOUND AND v_enrollment.consumed_at IS NULL THEN
    UPDATE public.e2ee_device_enrollment_requests AS enrollment
    SET device_name = COALESCE(
      NULLIF(TRIM(p_device_name), ''),
      enrollment.device_name
    )
    WHERE enrollment.id = v_enrollment.id
    RETURNING enrollment.*
    INTO v_enrollment;

    RETURN QUERY
    SELECT
      true,
      false,
      v_enrollment.id,
      v_enrollment.expires_at,
      CASE
        WHEN v_enrollment.consumed_at IS NOT NULL THEN 'consumed'
        WHEN v_enrollment.sealed_at IS NOT NULL THEN 'sealed'
        ELSE 'pending'
      END,
      v_enrollment.ephemeral_public_key,
      v_enrollment.nonce,
      v_enrollment.ciphertext,
      private.sync_device_slot_count(p_actor_user_id);
    RETURN;
  END IF;

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND (
      enrollment.device_fingerprint = p_device_fingerprint
      OR enrollment.recipient_public_key = p_recipient_public_key
    );

  SELECT EXISTS (
    SELECT 1
    FROM public.sync_devices AS device
    WHERE device.user_id = p_actor_user_id
      AND device.device_fingerprint = p_device_fingerprint
  )
  INTO v_reuses_active_slot;

  IF NOT v_reuses_active_slot
    AND private.sync_device_slot_count(p_actor_user_id) = v_max_devices
    AND p_replace_device_fingerprint IS NOT NULL
  THEN
    DELETE FROM public.sync_devices AS device
    WHERE device.user_id = p_actor_user_id
      AND device.device_fingerprint = p_replace_device_fingerprint;

    DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
    WHERE enrollment.user_id = p_actor_user_id
      AND enrollment.device_fingerprint = p_replace_device_fingerprint;
  END IF;

  IF NOT v_reuses_active_slot
    AND private.sync_device_slot_count(p_actor_user_id) >= v_max_devices
  THEN
    RETURN QUERY
    SELECT
      false,
      false,
      NULL::uuid,
      NULL::timestamptz,
      NULL::text,
      NULL::text,
      NULL::text,
      NULL::text,
      private.sync_device_slot_count(p_actor_user_id);
    RETURN;
  END IF;

  INSERT INTO public.e2ee_device_enrollment_requests (
    user_id,
    device_fingerprint,
    device_name,
    recipient_public_key
  )
  VALUES (
    p_actor_user_id,
    p_device_fingerprint,
    NULLIF(TRIM(p_device_name), ''),
    p_recipient_public_key
  )
  RETURNING *
  INTO v_enrollment;

  RETURN QUERY
  SELECT
    true,
    false,
    v_enrollment.id,
    v_enrollment.expires_at,
    'pending'::text,
    NULL::text,
    NULL::text,
    NULL::text,
    private.sync_device_slot_count(p_actor_user_id);
END;
$$;

REVOKE ALL ON FUNCTION public.register_e2ee_device_enrollment(uuid, text, text, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.register_e2ee_device_enrollment(uuid, text, text, text, text)
  TO service_role;


CREATE OR REPLACE FUNCTION public.claim_sync_device(
  p_actor_user_id uuid,
  p_device_fingerprint text,
  p_device_name text DEFAULT NULL
)
RETURNS TABLE (allowed boolean, device_count bigint)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_max_devices integer := private.sync_device_limit(p_actor_user_id);
  v_claimed boolean;
  v_enrollment_device_name text;
  v_has_enrollment boolean;
BEGIN
  IF p_actor_user_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
  THEN
    RAISE EXCEPTION 'Sync device identity is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.expires_at <= now();

  UPDATE public.sync_devices AS device
  SET last_seen_at = now(),
      device_name = COALESCE(device.device_name, NULLIF(TRIM(p_device_name), ''))
  WHERE device.user_id = p_actor_user_id
    AND device.device_fingerprint = p_device_fingerprint;
  v_claimed := FOUND;

  SELECT enrollment.device_name
  INTO v_enrollment_device_name
  FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.device_fingerprint = p_device_fingerprint
    AND enrollment.expires_at > now()
  LIMIT 1;
  v_has_enrollment := FOUND;

  IF NOT v_claimed
    AND (
      v_has_enrollment
      OR private.sync_device_slot_count(p_actor_user_id) < v_max_devices
    )
  THEN
    INSERT INTO public.sync_devices (user_id, device_fingerprint, device_name)
    VALUES (
      p_actor_user_id,
      p_device_fingerprint,
      COALESCE(
        NULLIF(TRIM(v_enrollment_device_name), ''),
        NULLIF(TRIM(p_device_name), '')
      )
    );
    v_claimed := true;
  END IF;

  IF v_claimed THEN
    DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
    WHERE enrollment.user_id = p_actor_user_id
      AND enrollment.device_fingerprint = p_device_fingerprint;
  END IF;

  RETURN QUERY
  SELECT
    v_claimed,
    private.sync_device_slot_count(p_actor_user_id);
END;
$$;

REVOKE ALL ON FUNCTION public.claim_sync_device(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.claim_sync_device(uuid, text, text)
  TO service_role;

COMMIT;
