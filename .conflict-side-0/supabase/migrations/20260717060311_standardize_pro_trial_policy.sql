DROP POLICY IF EXISTS profiles_insert_owner ON public.profiles;
DROP POLICY IF EXISTS profiles_update_owner ON public.profiles;
DROP POLICY IF EXISTS profiles_delete_owner ON public.profiles;

REVOKE INSERT, UPDATE, DELETE ON TABLE public.profiles FROM authenticated;
GRANT SELECT ON TABLE public.profiles TO authenticated;

CREATE UNIQUE INDEX IF NOT EXISTS profiles_stripe_customer_id_unique
  ON public.profiles (stripe_customer_id)
  WHERE stripe_customer_id IS NOT NULL;

ALTER TABLE public.profiles
  ADD COLUMN trial_reservation_id uuid,
  ADD COLUMN trial_reservation_channel text,
  ADD COLUMN trial_reserved_until timestamptz,
  ADD CONSTRAINT profiles_trial_reservation_channel_check
    CHECK (
      trial_reservation_channel IS NULL
      OR trial_reservation_channel IN ('native', 'web')
    ),
  ADD CONSTRAINT profiles_trial_reservation_shape_check
    CHECK (
      (
        trial_reservation_id IS NULL
        AND trial_reservation_channel IS NULL
        AND trial_reserved_until IS NULL
      )
      OR (
        trial_reservation_id IS NOT NULL
        AND trial_reservation_channel IS NOT NULL
        AND trial_reserved_until IS NOT NULL
      )
    );

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
  IF v_user_id IS NULL THEN
    RETURN false;
  END IF;

  SELECT stripe_customer_id, trial_reserved_until
  INTO v_customer_id, v_reserved_until
  FROM public.profiles
  WHERE id = v_user_id;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF v_reserved_until > now() THEN
    RETURN false;
  END IF;

  IF v_customer_id IS NULL THEN
    RETURN true;
  END IF;

  RETURN NOT EXISTS (
    SELECT 1
    FROM stripe.subscriptions
    WHERE customer = v_customer_id
  );
END;
$$;

COMMENT ON FUNCTION public.can_start_trial()
  IS 'Allows one new-user Pro trial per account; prior subscription history makes the account ineligible.';

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

CREATE OR REPLACE FUNCTION public.release_pro_trial_reservation(
  p_user_id uuid,
  p_reservation_id uuid
)
RETURNS void
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  UPDATE public.profiles
  SET
    trial_reservation_id = NULL,
    trial_reservation_channel = NULL,
    trial_reserved_until = NULL
  WHERE id = p_user_id
    AND trial_reservation_id = p_reservation_id;
$$;

REVOKE ALL ON FUNCTION public.reserve_pro_trial(text)
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.reserve_pro_trial(text)
  TO authenticated;

REVOKE ALL ON FUNCTION public.release_pro_trial_reservation(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.release_pro_trial_reservation(uuid, uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION private.require_hyprnote_pro_entitlement()
RETURNS void
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE
  v_claims jsonb := COALESCE(auth.jwt(), '{}'::jsonb);
  v_trial_end text;
BEGIN
  v_trial_end := v_claims ->> 'trial_end';

  IF v_claims ->> 'subscription_status' = 'trialing' THEN
    IF COALESCE(v_trial_end, '') ~ '^[0-9]+$'
      AND v_trial_end::bigint > EXTRACT(epoch FROM now())::bigint
    THEN
      RETURN;
    END IF;

    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;

  IF v_claims -> 'entitlements' @> '["hyprnote_pro"]'::jsonb THEN
    RETURN;
  END IF;

  RAISE EXCEPTION 'hyprnote pro entitlement required'
    USING ERRCODE = '42501';
END;
$$;

COMMENT ON FUNCTION private.require_hyprnote_pro_entitlement()
  IS 'Requires a paid Pro entitlement or an unexpired server-issued Pro trial claim.';
