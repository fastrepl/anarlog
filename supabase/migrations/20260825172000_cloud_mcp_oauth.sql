-- Keep the existing subscription hook intact and bind OAuth access tokens to
-- the MCP resource only so they cannot be used as ordinary Anarlog sessions.
-- Idempotent so environments that already applied the later repair migrations
-- can still record this version.
DO $$
BEGIN
  IF to_regprocedure('public.custom_access_token_hook_base(jsonb)') IS NULL THEN
    ALTER FUNCTION public.custom_access_token_hook(jsonb)
      RENAME TO custom_access_token_hook_base;
  END IF;
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
BEGIN
  event := public.custom_access_token_hook_base(event);
  claims := event->'claims';

  IF NULLIF(claims->>'client_id', '') IS NOT NULL THEN
    claims := jsonb_set(
      claims,
      '{aud}',
      '["https://api.anarlog.so/mcp"]'::jsonb
    );
    event := jsonb_set(event, '{claims}', claims);
  END IF;

  RETURN event;
END;
$$;

GRANT EXECUTE ON FUNCTION public.custom_access_token_hook(jsonb)
  TO supabase_auth_admin;
REVOKE EXECUTE ON FUNCTION public.custom_access_token_hook(jsonb)
  FROM authenticated, anon, public;

CREATE OR REPLACE FUNCTION public.verify_cloud_api_user(p_user_id uuid)
RETURNS TABLE (status text)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_enabled boolean;
BEGIN
  IF p_user_id IS NULL THEN
    RETURN QUERY SELECT 'unauthorized'::text;
    RETURN;
  END IF;

  IF NOT private.cloud_api_user_has_pro(p_user_id) THEN
    RETURN QUERY SELECT 'subscription_required'::text;
    RETURN;
  END IF;

  SELECT COALESCE(settings.enabled, false) INTO v_enabled
  FROM (SELECT 1) AS singleton
  LEFT JOIN public.api_cloud_settings AS settings
    ON settings.user_id = p_user_id;

  IF NOT v_enabled THEN
    RETURN QUERY SELECT 'cloud_api_not_enabled'::text;
    RETURN;
  END IF;

  RETURN QUERY SELECT 'ok'::text;
END;
$$;

REVOKE ALL ON FUNCTION public.verify_cloud_api_user(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.verify_cloud_api_user(uuid)
  TO service_role;
