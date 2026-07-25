CREATE OR REPLACE FUNCTION public.claim_account_analytics_events(
  p_lease_id uuid,
  p_limit integer DEFAULT 500,
  p_lease_seconds integer DEFAULT 300
)
RETURNS TABLE (
  id uuid,
  event_name text,
  user_id uuid,
  occurred_at timestamptz,
  properties jsonb,
  historical boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  lease_time timestamptz := clock_timestamp();
BEGIN
  IF p_lease_id IS NULL
    OR p_limit IS NULL
    OR p_limit < 1
    OR p_limit > 500
    OR p_lease_seconds IS NULL
    OR p_lease_seconds < 30
    OR p_lease_seconds > 900
  THEN
    RAISE EXCEPTION 'invalid account analytics lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT outbox.id
    FROM private.account_analytics_outbox AS outbox
    WHERE outbox.delivered_at IS NULL
      AND outbox.next_attempt_at <= lease_time
      AND outbox.attempt_count < 20
      AND (
        outbox.lease_expires_at IS NULL
        OR outbox.lease_expires_at <= lease_time
      )
    ORDER BY outbox.occurred_at, outbox.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE private.account_analytics_outbox AS outbox
    SET
      attempt_count = outbox.attempt_count + 1,
      lease_id = p_lease_id,
      lease_expires_at = lease_time
        + make_interval(secs => p_lease_seconds),
      updated_at = lease_time
    FROM candidates
    WHERE outbox.id = candidates.id
    RETURNING outbox.*
  )
  SELECT
    leased.id,
    leased.event_name,
    leased.user_id,
    leased.occurred_at,
    leased.properties,
    leased.historical
  FROM leased
  ORDER BY leased.occurred_at, leased.id;
END;
$$;
