CREATE TABLE private.referral_invites (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  referrer_user_id uuid NOT NULL REFERENCES auth.users (id) ON DELETE CASCADE,
  slot smallint NOT NULL CHECK (slot BETWEEN 1 AND 3),
  code text NOT NULL UNIQUE CHECK (code ~ '^[a-f0-9]{24}$'),
  referred_user_id uuid UNIQUE REFERENCES auth.users (id) ON DELETE SET NULL,
  claimed_at timestamptz,
  qualifying_invoice_id text UNIQUE,
  qualified_at timestamptz,
  reward_balance_transaction_id text UNIQUE,
  rewarded_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT referral_invites_referrer_slot_key
    UNIQUE (referrer_user_id, slot),
  CONSTRAINT referral_invites_claim_shape CHECK (
    (referred_user_id IS NULL AND claimed_at IS NULL)
    OR (referred_user_id IS NOT NULL AND claimed_at IS NOT NULL)
  ),
  CONSTRAINT referral_invites_qualification_shape CHECK (
    (qualifying_invoice_id IS NULL AND qualified_at IS NULL)
    OR (qualifying_invoice_id IS NOT NULL AND qualified_at IS NOT NULL)
  ),
  CONSTRAINT referral_invites_reward_shape CHECK (
    (reward_balance_transaction_id IS NULL AND rewarded_at IS NULL)
    OR (
      reward_balance_transaction_id IS NOT NULL
      AND rewarded_at IS NOT NULL
      AND qualifying_invoice_id IS NOT NULL
    )
  )
);

CREATE INDEX referral_invites_referred_user_idx
  ON private.referral_invites (referred_user_id)
  WHERE referred_user_id IS NOT NULL;

REVOKE ALL ON TABLE private.referral_invites
  FROM PUBLIC, anon, authenticated;

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
  IF v_user_id IS NULL OR NOT EXISTS (
    SELECT 1
    FROM public.profiles AS profile
    JOIN stripe.subscriptions AS subscription
      ON subscription.customer = profile.stripe_customer_id
    WHERE profile.id = v_user_id
      AND subscription.status = 'active'
      AND NOT EXISTS (
        SELECT 1
        FROM private.account_deletion_jobs AS deletion
        WHERE deletion.owner_user_id = v_user_id
      )
  ) THEN
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
  IS 'Returns three referral slots for active paid subscribers, creating missing slots atomically.';

REVOKE ALL ON FUNCTION public.get_or_create_referral_invites()
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.get_or_create_referral_invites()
  TO authenticated;

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
    OR NOT EXISTS (
      SELECT 1
      FROM public.profiles AS referrer_profile
      JOIN stripe.subscriptions AS referrer_subscription
        ON referrer_subscription.customer = referrer_profile.stripe_customer_id
      WHERE referrer_profile.id = v_referral.referrer_user_id
        AND referrer_subscription.status = 'active'
    )
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
  IS 'Claims one available referral slot for a new, trial-eligible account.';

REVOKE ALL ON FUNCTION public.claim_referral(text)
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.claim_referral(text)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.get_referral_trial_days()
RETURNS integer
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT CASE
    WHEN EXISTS (
      SELECT 1
      FROM private.referral_invites AS referral
      WHERE referral.referred_user_id = auth.uid()
    ) THEN 30
    ELSE NULL
  END;
$$;

COMMENT ON FUNCTION public.get_referral_trial_days()
  IS 'Returns the extended Pro trial duration for a referred account, otherwise null.';

REVOKE ALL ON FUNCTION public.get_referral_trial_days()
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.get_referral_trial_days()
  TO authenticated;

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

CREATE OR REPLACE FUNCTION public.complete_referral_reward(
  p_referral_id uuid,
  p_invoice_id text,
  p_balance_transaction_id text
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_updated integer;
BEGIN
  IF p_referral_id IS NULL
    OR p_invoice_id IS NULL
    OR p_balance_transaction_id IS NULL
    OR p_balance_transaction_id !~ '^cbtxn_[A-Za-z0-9]+$'
  THEN
    RETURN false;
  END IF;

  UPDATE private.referral_invites AS referral
  SET
    reward_balance_transaction_id = p_balance_transaction_id,
    rewarded_at = clock_timestamp()
  WHERE referral.id = p_referral_id
    AND referral.qualifying_invoice_id = p_invoice_id
    AND (
      referral.reward_balance_transaction_id IS NULL
      OR referral.reward_balance_transaction_id = p_balance_transaction_id
    );

  GET DIAGNOSTICS v_updated = ROW_COUNT;
  RETURN v_updated = 1;
END;
$$;

REVOKE ALL ON FUNCTION public.complete_referral_reward(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.complete_referral_reward(uuid, text, text)
  TO service_role;
