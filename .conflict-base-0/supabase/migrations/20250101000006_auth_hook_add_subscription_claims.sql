CREATE OR REPLACE FUNCTION public.custom_access_token_hook(event jsonb)
RETURNS jsonb
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
  claims jsonb;
  entitlements jsonb := '[]'::jsonb;
  v_user_id uuid := (event->>'user_id')::uuid;
  v_customer_id text;
  v_subscription_status text;
  v_trial_end bigint;
BEGIN
  SELECT p.stripe_customer_id INTO v_customer_id
  FROM public.profiles p
  WHERE p.id = v_user_id;

  SELECT
    COALESCE(
      jsonb_agg(ae.lookup_key ORDER BY ae.lookup_key)
        FILTER (WHERE ae.lookup_key IS NOT NULL),
      '[]'::jsonb
    )
  INTO entitlements
  FROM public.profiles p
  JOIN stripe.active_entitlements ae
    ON ae.customer = p.stripe_customer_id
  WHERE p.id = v_user_id;

  IF v_customer_id IS NOT NULL THEN
    SELECT
      s.status::text,
      (s.trial_end #>> '{}')::bigint
    INTO v_subscription_status, v_trial_end
    FROM stripe.subscriptions s
    WHERE s.customer = v_customer_id
      AND s.status IN ('trialing', 'active')
    ORDER BY
      CASE s.status WHEN 'active' THEN 1 WHEN 'trialing' THEN 2 END,
      s.created DESC
    LIMIT 1;
  END IF;

  claims := event->'claims';
  claims := jsonb_set(claims, '{entitlements}', entitlements);

  IF v_subscription_status IS NOT NULL THEN
    claims := jsonb_set(claims, '{subscription_status}', to_jsonb(v_subscription_status));
  END IF;

  IF v_trial_end IS NOT NULL THEN
    claims := jsonb_set(claims, '{trial_end}', to_jsonb(v_trial_end));
  END IF;

  event := jsonb_set(event, '{claims}', claims);

  RETURN event;
END;
$$;

GRANT SELECT ON TABLE stripe.subscriptions TO supabase_auth_admin;
