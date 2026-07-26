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
DECLARE
  person_properties jsonb;
BEGIN
  IF p_event_name NOT IN ('account_created', 'account_confirmed')
    OR p_user_id IS NULL
    OR p_occurred_at IS NULL
  THEN
    RAISE EXCEPTION 'invalid account analytics event'
      USING ERRCODE = '22023';
  END IF;

  person_properties := jsonb_strip_nulls(
    jsonb_build_object(
      'email',
      p_email,
      CASE
        WHEN p_event_name = 'account_created' THEN 'account_created_at'
        ELSE 'account_confirmed_at'
      END,
      p_occurred_at
    )
  );

  INSERT INTO private.account_analytics_outbox (
    event_name,
    user_id,
    occurred_at,
    email,
    properties,
    historical
  )
  VALUES (
    p_event_name,
    p_user_id,
    p_occurred_at,
    p_email,
    jsonb_strip_nulls(
      jsonb_build_object(
        'source',
        'supabase_auth',
        'auth_provider',
        p_auth_provider,
        '$set',
        person_properties
      )
    ),
    p_historical
  )
  ON CONFLICT (event_name, user_id) DO NOTHING;
END;
$$;
