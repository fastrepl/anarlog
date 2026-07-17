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
BEGIN
  IF v_user_id IS NULL THEN
    RETURN false;
  END IF;

  SELECT stripe_customer_id INTO v_customer_id
  FROM public.profiles
  WHERE id = v_user_id;

  IF v_customer_id IS NULL THEN
    RETURN true;
  END IF;

  RETURN NOT EXISTS (
    SELECT 1 FROM stripe.subscriptions
    WHERE customer = v_customer_id
      AND (
        status IN ('active', 'trialing')
        OR (
          trial_start IS NOT NULL
          AND (trial_start #>> '{}')::bigint > extract(epoch from now() - interval '3 months')
        )
      )
  );
END;
$$;

GRANT EXECUTE ON FUNCTION public.can_start_trial TO authenticated;
REVOKE EXECUTE ON FUNCTION public.can_start_trial FROM anon, public;
