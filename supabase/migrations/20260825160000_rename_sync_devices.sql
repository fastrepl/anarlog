CREATE OR REPLACE FUNCTION public.rename_sync_device(
  p_actor_user_id uuid,
  p_device_fingerprint text,
  p_device_name text
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_renamed boolean;
BEGIN
  IF p_actor_user_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
    OR p_device_name IS NULL
    OR octet_length(TRIM(p_device_name)) NOT BETWEEN 1 AND 128
  THEN
    RAISE EXCEPTION 'Sync device name is invalid' USING ERRCODE = '22023';
  END IF;

  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  UPDATE public.sync_devices AS device
  SET device_name = TRIM(p_device_name)
  WHERE device.user_id = p_actor_user_id
    AND device.device_fingerprint = p_device_fingerprint;
  v_renamed := FOUND;

  UPDATE public.e2ee_device_enrollment_requests AS enrollment
  SET device_name = TRIM(p_device_name)
  WHERE enrollment.user_id = p_actor_user_id
    AND enrollment.device_fingerprint = p_device_fingerprint
    AND enrollment.expires_at > now();
  v_renamed := FOUND OR v_renamed;

  RETURN v_renamed;
END;
$$;

REVOKE ALL ON FUNCTION public.rename_sync_device(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.rename_sync_device(uuid, text, text)
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
