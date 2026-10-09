-- Activate only after deploying the billing worker and new referral screens.
CREATE TABLE private.referral_program (
  id boolean PRIMARY KEY DEFAULT true CHECK (id),
  enabled boolean NOT NULL DEFAULT false,
  activated_at timestamptz,
  price_ids text[] NOT NULL DEFAULT ARRAY[]::text[],
  CHECK (NOT enabled OR cardinality(price_ids) > 0)
);
INSERT INTO private.referral_program DEFAULT VALUES;
CREATE FUNCTION private.activate_referral_program() RETURNS trigger
LANGUAGE plpgsql SET search_path = '' AS $$
BEGIN
  NEW.activated_at := COALESCE(OLD.activated_at, CASE WHEN NEW.enabled THEN now() END);
  RETURN NEW;
END;
$$;
CREATE TRIGGER activate_referral_program BEFORE UPDATE ON private.referral_program
  FOR EACH ROW EXECUTE FUNCTION private.activate_referral_program();
REVOKE ALL ON FUNCTION private.activate_referral_program() FROM PUBLIC,anon,authenticated;

ALTER TABLE private.referral_invites ADD COLUMN reward_policy text NOT NULL DEFAULT 'legacy_payment'
  CHECK (reward_policy IN ('legacy_payment', 'trial_month'));
