CREATE TABLE private.pro_trial_identity_claims (
  kind text NOT NULL CHECK (kind IN ('email', 'device')),
  identity_hash bytea NOT NULL,
  user_id uuid NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (kind, identity_hash)
);

ALTER TABLE private.pro_trial_identity_claims ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE private.pro_trial_identity_claims
  FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE
  ON TABLE private.pro_trial_identity_claims TO service_role;

CREATE OR REPLACE FUNCTION private.normalize_trial_email(p_email text)
RETURNS text
LANGUAGE plpgsql
IMMUTABLE
SET search_path = ''
AS $$
DECLARE
  v_email text := lower(btrim(p_email));
  v_local text;
  v_domain text;
BEGIN
  IF v_email IS NULL THEN
    RETURN NULL;
  END IF;

  IF length(v_email) - length(replace(v_email, '@', '')) <> 1 THEN
    RETURN v_email;
  END IF;

  v_local := split_part(v_email, '@', 1);
  v_domain := split_part(v_email, '@', 2);
  v_local := split_part(v_local, '+', 1);

  IF v_domain IN ('gmail.com', 'googlemail.com') THEN
    v_local := replace(v_local, '.', '');
    v_domain := 'gmail.com';
  END IF;

  RETURN v_local || '@' || v_domain;
END;
$$;

REVOKE ALL ON FUNCTION private.normalize_trial_email(text)
  FROM PUBLIC, anon, authenticated;

WITH eligible_users AS (
  SELECT
    extensions.digest(
      private.normalize_trial_email(auth_user.email),
      'sha256'
    ) AS identity_hash,
    auth_user.id AS user_id,
    auth_user.created_at
  FROM auth.users AS auth_user
  JOIN public.profiles AS profile
    ON profile.id = auth_user.id
  WHERE auth_user.email IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false
    AND (
      profile.trial_reservation_id IS NOT NULL
      OR EXISTS (
        SELECT 1
        FROM stripe.subscriptions AS subscription
        WHERE subscription.customer = profile.stripe_customer_id
      )
    )
),
earliest_users AS (
  SELECT DISTINCT ON (identity_hash)
    identity_hash,
    user_id
  FROM eligible_users
  ORDER BY identity_hash, created_at, user_id
)
INSERT INTO private.pro_trial_identity_claims (
  kind,
  identity_hash,
  user_id
)
SELECT 'email', identity_hash, user_id
FROM earliest_users
ON CONFLICT (kind, identity_hash) DO NOTHING;

DROP FUNCTION public.can_start_trial();
DROP FUNCTION public.reserve_pro_trial(text);

CREATE FUNCTION public.can_start_trial(
  p_device_fingerprint text DEFAULT NULL
)
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
  v_email text;
  v_email_hash bytea;
  v_device_hash bytea;
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

  SELECT private.normalize_trial_email(auth_user.email)
  INTO v_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_user_id;

  IF v_email IS NOT NULL THEN
    v_email_hash := extensions.digest(v_email, 'sha256');
    IF EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'email'
        AND claim.identity_hash = v_email_hash
        AND claim.user_id <> v_user_id
    ) THEN
      RETURN false;
    END IF;
  END IF;

  IF p_device_fingerprint IS NOT NULL
    AND p_device_fingerprint <> '30406ea523c53def'
    AND p_device_fingerprint ~ '^[0-9a-f]{1,16}$'
  THEN
    v_device_hash := extensions.digest(p_device_fingerprint, 'sha256');
    IF EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'device'
        AND claim.identity_hash = v_device_hash
        AND claim.user_id <> v_user_id
    ) THEN
      RETURN false;
    END IF;
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

COMMENT ON FUNCTION public.can_start_trial(text)
  IS 'Allows one new-user Pro trial per account, normalized email, and desktop device; prior subscription history makes the account ineligible.';

REVOKE ALL ON FUNCTION public.can_start_trial(text) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.can_start_trial(text) TO authenticated;

CREATE FUNCTION public.reserve_pro_trial(
  p_channel text,
  p_device_fingerprint text DEFAULT NULL
)
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
  v_email text;
  v_email_hash bytea;
  v_device_hash bytea;
  v_inserted_email boolean := false;
  v_inserted_device boolean := false;
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

  SELECT private.normalize_trial_email(auth_user.email)
  INTO v_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_user_id;

  IF v_email IS NOT NULL THEN
    v_email_hash := extensions.digest(v_email, 'sha256');
    IF EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'email'
        AND claim.identity_hash = v_email_hash
        AND claim.user_id <> v_user_id
    ) THEN
      RETURN;
    END IF;
  END IF;

  IF p_device_fingerprint IS NOT NULL
    AND p_device_fingerprint <> '30406ea523c53def'
    AND p_device_fingerprint ~ '^[0-9a-f]{1,16}$'
  THEN
    v_device_hash := extensions.digest(p_device_fingerprint, 'sha256');
    IF EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'device'
        AND claim.identity_hash = v_device_hash
        AND claim.user_id <> v_user_id
    ) THEN
      RETURN;
    END IF;
  END IF;

  IF v_email_hash IS NOT NULL THEN
    INSERT INTO private.pro_trial_identity_claims (
      kind,
      identity_hash,
      user_id
    )
    VALUES ('email', v_email_hash, v_user_id)
    ON CONFLICT (kind, identity_hash) DO NOTHING;
    v_inserted_email := FOUND;
  END IF;

  IF v_device_hash IS NOT NULL THEN
    INSERT INTO private.pro_trial_identity_claims (
      kind,
      identity_hash,
      user_id
    )
    VALUES ('device', v_device_hash, v_user_id)
    ON CONFLICT (kind, identity_hash) DO NOTHING;
    v_inserted_device := FOUND;
  END IF;

  IF (
    v_email_hash IS NOT NULL
    AND EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'email'
        AND claim.identity_hash = v_email_hash
        AND claim.user_id <> v_user_id
    )
  ) OR (
    v_device_hash IS NOT NULL
    AND EXISTS (
      SELECT 1
      FROM private.pro_trial_identity_claims AS claim
      WHERE claim.kind = 'device'
        AND claim.identity_hash = v_device_hash
        AND claim.user_id <> v_user_id
    )
  ) THEN
    IF v_inserted_email THEN
      DELETE FROM private.pro_trial_identity_claims
      WHERE kind = 'email'
        AND identity_hash = v_email_hash
        AND user_id = v_user_id;
    END IF;

    IF v_inserted_device THEN
      DELETE FROM private.pro_trial_identity_claims
      WHERE kind = 'device'
        AND identity_hash = v_device_hash
        AND user_id = v_user_id;
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

REVOKE ALL ON FUNCTION public.reserve_pro_trial(text, text) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.reserve_pro_trial(text, text) TO authenticated;
