-- Referral eligibility previously required a personal active Stripe
-- subscription, so members whose Pro came from a paid workspace seat were
-- excluded even though the access token grants them hyprnote_pro. Keep the
-- original personal active-subscription check and additionally allow
-- workspace-based Pro entitlements.
CREATE OR REPLACE FUNCTION private.is_paid_referrer(p_user_id uuid)
RETURNS boolean
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.profiles AS profile
    JOIN stripe.subscriptions AS subscription
      ON subscription.customer = profile.stripe_customer_id
    WHERE profile.id = p_user_id
      AND subscription.status = 'active'
  )
  OR EXISTS (
    SELECT 1
    FROM public.workspace_memberships AS membership
    JOIN public.workspaces AS workspace
      ON workspace.id = membership.workspace_id
    JOIN stripe.subscriptions AS subscription
      ON subscription.customer = workspace.stripe_customer_id
    WHERE membership.user_id = p_user_id
      AND membership.deleted_at IS NULL
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND workspace.stripe_customer_id IS NOT NULL
      AND subscription.status = 'active'
  );
$$;

COMMENT ON FUNCTION private.is_paid_referrer(uuid)
  IS 'Returns true for a user with an active paid personal or shared-workspace subscription, excluding trials.';

CREATE OR REPLACE FUNCTION public.get_or_create_referral_invites()
RETURNS TABLE (
  slot smallint,
  code text,
  status text,
  reward_amount_cents integer,
  reward_currency text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid := auth.uid();
BEGIN
  IF v_user_id IS NULL
    OR NOT private.is_paid_referrer(v_user_id)
    OR EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = v_user_id
    )
  THEN
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_user_id::text, 180001)
  );

  INSERT INTO private.referral_invites (
    referrer_user_id,
    slot,
    code
  )
  SELECT
    v_user_id,
    generated_slot,
    encode(extensions.gen_random_bytes(12), 'hex')
  FROM generate_series(1, 3) AS generated_slot
  ON CONFLICT ON CONSTRAINT referral_invites_referrer_slot_key DO NOTHING;

  RETURN QUERY
  SELECT
    referral.slot,
    referral.code,
    CASE
      WHEN referral.rewarded_at IS NOT NULL THEN 'reward_earned'
      WHEN referral.referred_user_id IS NOT NULL THEN 'trial_started'
      ELSE 'available'
    END,
    1500,
    'usd'
  FROM private.referral_invites AS referral
  WHERE referral.referrer_user_id = v_user_id
  ORDER BY referral.slot;
END;
$$;

COMMENT ON FUNCTION public.get_or_create_referral_invites()
  IS 'Returns three referral slots for active paid Pro subscribers, including workspace seats, creating missing slots atomically.';

CREATE OR REPLACE FUNCTION public.claim_referral(p_code text)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid := auth.uid();
  v_referral private.referral_invites%ROWTYPE;
  v_customer_id text;
BEGIN
  IF v_user_id IS NULL OR p_code !~ '^[a-f0-9]{24}$' THEN
    RETURN false;
  END IF;

  SELECT referral.*
  INTO v_referral
  FROM private.referral_invites AS referral
  WHERE referral.code = p_code
  FOR UPDATE;

  IF NOT FOUND OR v_referral.referrer_user_id = v_user_id THEN
    RETURN false;
  END IF;

  IF v_referral.referred_user_id = v_user_id THEN
    RETURN true;
  END IF;

  IF v_referral.referred_user_id IS NOT NULL
    OR EXISTS (
      SELECT 1
      FROM private.referral_invites AS existing
      WHERE existing.referred_user_id = v_user_id
    )
    OR NOT EXISTS (
      SELECT 1
      FROM auth.users AS auth_user
      WHERE auth_user.id = v_user_id
        AND COALESCE(auth_user.is_anonymous, false) = false
        AND auth_user.created_at >= now() - interval '7 days'
        AND NOT EXISTS (
          SELECT 1
          FROM private.account_deletion_jobs AS deletion
          WHERE deletion.owner_user_id = v_user_id
        )
    )
    OR NOT private.is_paid_referrer(v_referral.referrer_user_id)
  THEN
    RETURN false;
  END IF;

  SELECT profile.stripe_customer_id
  INTO v_customer_id
  FROM public.profiles AS profile
  WHERE profile.id = v_user_id;

  IF NOT FOUND OR (
    v_customer_id IS NOT NULL AND EXISTS (
      SELECT 1
      FROM stripe.subscriptions AS subscription
      WHERE subscription.customer = v_customer_id
    )
  ) THEN
    RETURN false;
  END IF;

  UPDATE private.referral_invites
  SET
    referred_user_id = v_user_id,
    claimed_at = clock_timestamp()
  WHERE id = v_referral.id;

  RETURN true;
END;
$$;

COMMENT ON FUNCTION public.claim_referral(text)
  IS 'Claims one available referral slot for a new, trial-eligible account. The referrer must have an active paid personal or shared-workspace subscription.';

CREATE OR REPLACE FUNCTION public.prepare_referral_reward(
  p_referred_user_id uuid,
  p_invoice_id text
)
RETURNS TABLE (
  referral_id uuid,
  referrer_user_id uuid,
  referrer_customer_id text,
  reward_amount_cents integer,
  reward_currency text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_referral private.referral_invites%ROWTYPE;
  v_referrer_customer_id text;
BEGIN
  IF p_referred_user_id IS NULL
    OR p_invoice_id IS NULL
    OR p_invoice_id !~ '^in_[A-Za-z0-9]+$'
  THEN
    RETURN;
  END IF;

  SELECT referral.*
  INTO v_referral
  FROM private.referral_invites AS referral
  WHERE referral.referred_user_id = p_referred_user_id
  FOR UPDATE;

  IF NOT FOUND OR v_referral.rewarded_at IS NOT NULL THEN
    RETURN;
  END IF;

  IF v_referral.qualifying_invoice_id IS NULL THEN
    UPDATE private.referral_invites
    SET
      qualifying_invoice_id = p_invoice_id,
      qualified_at = clock_timestamp()
    WHERE id = v_referral.id;
    v_referral.qualifying_invoice_id := p_invoice_id;
  ELSIF v_referral.qualifying_invoice_id <> p_invoice_id THEN
    RETURN;
  END IF;

  SELECT profile.stripe_customer_id
  INTO v_referrer_customer_id
  FROM public.profiles AS profile
  WHERE profile.id = v_referral.referrer_user_id;

  IF v_referrer_customer_id IS NULL THEN
    SELECT workspace.stripe_customer_id
    INTO v_referrer_customer_id
    FROM public.workspace_memberships AS membership
    JOIN public.workspaces AS workspace
      ON workspace.id = membership.workspace_id
    JOIN stripe.subscriptions AS subscription
      ON subscription.customer = workspace.stripe_customer_id
    WHERE membership.user_id = v_referral.referrer_user_id
      AND membership.deleted_at IS NULL
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND workspace.stripe_customer_id IS NOT NULL
      AND subscription.status = 'active'
    ORDER BY workspace.created_at
    LIMIT 1;
  END IF;

  IF v_referrer_customer_id IS NULL THEN
    RETURN;
  END IF;

  RETURN QUERY SELECT
    v_referral.id,
    v_referral.referrer_user_id,
    v_referrer_customer_id,
    1500,
    'usd';
END;
$$;

REVOKE ALL ON FUNCTION public.prepare_referral_reward(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.prepare_referral_reward(uuid, text)
  TO service_role;
