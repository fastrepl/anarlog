BEGIN;

SET LOCAL lock_timeout = '30s';

-- Devices that are recording the same calendar meeting announce themselves
-- here so they can agree on which one keeps the recording.
CREATE TABLE public.meeting_device_presence (
  user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  meeting_key text NOT NULL,
  device_fingerprint text NOT NULL,
  is_primary boolean NOT NULL DEFAULT false,
  seen_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, meeting_key, device_fingerprint),
  CONSTRAINT meeting_device_presence_key_check CHECK (
    meeting_key ~ '^[A-Za-z0-9_-]{16,128}$'
  ),
  CONSTRAINT meeting_device_presence_fingerprint_check CHECK (
    device_fingerprint ~ '^[A-Za-z0-9_-]{8,128}$'
  )
);

CREATE UNIQUE INDEX meeting_device_presence_one_primary
  ON public.meeting_device_presence (user_id, meeting_key)
  WHERE is_primary;

CREATE INDEX meeting_device_presence_seen_at
  ON public.meeting_device_presence (seen_at);

ALTER TABLE public.meeting_device_presence ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.meeting_device_presence
  FROM PUBLIC, anon, authenticated;

-- p_intent:
--   'present' renews this device's presence and keeps any existing primary;
--   'claim'   makes this device the primary for the meeting;
--   'release' removes this device from the meeting.
-- Returns every device still present for the meeting.
CREATE OR REPLACE FUNCTION public.heartbeat_meeting_device(
  p_actor_user_id uuid,
  p_meeting_key text,
  p_device_fingerprint text,
  p_intent text
)
RETURNS TABLE (
  device_fingerprint text,
  device_name text,
  is_primary boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
#variable_conflict use_column
BEGIN
  IF p_actor_user_id IS NULL
    OR p_meeting_key IS NULL
    OR p_meeting_key !~ '^[A-Za-z0-9_-]{16,128}$'
    OR p_device_fingerprint IS NULL
    OR p_device_fingerprint !~ '^[A-Za-z0-9_-]{8,128}$'
    OR p_intent IS NULL
    OR p_intent NOT IN ('present', 'claim', 'release')
  THEN
    RAISE EXCEPTION 'invalid meeting device presence request'
      USING ERRCODE = '22023';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM public.sync_devices AS device
    WHERE device.user_id = p_actor_user_id
      AND device.device_fingerprint = p_device_fingerprint
  ) THEN
    RAISE EXCEPTION 'sync device is not registered'
      USING ERRCODE = '42501';
  END IF;

  PERFORM pg_advisory_xact_lock(
    hashtextextended(p_actor_user_id::text || ':' || p_meeting_key, 0)
  );

  DELETE FROM public.meeting_device_presence AS presence
  WHERE presence.user_id = p_actor_user_id
    AND presence.seen_at < now() - interval '45 seconds';

  IF p_intent = 'release' THEN
    DELETE FROM public.meeting_device_presence AS presence
    WHERE presence.user_id = p_actor_user_id
      AND presence.meeting_key = p_meeting_key
      AND presence.device_fingerprint = p_device_fingerprint;
  ELSE
    IF p_intent = 'claim' THEN
      UPDATE public.meeting_device_presence AS presence
      SET is_primary = false
      WHERE presence.user_id = p_actor_user_id
        AND presence.meeting_key = p_meeting_key
        AND presence.device_fingerprint <> p_device_fingerprint
        AND presence.is_primary;
    END IF;

    INSERT INTO public.meeting_device_presence AS presence (
      user_id,
      meeting_key,
      device_fingerprint,
      is_primary,
      seen_at
    )
    VALUES (
      p_actor_user_id,
      p_meeting_key,
      p_device_fingerprint,
      p_intent = 'claim',
      now()
    )
    ON CONFLICT ON CONSTRAINT meeting_device_presence_pkey DO UPDATE
    SET
      seen_at = now(),
      is_primary = presence.is_primary OR EXCLUDED.is_primary;
  END IF;

  RETURN QUERY
  SELECT
    presence.device_fingerprint,
    device.device_name,
    presence.is_primary
  FROM public.meeting_device_presence AS presence
  LEFT JOIN public.sync_devices AS device
    ON device.user_id = presence.user_id
    AND device.device_fingerprint = presence.device_fingerprint
  WHERE presence.user_id = p_actor_user_id
    AND presence.meeting_key = p_meeting_key
  ORDER BY presence.device_fingerprint;
END;
$$;

REVOKE ALL ON FUNCTION public.heartbeat_meeting_device(uuid, text, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.heartbeat_meeting_device(uuid, text, text, text)
  TO service_role;

COMMIT;
