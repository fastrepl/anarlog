CREATE TABLE public.e2ee_device_enrollment_requests (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  device_fingerprint text NOT NULL,
  device_name text,
  recipient_public_key text NOT NULL,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL DEFAULT (now() + interval '24 hours'),
  sealed_at timestamptz,
  consumed_at timestamptz,
  CONSTRAINT e2ee_device_enrollment_fingerprint_check CHECK (
    device_fingerprint ~ '^[A-Za-z0-9_-]{8,128}$'
  ),
  CONSTRAINT e2ee_device_enrollment_name_check CHECK (
    device_name IS NULL OR octet_length(device_name) BETWEEN 1 AND 128
  ),
  CONSTRAINT e2ee_device_enrollment_public_key_check CHECK (
    recipient_public_key ~ '^[A-Za-z0-9_-]{43}$'
  ),
  CONSTRAINT e2ee_device_enrollment_package_check CHECK (
    (
      sealed_at IS NULL
      AND consumed_at IS NULL
      AND ephemeral_public_key IS NULL
      AND nonce IS NULL
      AND ciphertext IS NULL
    )
    OR (
      sealed_at IS NOT NULL
      AND consumed_at IS NULL
      AND ephemeral_public_key ~ '^[A-Za-z0-9_-]{43}$'
      AND nonce ~ '^[A-Za-z0-9_-]{32}$'
      AND ciphertext ~ '^[A-Za-z0-9_-]+$'
      AND octet_length(ciphertext) BETWEEN 64 AND 2048
    )
    OR (
      sealed_at IS NOT NULL
      AND consumed_at IS NOT NULL
      AND ephemeral_public_key IS NULL
      AND nonce IS NULL
      AND ciphertext IS NULL
    )
  ),
  CONSTRAINT e2ee_device_enrollment_timestamps_check CHECK (
    expires_at > created_at
    AND (sealed_at IS NULL OR sealed_at >= created_at)
    AND (consumed_at IS NULL OR consumed_at >= sealed_at)
  ),
  CONSTRAINT e2ee_device_enrollment_user_public_key_key UNIQUE (
    user_id,
    recipient_public_key
  ),
  CONSTRAINT e2ee_device_enrollment_user_fingerprint_key UNIQUE (
    user_id,
    device_fingerprint
  )
);

ALTER TABLE public.e2ee_device_enrollment_requests ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.e2ee_device_enrollment_requests
  FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE
  ON TABLE public.e2ee_device_enrollment_requests
  TO service_role;
GRANT SELECT ON TABLE public.sync_devices TO service_role;

CREATE OR REPLACE FUNCTION private.sync_device_slot_count(
  p_actor_user_id uuid
)
RETURNS bigint
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT COUNT(*)
  FROM (
    SELECT device.device_fingerprint
    FROM public.sync_devices AS device
    WHERE device.user_id = p_actor_user_id

    UNION

    SELECT enrollment.device_fingerprint
    FROM public.e2ee_device_enrollment_requests AS enrollment
    WHERE enrollment.user_id = p_actor_user_id
      AND enrollment.expires_at > now()
  ) AS occupied_slots;
$$;

REVOKE ALL ON FUNCTION private.sync_device_slot_count(uuid)
  FROM PUBLIC, anon, authenticated;

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
  v_max_devices constant integer := 5;
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
    AND private.sync_device_slot_count(p_actor_user_id) >= v_max_devices
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

