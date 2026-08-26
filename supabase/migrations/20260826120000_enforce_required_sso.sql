-- Enforce workspace Require SSO for matching email domains, and refuse to
-- enable the policy until a domain is claimed. Older clients ignore the new
-- lookup RPC.

BEGIN;

SET LOCAL lock_timeout = '30s';

CREATE OR REPLACE FUNCTION private.email_domain_requires_sso(p_email text)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.workspace_verified_domains AS claimed
    JOIN public.workspaces AS workspace
      ON workspace.id = claimed.workspace_id
    JOIN public.workspace_policies AS policy
      ON policy.workspace_id = claimed.workspace_id
    WHERE claimed.domain = lower(split_part(btrim(COALESCE(p_email, '')), '@', 2))
      AND position('@' in COALESCE(p_email, '')) > 0
      AND workspace.deleted_at IS NULL
      AND workspace.kind = 'shared'
      AND policy.require_sso
  );
$$;

REVOKE ALL ON FUNCTION private.email_domain_requires_sso(text)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.session_is_sso(p_claims jsonb)
RETURNS boolean
LANGUAGE sql
IMMUTABLE
SET search_path = ''
AS $$
  SELECT
    COALESCE(p_claims -> 'app_metadata' ->> 'provider', '') LIKE 'sso:%'
    OR EXISTS (
      SELECT 1
      FROM jsonb_array_elements(
        CASE
          WHEN jsonb_typeof(p_claims -> 'amr') = 'array' THEN p_claims -> 'amr'
          ELSE '[]'::jsonb
        END
      ) AS method(value)
      WHERE method.value ->> 'method' IN ('sso', 'sso/saml', 'saml')
    );
$$;

REVOKE ALL ON FUNCTION private.session_is_sso(jsonb)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.session_blocked_for_required_sso(
  p_user_id uuid,
  p_claims jsonb
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_email text;
BEGIN
  v_email := lower(btrim(COALESCE(p_claims ->> 'email', '')));

  IF v_email = '' OR position('@' in v_email) = 0 THEN
    SELECT lower(btrim(COALESCE(users.email, '')))
    INTO v_email
    FROM auth.users AS users
    WHERE users.id = p_user_id;
  END IF;

  IF v_email IS NULL OR position('@' in v_email) = 0 THEN
    RETURN false;
  END IF;

  IF NOT private.email_domain_requires_sso(v_email) THEN
    RETURN false;
  END IF;

  RETURN NOT private.session_is_sso(COALESCE(p_claims, '{}'::jsonb));
END;
$$;

REVOKE ALL ON FUNCTION private.session_blocked_for_required_sso(uuid, jsonb)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.session_blocked_for_required_sso(uuid, jsonb)
  TO supabase_auth_admin;

CREATE OR REPLACE FUNCTION public.email_requires_sso(p_email text)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT private.email_domain_requires_sso(p_email);
$$;

REVOKE ALL ON FUNCTION public.email_requires_sso(text)
  FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.email_requires_sso(text)
  TO anon, authenticated;

CREATE OR REPLACE FUNCTION public.set_workspace_policy(
  p_workspace_id uuid,
  p_allowed_share_scopes text[],
  p_default_share_scope text,
  p_retention_days integer,
  p_model_training_opt_out boolean,
  p_consent_notification_enabled boolean,
  p_require_sso boolean
)
RETURNS TABLE (
  workspace_id uuid,
  allowed_share_scopes text[],
  default_share_scope text,
  retention_days integer,
  model_training_opt_out boolean,
  consent_notification_enabled boolean,
  require_sso boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
#variable_conflict use_column
BEGIN
  PERFORM private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_hyprnote_pro_entitlement();

  IF COALESCE(p_require_sso, false)
    AND NOT EXISTS (
      SELECT 1
      FROM public.workspace_verified_domains AS claimed
      WHERE claimed.workspace_id = p_workspace_id
    )
  THEN
    RAISE EXCEPTION 'claim an email domain before requiring SSO'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.workspace_policies (
    workspace_id,
    allowed_share_scopes,
    default_share_scope,
    retention_days,
    model_training_opt_out,
    consent_notification_enabled,
    require_sso,
    updated_at
  ) VALUES (
    p_workspace_id,
    p_allowed_share_scopes,
    p_default_share_scope,
    p_retention_days,
    COALESCE(p_model_training_opt_out, true),
    COALESCE(p_consent_notification_enabled, true),
    COALESCE(p_require_sso, false),
    now()
  )
  ON CONFLICT (workspace_id) DO UPDATE SET
    allowed_share_scopes = EXCLUDED.allowed_share_scopes,
    default_share_scope = EXCLUDED.default_share_scope,
    retention_days = EXCLUDED.retention_days,
    model_training_opt_out = EXCLUDED.model_training_opt_out,
    consent_notification_enabled = EXCLUDED.consent_notification_enabled,
    require_sso = EXCLUDED.require_sso,
    updated_at = now();

  RETURN QUERY
  SELECT
    policy.workspace_id,
    policy.allowed_share_scopes,
    policy.default_share_scope,
    policy.retention_days,
    policy.model_training_opt_out,
    policy.consent_notification_enabled,
    policy.require_sso
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = p_workspace_id;
END;
$$;

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
  IF private.session_blocked_for_required_sso(
    v_user_id,
    COALESCE(event->'claims', '{}'::jsonb)
  ) THEN
    RAISE EXCEPTION 'this organization requires SSO'
      USING ERRCODE = '42501';
  END IF;

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

COMMIT;
