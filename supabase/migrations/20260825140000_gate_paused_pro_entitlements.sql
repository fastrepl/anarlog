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

  IF v_claims ->> 'subscription_status' = 'paused' THEN
    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;

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
  IS 'Requires a paid Pro entitlement or an unexpired server-issued Pro trial claim, and rejects paused subscriptions.';