CREATE OR REPLACE FUNCTION public.seal_e2ee_device_enrollment(
  p_actor_user_id uuid,
  p_request_id uuid,
  p_ephemeral_public_key text,
  p_nonce text,
  p_ciphertext text
)
RETURNS TABLE (result text)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result text;
BEGIN
  IF p_actor_user_id IS NULL
    OR p_request_id IS NULL
    OR p_ephemeral_public_key IS NULL
    OR p_ephemeral_public_key !~ '^[A-Za-z0-9_-]{43}$'
    OR p_nonce IS NULL
    OR p_nonce !~ '^[A-Za-z0-9_-]{32}$'
    OR p_ciphertext IS NULL
    OR p_ciphertext !~ '^[A-Za-z0-9_-]+$'
    OR octet_length(p_ciphertext) NOT BETWEEN 64 AND 2048
  THEN
    RAISE EXCEPTION 'E2EE device enrollment package is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.expires_at <= now();

  UPDATE public.e2ee_device_enrollment_requests AS enrollment
  SET ephemeral_public_key = p_ephemeral_public_key,
      nonce = p_nonce,
      ciphertext = p_ciphertext,
      sealed_at = now()
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.id = p_request_id
    AND enrollment.sealed_at IS NULL
    AND enrollment.consumed_at IS NULL
    AND enrollment.expires_at > now();

  IF FOUND THEN
    v_result := 'sealed';
  ELSE
    SELECT CASE
      WHEN enrollment.ephemeral_public_key = p_ephemeral_public_key
        AND enrollment.nonce = p_nonce
        AND enrollment.ciphertext = p_ciphertext
        AND enrollment.consumed_at IS NULL
      THEN 'sealed'
      WHEN enrollment.id IS NOT NULL THEN 'conflict'
      ELSE 'unavailable'
    END
    INTO v_result
    FROM public.e2ee_device_enrollment_requests AS enrollment
    WHERE enrollment.user_id = p_actor_user_id
      AND enrollment.id = p_request_id
      AND enrollment.expires_at > now();

    v_result := COALESCE(v_result, 'unavailable');
  END IF;

  RETURN QUERY SELECT v_result;
END;
$$;

REVOKE ALL ON FUNCTION public.seal_e2ee_device_enrollment(uuid, uuid, text, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.seal_e2ee_device_enrollment(uuid, uuid, text, text, text)
  TO service_role;

CREATE OR REPLACE FUNCTION public.consume_e2ee_device_enrollment(
  p_actor_user_id uuid,
  p_request_id uuid,
  p_device_fingerprint text,
  p_recipient_public_key text
)
RETURNS TABLE (consumed boolean)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_actor_user_id IS NULL
    OR p_request_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
    OR p_recipient_public_key IS NULL
    OR p_recipient_public_key !~ '^[A-Za-z0-9_-]{43}$'
  THEN
    RAISE EXCEPTION 'E2EE device enrollment is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.expires_at <= now();

  UPDATE public.e2ee_device_enrollment_requests AS enrollment
  SET consumed_at = COALESCE(enrollment.consumed_at, now()),
      ephemeral_public_key = NULL,
      nonce = NULL,
      ciphertext = NULL
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.id = p_request_id
    AND enrollment.device_fingerprint = p_device_fingerprint
    AND enrollment.recipient_public_key = p_recipient_public_key
    AND enrollment.sealed_at IS NOT NULL
    AND enrollment.expires_at > now();

  RETURN QUERY SELECT FOUND;
END;
$$;

REVOKE ALL ON FUNCTION public.consume_e2ee_device_enrollment(uuid, uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.consume_e2ee_device_enrollment(uuid, uuid, text, text)
  TO service_role;

CREATE OR REPLACE FUNCTION public.remove_sync_device(
  p_actor_user_id uuid,
  p_device_fingerprint text
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_actor_user_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
  THEN
    RAISE EXCEPTION 'Sync device identity is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  DELETE FROM public.sync_devices AS device
  WHERE device.user_id = p_actor_user_id
    AND device.device_fingerprint = p_device_fingerprint;

  DELETE FROM public.e2ee_device_enrollment_requests AS enrollment
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.device_fingerprint = p_device_fingerprint;
END;
$$;

REVOKE ALL ON FUNCTION public.remove_sync_device(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.remove_sync_device(uuid, text)
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
  v_max_devices constant integer := 5;
  v_claimed boolean;
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
      device_name = COALESCE(NULLIF(TRIM(p_device_name), ''), device.device_name)
  WHERE device.user_id = p_actor_user_id
    AND device.device_fingerprint = p_device_fingerprint;
  v_claimed := FOUND;

  SELECT EXISTS (
    SELECT 1
    FROM public.e2ee_device_enrollment_requests AS enrollment
    WHERE enrollment.user_id = p_actor_user_id
      AND enrollment.device_fingerprint = p_device_fingerprint
      AND enrollment.expires_at > now()
  )
  INTO v_has_enrollment;

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
      NULLIF(TRIM(p_device_name), '')
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
