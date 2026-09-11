-- Referral eligibility previously required a personal active Stripe
-- subscription, so members whose Pro came from a paid workspace seat were
-- excluded even though the access token grants them hyprnote_pro. Mirror the
-- cloud_api_user_has_pro hook for both creating invites and claiming them.
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
  IF v_user_id IS NULL OR NOT private.cloud_api_user_has_pro(v_user_id)
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
  IS 'Returns three referral slots for paid Pro subscribers, including workspace seats, creating missing slots atomically.';

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
    OR NOT private.cloud_api_user_has_pro(v_referral.referrer_user_id)
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
  IS 'Claims one available referral slot for a new, trial-eligible account. The referrer may be Pro through a personal subscription or a paid workspace seat.';
