GRANT SELECT (subscription, current_period_end)
ON TABLE stripe.subscription_items
TO supabase_auth_admin;

-- search_path is pinned here rather than left to the ALTER in
-- 20260714134923: CREATE OR REPLACE resets attributes it does not restate.
CREATE OR REPLACE FUNCTION public.custom_access_token_hook(event jsonb)
RETURNS jsonb
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
DECLARE
  claims jsonb;
  entitlements jsonb := '[]'::jsonb;
  v_user_id uuid := (event->>'user_id')::uuid;
  v_customer_id text;
  v_subscription_status text;
  v_trial_end bigint;
  v_has_payment_method boolean;
  v_cancel_at_period_end boolean;
  v_current_period_end bigint;
BEGIN
  SELECT p.stripe_customer_id INTO v_customer_id
  FROM public.profiles p
  WHERE p.id = v_user_id;

  SELECT
    COALESCE(
      jsonb_agg(DISTINCT granted.lookup_key ORDER BY granted.lookup_key)
        FILTER (WHERE granted.lookup_key IS NOT NULL),
      '[]'::jsonb
    )
  INTO entitlements
  FROM (
    SELECT ae.lookup_key
    FROM public.profiles p
    JOIN stripe.active_entitlements ae
      ON ae.customer = p.stripe_customer_id
    WHERE p.id = v_user_id

    UNION

    SELECT ae.lookup_key
    FROM public.workspace_memberships m
    JOIN public.workspaces w
      ON w.id = m.workspace_id
    JOIN stripe.active_entitlements ae
      ON ae.customer = w.stripe_customer_id
    WHERE m.user_id = v_user_id
      AND m.deleted_at IS NULL
      AND w.deleted_at IS NULL
      AND w.stripe_customer_id IS NOT NULL
  ) AS granted;

  IF v_customer_id IS NOT NULL THEN
    SELECT
      s.status::text,
      (s.trial_end #>> '{}')::bigint,
      s.default_payment_method IS NOT NULL
        OR c.invoice_settings->>'default_payment_method' IS NOT NULL
        OR c.default_source IS NOT NULL,
      COALESCE(s.cancel_at_period_end, false),
      -- cancel_at is the scheduled end; newer Stripe APIs keep the period on items.
      COALESCE(
        s.cancel_at,
        s.current_period_end,
        (
          SELECT MAX(si.current_period_end)
          FROM stripe.subscription_items si
          WHERE si.subscription = s.id
        )
      )
    INTO
      v_subscription_status,
      v_trial_end,
      v_has_payment_method,
      v_cancel_at_period_end,
      v_current_period_end
    FROM stripe.subscriptions s
    JOIN stripe.customers c ON c.id = s.customer
    WHERE s.customer = v_customer_id
      AND s.status IN ('trialing', 'active')
    ORDER BY
      CASE s.status WHEN 'active' THEN 1 WHEN 'trialing' THEN 2 END,
      s.created DESC
    LIMIT 1;
  END IF;

  -- A member with no subscription of their own still reads as subscribed while
  -- a workspace covers them; personal billing state wins when both exist.
  IF v_subscription_status IS NULL THEN
    SELECT s.status::text
    INTO v_subscription_status
    FROM public.workspace_memberships m
    JOIN public.workspaces w
      ON w.id = m.workspace_id
    JOIN stripe.subscriptions s
      ON s.customer = w.stripe_customer_id
    WHERE m.user_id = v_user_id
      AND m.deleted_at IS NULL
      AND w.deleted_at IS NULL
      AND w.stripe_customer_id IS NOT NULL
      AND s.status IN ('trialing', 'active')
    ORDER BY
      CASE s.status WHEN 'active' THEN 1 WHEN 'trialing' THEN 2 END,
      s.created DESC
    LIMIT 1;
  END IF;

  -- A paused personal subscription is resumable, but it must not override an
  -- effective personal or workspace subscription selected above.
  IF v_subscription_status IS NULL AND v_customer_id IS NOT NULL THEN
    SELECT
      s.status::text,
      (s.trial_end #>> '{}')::bigint,
      s.default_payment_method IS NOT NULL
        OR c.invoice_settings->>'default_payment_method' IS NOT NULL
        OR c.default_source IS NOT NULL,
      COALESCE(s.cancel_at_period_end, false),
      COALESCE(
        s.cancel_at,
        s.current_period_end,
        (
          SELECT MAX(si.current_period_end)
          FROM stripe.subscription_items si
          WHERE si.subscription = s.id
        )
      )
    INTO
      v_subscription_status,
      v_trial_end,
      v_has_payment_method,
      v_cancel_at_period_end,
      v_current_period_end
    FROM stripe.subscriptions s
    JOIN stripe.customers c ON c.id = s.customer
    WHERE s.customer = v_customer_id
      AND s.status = 'paused'
    ORDER BY s.created DESC
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

  IF v_has_payment_method IS NOT NULL THEN
    claims := jsonb_set(claims, '{has_payment_method}', to_jsonb(v_has_payment_method));
  END IF;

  IF v_cancel_at_period_end IS NOT NULL THEN
    claims := jsonb_set(
      claims,
      '{cancel_at_period_end}',
      to_jsonb(v_cancel_at_period_end)
    );
  END IF;

  IF v_current_period_end IS NOT NULL THEN
    claims := jsonb_set(claims, '{current_period_end}', to_jsonb(v_current_period_end));
  END IF;

  event := jsonb_set(event, '{claims}', claims);

  RETURN event;
END;
$$;
