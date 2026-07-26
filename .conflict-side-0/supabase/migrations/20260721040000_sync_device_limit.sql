CREATE TABLE public.sync_devices (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  device_fingerprint text NOT NULL,
  device_name text,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT sync_devices_fingerprint_check CHECK (
    device_fingerprint ~ '^[A-Za-z0-9_-]{8,128}$'
  ),
  CONSTRAINT sync_devices_user_fingerprint_key UNIQUE (user_id, device_fingerprint)
);

ALTER TABLE public.sync_devices ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can view their own sync devices"
  ON public.sync_devices
  FOR SELECT
  USING ((SELECT auth.uid()) = user_id);

CREATE POLICY "Users can remove their own sync devices"
  ON public.sync_devices
  FOR DELETE
  USING ((SELECT auth.uid()) = user_id);

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
BEGIN
  IF p_actor_user_id IS NULL
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
  THEN
    RAISE EXCEPTION 'Sync device identity is invalid' USING ERRCODE = '22023';
  END IF;

  -- Serialize claims per user so concurrent token requests cannot
  -- register more devices than the limit allows.
  PERFORM pg_advisory_xact_lock(hashtextextended('sync_devices:' || p_actor_user_id::text, 0));

  UPDATE public.sync_devices AS device
  SET last_seen_at = now(),
      device_name = COALESCE(NULLIF(TRIM(p_device_name), ''), device.device_name)
  WHERE device.user_id = p_actor_user_id
    AND device.device_fingerprint = p_device_fingerprint;
  v_claimed := FOUND;

  IF NOT v_claimed THEN
    IF (
      SELECT COUNT(*)
      FROM public.sync_devices AS device
      WHERE device.user_id = p_actor_user_id
    ) < v_max_devices THEN
      INSERT INTO public.sync_devices (user_id, device_fingerprint, device_name)
      VALUES (
        p_actor_user_id,
        p_device_fingerprint,
        NULLIF(TRIM(p_device_name), '')
      );
      v_claimed := true;
    END IF;
  END IF;

  RETURN QUERY
  SELECT
    v_claimed,
    (
      SELECT COUNT(*)
      FROM public.sync_devices AS device
      WHERE device.user_id = p_actor_user_id
    );
END;
$$;

REVOKE ALL ON FUNCTION public.claim_sync_device(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.claim_sync_device(uuid, text, text)
  TO service_role;