CREATE TABLE private.referral_links (
  referrer_user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  code text NOT NULL UNIQUE DEFAULT encode(extensions.gen_random_bytes(12), 'hex'),
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE private.referral_trial_starts (
  referred_user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  subscription_id text NOT NULL UNIQUE,
  stripe_event_id text NOT NULL UNIQUE,
  started_at timestamptz NOT NULL,
  ends_at timestamptz NOT NULL
);
CREATE TABLE private.referral_month_rewards (
  referral_id uuid PRIMARY KEY REFERENCES private.referral_invites(id) ON DELETE CASCADE,
  subscription_id text,
  previous_end bigint,
  target_end bigint,
  applied_at timestamptz,
  attempts integer NOT NULL DEFAULT 0,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  last_error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  CHECK ((previous_end IS NULL AND target_end IS NULL) OR
    (previous_end IS NOT NULL AND target_end > previous_end))
);
CREATE INDEX referral_month_pending_idx ON private.referral_month_rewards(next_attempt_at, created_at)
  WHERE applied_at IS NULL;
CREATE TABLE private.referral_events (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  event_key text NOT NULL UNIQUE,
  kind text NOT NULL,
  referrer_user_id uuid NOT NULL,
  referred_user_id uuid,
  referral_id uuid,
  occurred_at timestamptz NOT NULL DEFAULT now(),
  details jsonb NOT NULL DEFAULT '{}'::jsonb
);
ALTER TABLE private.referral_program ENABLE ROW LEVEL SECURITY;
ALTER TABLE private.referral_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE private.referral_trial_starts ENABLE ROW LEVEL SECURITY;
ALTER TABLE private.referral_month_rewards ENABLE ROW LEVEL SECURITY;
ALTER TABLE private.referral_events ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON private.referral_program, private.referral_links, private.referral_trial_starts,
  private.referral_month_rewards, private.referral_events FROM PUBLIC, anon, authenticated;

CREATE FUNCTION private.can_refer_for_month(p_user_id uuid) RETURNS boolean
LANGUAGE sql SECURITY DEFINER SET search_path = '' AS $$
  SELECT EXISTS (
    SELECT 1 FROM public.profiles p JOIN stripe.subscriptions s ON s.customer = p.stripe_customer_id
    WHERE p.id = p_user_id AND EXISTS (SELECT 1 FROM stripe.subscription_items si
      WHERE si.subscription=s.id AND si.price = ANY((SELECT price_ids FROM private.referral_program)::text[])) AND (s.status = 'active' OR
      (s.status = 'trialing' AND s.metadata->>'referral_extension' = 'true'))
  ) AND NOT EXISTS (SELECT 1 FROM private.account_deletion_jobs WHERE owner_user_id = p_user_id);
$$;

CREATE OR REPLACE FUNCTION private.claim_legacy_referral(p_code text)
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

COMMENT ON FUNCTION private.claim_legacy_referral(text)
  IS 'Claims one available referral slot for a new, trial-eligible account. The referrer must have an active paid personal or shared-workspace subscription.';


CREATE OR REPLACE FUNCTION public.claim_referral(p_code text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
DECLARE
  v_user uuid := auth.uid();
  v_referrer uuid;
  v_invite private.referral_invites%ROWTYPE;
BEGIN
  IF NOT (SELECT enabled FROM private.referral_program) THEN
    IF (SELECT activated_at FROM private.referral_program) IS NOT NULL THEN RETURN false; END IF;
    RETURN private.claim_legacy_referral(p_code);
  END IF;
  IF v_user IS NULL OR p_code !~ '^[a-f0-9]{24}$' THEN RETURN false; END IF;
  SELECT referrer_user_id INTO v_referrer FROM private.referral_links WHERE code = p_code;
  IF v_referrer IS NULL THEN
    SELECT referrer_user_id INTO v_referrer FROM private.referral_invites WHERE code = p_code;
  END IF;
  IF v_referrer IS NULL OR v_referrer = v_user THEN RETURN false; END IF;
  PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(v_user::text, 180002));
  PERFORM pg_catalog.pg_advisory_xact_lock(pg_catalog.hashtextextended(v_referrer::text, 180001));
  SELECT * INTO v_invite FROM private.referral_invites WHERE referred_user_id = v_user;
  IF FOUND THEN RETURN v_invite.referrer_user_id = v_referrer; END IF;
  -- Claimed legacy URLs stay single-use; only unused URLs are aliases.
  IF EXISTS (SELECT 1 FROM private.referral_invites WHERE code = p_code AND claimed_at IS NOT NULL AND reward_policy = 'legacy_payment')
    OR NOT private.can_refer_for_month(v_referrer) THEN RETURN false; END IF;
  IF NOT EXISTS (
    SELECT 1 FROM auth.users u JOIN public.profiles p ON p.id = u.id
    WHERE u.id = v_user AND NOT COALESCE(u.is_anonymous, false)
      AND u.email_confirmed_at IS NOT NULL AND u.created_at >= now() - interval '7 days'
      AND NOT EXISTS (SELECT 1 FROM private.account_deletion_jobs WHERE owner_user_id = v_user)
      AND NOT EXISTS (SELECT 1 FROM stripe.subscriptions s WHERE s.customer = p.stripe_customer_id)
  ) THEN RETURN false; END IF;
  INSERT INTO private.referral_invites(referrer_user_id, slot, code)
    SELECT v_referrer, n, encode(extensions.gen_random_bytes(12), 'hex') FROM generate_series(1,3) n
    ON CONFLICT ON CONSTRAINT referral_invites_referrer_slot_key DO NOTHING;
  SELECT * INTO v_invite FROM private.referral_invites
    WHERE referrer_user_id = v_referrer AND claimed_at IS NULL ORDER BY slot LIMIT 1 FOR UPDATE;
  IF NOT FOUND THEN RETURN false; END IF;
  UPDATE private.referral_invites SET referred_user_id = v_user, claimed_at = now(), reward_policy = 'trial_month'
    WHERE id = v_invite.id;
  INSERT INTO private.referral_events(event_key, kind, referrer_user_id, referred_user_id, referral_id)
    VALUES ('claimed:' || v_invite.id, 'referral_claimed', v_referrer, v_user, v_invite.id);
  RETURN true;
END;
$$;

CREATE FUNCTION public.get_referral_summary() RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
DECLARE
  v_user uuid := auth.uid();
  v_enabled boolean;
  v_eligible boolean;
  v_code text;
  v_accepted integer;
  v_earned integer;
  v_invites jsonb;
BEGIN
  IF v_user IS NULL OR NOT EXISTS (SELECT 1 FROM auth.users WHERE id = v_user AND NOT COALESCE(is_anonymous, false))
    THEN RAISE EXCEPTION 'Unauthorized'; END IF;
  SELECT enabled INTO v_enabled FROM private.referral_program;
  v_eligible := v_enabled AND private.can_refer_for_month(v_user);
  IF v_eligible THEN
    INSERT INTO private.referral_links(referrer_user_id) VALUES (v_user) ON CONFLICT DO NOTHING;
    SELECT code INTO v_code FROM private.referral_links WHERE referrer_user_id = v_user;
    INSERT INTO private.referral_events(event_key, kind, referrer_user_id)
      VALUES ('link:' || v_user, 'referral_link_created', v_user) ON CONFLICT DO NOTHING;
  END IF;
  SELECT count(*)::integer, count(r.applied_at)::integer,
    COALESCE(jsonb_agg(jsonb_build_object(
      'id', i.id, 'slot', i.slot, 'policy', i.reward_policy, 'accepted_at', i.claimed_at,
      'trial_started_at', t.started_at, 'reward_applied_at', COALESCE(r.applied_at, i.rewarded_at),
      'extended_until', CASE WHEN r.applied_at IS NOT NULL THEN to_timestamp(r.target_end) END,
      'legacy_amount_cents', i.reward_amount_cents,
      'status', CASE WHEN i.reward_policy = 'legacy_payment' THEN
          CASE WHEN i.rewarded_at IS NOT NULL THEN 'legacy_applied' ELSE 'legacy_pending' END
        WHEN r.applied_at IS NOT NULL THEN 'applied'
        WHEN r.last_error IS NOT NULL THEN 'pending'
        WHEN t.started_at IS NOT NULL THEN 'applying'
        ELSE 'accepted' END
    ) ORDER BY i.slot), '[]'::jsonb)
    INTO v_accepted, v_earned, v_invites
    FROM private.referral_invites i
    LEFT JOIN private.referral_trial_starts t ON t.referred_user_id = i.referred_user_id
    LEFT JOIN private.referral_month_rewards r ON r.referral_id = i.id
    WHERE i.referrer_user_id = v_user AND i.claimed_at IS NOT NULL;
  RETURN jsonb_build_object('enabled', v_enabled, 'eligible', v_eligible, 'code', v_code,
    'accepted', v_accepted, 'remaining', GREATEST(0, 3-v_accepted), 'months_earned', v_earned, 'invites', v_invites);
END;
$$;

-- Keep the old wire contract, but never return new-policy links to old clients.
ALTER FUNCTION public.get_or_create_referral_invites() RENAME TO get_or_create_legacy_referral_invites;
ALTER FUNCTION public.get_or_create_legacy_referral_invites() SET SCHEMA private;
CREATE FUNCTION public.get_or_create_referral_invites()
RETURNS TABLE(slot smallint, code text, status text, reward_amount_cents integer, reward_currency text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
BEGIN
  IF (SELECT activated_at FROM private.referral_program) IS NOT NULL THEN
    RAISE EXCEPTION 'Please update Anarlog to view the new referral offer';
  END IF;
  RETURN QUERY SELECT * FROM private.get_or_create_legacy_referral_invites();
END;
$$;
ALTER FUNCTION public.prepare_referral_reward(uuid, text) RENAME TO prepare_legacy_referral_reward;
ALTER FUNCTION public.prepare_legacy_referral_reward(uuid, text) SET SCHEMA private;
CREATE FUNCTION public.prepare_referral_reward(p_referred_user_id uuid, p_invoice_id text)
RETURNS TABLE(referral_id uuid, referrer_user_id uuid, referrer_customer_id text, reward_amount_cents integer, reward_currency text)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM private.referral_invites WHERE referred_user_id = p_referred_user_id AND reward_policy = 'legacy_payment') THEN
    RETURN QUERY SELECT * FROM private.prepare_legacy_referral_reward(p_referred_user_id, p_invoice_id);
  END IF;
END;
$$;

CREATE VIEW private.referral_audit AS
SELECT i.id AS referral_id, i.referrer_user_id, i.referred_user_id, i.reward_policy,
  COALESCE(l.created_at, i.created_at) AS link_created_at, i.claimed_at, t.subscription_id AS friend_subscription_id,
  t.stripe_event_id, t.started_at AS trial_started_at, t.ends_at AS trial_ends_at,
  r.subscription_id AS rewarded_subscription_id, r.previous_end, r.target_end,
  r.applied_at, r.attempts, r.last_error, r.next_attempt_at,
  i.reward_amount_cents AS legacy_amount_cents, i.rewarded_at AS legacy_rewarded_at
FROM private.referral_invites i
LEFT JOIN private.referral_links l ON l.referrer_user_id = i.referrer_user_id
LEFT JOIN private.referral_trial_starts t ON t.referred_user_id = i.referred_user_id
LEFT JOIN private.referral_month_rewards r ON r.referral_id = i.id
WHERE i.claimed_at IS NOT NULL;
REVOKE ALL ON private.referral_audit FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.can_refer_for_month(uuid), private.claim_legacy_referral(text),
  private.get_or_create_legacy_referral_invites(), private.prepare_legacy_referral_reward(uuid,text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.get_referral_summary(), public.get_or_create_referral_invites(), public.claim_referral(text)
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.get_referral_summary(), public.get_or_create_referral_invites(), public.claim_referral(text) TO authenticated;
REVOKE ALL ON FUNCTION public.prepare_referral_reward(uuid,text) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.prepare_referral_reward(uuid,text) TO service_role;

-- Keep paid extensions out of new-account trial UI while retaining Stripe's real status.
CREATE FUNCTION private.is_referral_extension(p_user uuid, p_trial_end bigint) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = '' AS $$
  SELECT EXISTS (SELECT 1 FROM public.profiles p JOIN stripe.subscriptions s ON s.customer=p.stripe_customer_id
    WHERE p.id=p_user AND s.status='trialing' AND s.metadata->>'referral_extension'='true'
      AND (s.trial_end #>> '{}')::bigint=p_trial_end);
$$;
REVOKE ALL ON FUNCTION private.is_referral_extension(uuid,bigint) FROM PUBLIC,anon,authenticated;
GRANT EXECUTE ON FUNCTION private.is_referral_extension(uuid,bigint) TO supabase_auth_admin;
ALTER FUNCTION public.custom_access_token_hook(jsonb) RENAME TO custom_access_token_hook_before_referrals;
CREATE FUNCTION public.custom_access_token_hook(event jsonb) RETURNS jsonb
LANGUAGE plpgsql STABLE SET search_path = '' AS $$
BEGIN
  event := public.custom_access_token_hook_before_referrals(event);
  RETURN jsonb_set(event,'{claims,referral_extension}',to_jsonb(private.is_referral_extension(
    (event->>'user_id')::uuid, (event->'claims'->>'trial_end')::bigint)));
END;
$$;
REVOKE ALL ON FUNCTION public.custom_access_token_hook(jsonb) FROM PUBLIC,anon,authenticated;
GRANT EXECUTE ON FUNCTION public.custom_access_token_hook(jsonb) TO supabase_auth_admin;

-- A deleted invitee still consumed an invite; keep the acceptance tombstone.
ALTER TABLE private.referral_invites DROP CONSTRAINT referral_invites_claim_shape;
ALTER TABLE private.referral_invites ADD CONSTRAINT referral_invites_claim_shape
  CHECK (claimed_at IS NOT NULL OR referred_user_id IS NULL);
GRANT ALL ON private.referral_program, private.referral_links, private.referral_trial_starts,
  private.referral_month_rewards, private.referral_events TO service_role;
GRANT SELECT ON private.referral_audit TO service_role;
GRANT USAGE,SELECT ON SEQUENCE private.referral_events_id_seq TO service_role;

CREATE OR REPLACE FUNCTION public.list_due_trial_reminders(
  p_now timestamptz DEFAULT clock_timestamp(),
  p_window_seconds integer DEFAULT 82800
)
RETURNS TABLE (
  subscription_id text,
  customer_email text,
  customer_name text,
  trial_end bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_now IS NULL
    OR p_window_seconds IS NULL
    OR p_window_seconds < 300
    OR p_window_seconds > 86400
  THEN
    RAISE EXCEPTION 'invalid trial reminder window'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  SELECT
    subscription.id,
    btrim(customer.email),
    customer.name,
    trial.ending_at_epoch
  FROM stripe.subscriptions AS subscription
  JOIN stripe.customers AS customer
    ON customer.id = subscription.customer
  CROSS JOIN LATERAL (
    SELECT CASE
      WHEN jsonb_typeof(subscription.trial_end) = 'number'
      THEN (subscription.trial_end #>> '{}')::bigint
    END AS ending_at_epoch
  ) AS trial
  WHERE subscription.status = 'trialing'
    AND COALESCE(subscription.metadata->>'referral_extension','false') <> 'true'
    AND trial.ending_at_epoch IS NOT NULL
    AND NULLIF(btrim(customer.email), '') IS NOT NULL
    AND subscription.default_payment_method IS NULL
    AND customer.invoice_settings ->> 'default_payment_method' IS NULL
    AND customer.default_source IS NULL
    AND to_timestamp(trial.ending_at_epoch) > p_now
    AND to_timestamp(trial.ending_at_epoch) - interval '7 days' <= p_now
    AND to_timestamp(trial.ending_at_epoch) - interval '7 days'
      > p_now - make_interval(secs => p_window_seconds)
  ORDER BY subscription.id;
END;
$$;

CREATE FUNCTION public.referral_link_available(p_code text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
DECLARE v_referrer uuid; v_enabled boolean;
BEGIN
  IF p_code IS NULL OR p_code !~ '^[a-f0-9]{24}$' THEN RETURN false; END IF;
  SELECT enabled INTO v_enabled FROM private.referral_program;
  IF NOT v_enabled THEN
    IF (SELECT activated_at FROM private.referral_program) IS NOT NULL THEN RETURN false; END IF;
    RETURN EXISTS (SELECT 1 FROM private.referral_invites WHERE code=p_code AND claimed_at IS NULL AND private.is_paid_referrer(referrer_user_id));
  END IF;
  SELECT referrer_user_id INTO v_referrer FROM private.referral_links WHERE code=p_code;
  IF v_referrer IS NULL THEN
    SELECT referrer_user_id INTO v_referrer FROM private.referral_invites
      WHERE code=p_code AND (claimed_at IS NULL OR reward_policy='trial_month');
  END IF;
  RETURN v_referrer IS NOT NULL AND private.can_refer_for_month(v_referrer)
    AND (SELECT count(*) FROM private.referral_invites WHERE referrer_user_id=v_referrer AND claimed_at IS NOT NULL)<3;
END;
$$;
REVOKE ALL ON FUNCTION public.referral_link_available(text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.referral_link_available(text) TO anon,authenticated;

GRANT USAGE ON SCHEMA private TO supabase_auth_admin;
