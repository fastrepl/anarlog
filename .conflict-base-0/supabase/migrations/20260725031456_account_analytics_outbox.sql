CREATE TABLE private.account_analytics_outbox (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  event_name text NOT NULL,
  user_id uuid NOT NULL,
  occurred_at timestamptz NOT NULL,
  email text,
  properties jsonb NOT NULL DEFAULT '{}'::jsonb,
  historical boolean NOT NULL DEFAULT false,
  attempt_count integer NOT NULL DEFAULT 0,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  lease_id uuid,
  lease_expires_at timestamptz,
  delivered_at timestamptz,
  last_error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT account_analytics_outbox_event_name_check CHECK (
    event_name IN ('account_created', 'account_confirmed')
  ),
  CONSTRAINT account_analytics_outbox_event_user_key UNIQUE (
    event_name,
    user_id
  )
);

CREATE INDEX account_analytics_outbox_claim_idx
  ON private.account_analytics_outbox (
    next_attempt_at,
    occurred_at
  )
  WHERE delivered_at IS NULL;

REVOKE ALL ON TABLE private.account_analytics_outbox
  FROM PUBLIC, anon, authenticated;

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
  ON CONFLICT (event_name, user_id) DO UPDATE SET
    email = COALESCE(EXCLUDED.email, private.account_analytics_outbox.email),
    properties = private.account_analytics_outbox.properties
      || EXCLUDED.properties,
    updated_at = clock_timestamp();
END;
$$;

CREATE OR REPLACE FUNCTION private.handle_account_analytics_user()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  confirmation_time timestamptz;
  provider text;
