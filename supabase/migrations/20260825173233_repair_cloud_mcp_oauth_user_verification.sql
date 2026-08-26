-- Repair environments where this RPC was recorded in migration history but
-- is missing from the deployed schema.
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
