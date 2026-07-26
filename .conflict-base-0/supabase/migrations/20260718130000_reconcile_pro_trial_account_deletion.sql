CREATE OR REPLACE FUNCTION public.can_start_trial()
RETURNS boolean
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid := auth.uid();
  v_customer_id text;
  v_reserved_until timestamptz;
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = v_user_id
      AND COALESCE(auth_user.is_anonymous, false) = false
      AND NOT EXISTS (
        SELECT 1
        FROM private.account_deletion_jobs AS deletion
        WHERE deletion.owner_user_id = auth_user.id
      )
  ) THEN
    RETURN false;
  END IF;

  SELECT profile.stripe_customer_id, profile.trial_reserved_until
  INTO v_customer_id, v_reserved_until
  FROM public.profiles AS profile
  WHERE profile.id = v_user_id;

  IF NOT FOUND OR v_reserved_until > now() THEN
    RETURN false;
  END IF;

  IF v_customer_id IS NULL THEN
    RETURN true;
  END IF;

  RETURN NOT EXISTS (
    SELECT 1
    FROM stripe.subscriptions AS subscription
    WHERE subscription.customer = v_customer_id
  );
END;
$$;

COMMENT ON FUNCTION public.can_start_trial()
  IS 'Allows one new-user Pro trial per account; prior subscription history makes the account ineligible.';

REVOKE ALL ON FUNCTION public.can_start_trial() FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.can_start_trial() TO authenticated;

CREATE OR REPLACE FUNCTION public.reserve_pro_trial(p_channel text)
RETURNS TABLE (
  reservation_id uuid,
  reserved_until timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid := auth.uid();
  v_customer_id text;
  v_reservation_id uuid;
  v_reservation_channel text;
  v_reserved_until timestamptz;
BEGIN
  IF v_user_id IS NULL
    OR p_channel IS NULL
    OR p_channel NOT IN ('native', 'web')
  THEN
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_user_id::text, 170001)
  );

  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = v_user_id
      AND COALESCE(auth_user.is_anonymous, false) = false
      AND NOT EXISTS (
        SELECT 1
        FROM private.account_deletion_jobs AS deletion
        WHERE deletion.owner_user_id = auth_user.id
      )
  ) THEN
    RETURN;
  END IF;

  SELECT
    profile.stripe_customer_id,
    profile.trial_reservation_id,
    profile.trial_reservation_channel,
    profile.trial_reserved_until
  INTO
    v_customer_id,
    v_reservation_id,
    v_reservation_channel,
    v_reserved_until
  FROM public.profiles AS profile
  WHERE profile.id = v_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF v_customer_id IS NOT NULL AND EXISTS (
    SELECT 1
    FROM stripe.subscriptions AS subscription
    WHERE subscription.customer = v_customer_id
  ) THEN
    RETURN;
  END IF;

  IF v_reserved_until > now() THEN
    IF v_reservation_channel = p_channel THEN
      RETURN QUERY SELECT v_reservation_id, v_reserved_until;
    END IF;
    RETURN;
  END IF;

  v_reservation_id := gen_random_uuid();
  v_reserved_until := now() + interval '25 hours';

  UPDATE public.profiles
  SET
    trial_reservation_id = v_reservation_id,
    trial_reservation_channel = p_channel,
    trial_reserved_until = v_reserved_until
  WHERE id = v_user_id;

  RETURN QUERY SELECT v_reservation_id, v_reserved_until;
END;
$$;

REVOKE ALL ON FUNCTION public.reserve_pro_trial(text) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.reserve_pro_trial(text) TO authenticated;