BEGIN
  IF COALESCE(NEW.is_anonymous, false) THEN
    RETURN NEW;
  END IF;

  provider := COALESCE(NEW.raw_app_meta_data ->> 'provider', 'unknown');
  confirmation_time := COALESCE(
    NEW.confirmed_at,
    NEW.email_confirmed_at,
    NEW.phone_confirmed_at
  );

  IF TG_OP = 'INSERT' THEN
    PERFORM private.enqueue_account_analytics_event(
      'account_created',
      NEW.id,
      NEW.created_at,
      NEW.email,
      provider,
      false
    );
  END IF;

  IF confirmation_time IS NOT NULL THEN
    PERFORM private.enqueue_account_analytics_event(
      'account_confirmed',
      NEW.id,
      confirmation_time,
      NEW.email,
      provider,
      false
    );
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.enqueue_account_analytics_event(
  text,
  uuid,
  timestamptz,
  text,
  text,
  boolean
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.handle_account_analytics_user()
  FROM PUBLIC, anon, authenticated;
GRANT USAGE ON SCHEMA private TO supabase_auth_admin;
GRANT EXECUTE ON FUNCTION private.handle_account_analytics_user()
  TO supabase_auth_admin;

DROP TRIGGER IF EXISTS on_auth_user_account_analytics_created ON auth.users;
CREATE TRIGGER on_auth_user_account_analytics_created
  AFTER INSERT ON auth.users
  FOR EACH ROW
  EXECUTE FUNCTION private.handle_account_analytics_user();

DROP TRIGGER IF EXISTS on_auth_user_account_analytics_confirmed ON auth.users;
CREATE TRIGGER on_auth_user_account_analytics_confirmed
  AFTER UPDATE OF confirmed_at, email_confirmed_at, phone_confirmed_at
  ON auth.users
  FOR EACH ROW
  WHEN (
    COALESCE(
      OLD.confirmed_at,
      OLD.email_confirmed_at,
      OLD.phone_confirmed_at
    ) IS NULL
    AND COALESCE(
      NEW.confirmed_at,
      NEW.email_confirmed_at,
      NEW.phone_confirmed_at
    ) IS NOT NULL
  )
  EXECUTE FUNCTION private.handle_account_analytics_user();

CREATE OR REPLACE FUNCTION public.reconcile_account_analytics_events()
RETURNS TABLE (
  source_accounts bigint,
  source_confirmed_accounts bigint,
  queued_account_created bigint,
  queued_account_confirmed bigint,
  pending_events bigint,
  delivered_events bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.enqueue_account_analytics_event(
    'account_created',
    account.id,
    account.created_at,
    account.email,
    COALESCE(account.raw_app_meta_data ->> 'provider', 'unknown'),
    account.created_at < clock_timestamp() - interval '10 minutes'
  )
  FROM auth.users AS account
  WHERE COALESCE(account.is_anonymous, false) = false;

  PERFORM private.enqueue_account_analytics_event(
    'account_confirmed',
    account.id,
    confirmation.occurred_at,
    account.email,
    COALESCE(account.raw_app_meta_data ->> 'provider', 'unknown'),
    confirmation.occurred_at < clock_timestamp() - interval '10 minutes'
  )
  FROM auth.users AS account
  CROSS JOIN LATERAL (
    SELECT COALESCE(
      account.confirmed_at,
      account.email_confirmed_at,
      account.phone_confirmed_at
    ) AS occurred_at
  ) AS confirmation
  WHERE COALESCE(account.is_anonymous, false) = false
    AND confirmation.occurred_at IS NOT NULL;

  RETURN QUERY
  SELECT
    (
      SELECT count(*)
      FROM auth.users AS account
      WHERE COALESCE(account.is_anonymous, false) = false
    ),
    (
      SELECT count(*)
      FROM auth.users AS account
      WHERE COALESCE(account.is_anonymous, false) = false
        AND COALESCE(
          account.confirmed_at,
          account.email_confirmed_at,
          account.phone_confirmed_at
        ) IS NOT NULL
    ),
    count(*) FILTER (WHERE outbox.event_name = 'account_created'),
    count(*) FILTER (WHERE outbox.event_name = 'account_confirmed'),
    count(*) FILTER (WHERE outbox.delivered_at IS NULL),
    count(*) FILTER (WHERE outbox.delivered_at IS NOT NULL)
  FROM private.account_analytics_outbox AS outbox;
END;
$$;

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
  current_time timestamptz := clock_timestamp();
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
      AND outbox.next_attempt_at <= current_time
      AND outbox.attempt_count < 20
      AND (
        outbox.lease_expires_at IS NULL
        OR outbox.lease_expires_at <= current_time
      )
    ORDER BY outbox.occurred_at, outbox.id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE private.account_analytics_outbox AS outbox
    SET
      attempt_count = outbox.attempt_count + 1,
      lease_id = p_lease_id,
      lease_expires_at = current_time
        + make_interval(secs => p_lease_seconds),
      updated_at = current_time
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

CREATE OR REPLACE FUNCTION public.complete_account_analytics_events(
  p_lease_id uuid,
  p_event_ids uuid[]
)
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  updated_count integer;
BEGIN
  IF p_lease_id IS NULL
    OR p_event_ids IS NULL
    OR cardinality(p_event_ids) < 1
    OR cardinality(p_event_ids) > 500
  THEN
    RAISE EXCEPTION 'invalid account analytics completion'
      USING ERRCODE = '22023';
  END IF;

  UPDATE private.account_analytics_outbox AS outbox
  SET
    delivered_at = clock_timestamp(),
    lease_id = NULL,
    lease_expires_at = NULL,
    last_error = NULL,
    updated_at = clock_timestamp()
  WHERE outbox.id = ANY (p_event_ids)
    AND outbox.lease_id = p_lease_id
    AND outbox.delivered_at IS NULL;

  GET DIAGNOSTICS updated_count = ROW_COUNT;
  RETURN updated_count;
END;
$$;

CREATE OR REPLACE FUNCTION public.fail_account_analytics_events(
  p_lease_id uuid,
  p_event_ids uuid[],
  p_error text
)
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  updated_count integer;
BEGIN
  IF p_lease_id IS NULL
    OR p_event_ids IS NULL
    OR cardinality(p_event_ids) < 1
    OR cardinality(p_event_ids) > 500
  THEN
    RAISE EXCEPTION 'invalid account analytics failure'
      USING ERRCODE = '22023';
  END IF;

  UPDATE private.account_analytics_outbox AS outbox
  SET
    next_attempt_at = clock_timestamp()
      + make_interval(
        secs => LEAST(3600, power(2, LEAST(outbox.attempt_count, 10))::integer)
      ),
    lease_id = NULL,
    lease_expires_at = NULL,
    last_error = left(COALESCE(p_error, 'unknown error'), 2000),
    updated_at = clock_timestamp()
  WHERE outbox.id = ANY (p_event_ids)
    AND outbox.lease_id = p_lease_id
    AND outbox.delivered_at IS NULL;

  GET DIAGNOSTICS updated_count = ROW_COUNT;
  RETURN updated_count;
END;
$$;

REVOKE ALL ON FUNCTION public.reconcile_account_analytics_events()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.claim_account_analytics_events(
  uuid,
  integer,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.complete_account_analytics_events(uuid, uuid[])
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.fail_account_analytics_events(
  uuid,
  uuid[],
  text
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.reconcile_account_analytics_events()
  TO service_role;
GRANT EXECUTE ON FUNCTION public.claim_account_analytics_events(
  uuid,
  integer,
  integer
) TO service_role;
GRANT EXECUTE ON FUNCTION public.complete_account_analytics_events(uuid, uuid[])
  TO service_role;
GRANT EXECUTE ON FUNCTION public.fail_account_analytics_events(
  uuid,
  uuid[],
  text
) TO service_role;

DO $$
BEGIN
  PERFORM public.reconcile_account_analytics_events();
END;
$$;
