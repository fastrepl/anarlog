-- Account identity analytics cannot reliably honor browser privacy signals at
-- auth-user creation time. Stop producing and delivering these events, and
-- remove queued identity data.

DROP TRIGGER IF EXISTS on_auth_user_account_analytics_created ON auth.users;
DROP TRIGGER IF EXISTS on_auth_user_account_analytics_confirmed ON auth.users;

CREATE OR REPLACE FUNCTION private.enqueue_account_analytics_event(
  p_event_name text,
  p_user_id uuid,
  p_occurred_at timestamptz,
  p_email text,
  p_auth_provider text,
  p_historical boolean
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RETURN;
END;
$$;

DELETE FROM private.account_analytics_outbox;
