CREATE OR REPLACE FUNCTION public.custom_access_token_hook(event jsonb)
RETURNS jsonb
LANGUAGE plpgsql
STABLE
AS $$
DECLARE
  claims jsonb;
  entitlements jsonb := '[]'::jsonb;
BEGIN
  SELECT
    COALESCE(
      jsonb_agg(ae.lookup_key ORDER BY ae.lookup_key)
        FILTER (WHERE ae.lookup_key IS NOT NULL),
      '[]'::jsonb
    )
  INTO entitlements
  FROM public.profiles p
  JOIN stripe.active_entitlements ae
    ON ae.customer = p.stripe_customer_id
  WHERE p.id = (event->>'user_id')::uuid;

  claims := event->'claims';
  claims := jsonb_set(claims, '{entitlements}', entitlements);
  event := jsonb_set(event, '{claims}', claims);

  RETURN event;
END;
$$;

GRANT USAGE ON SCHEMA public TO supabase_auth_admin;

GRANT EXECUTE ON FUNCTION public.custom_access_token_hook TO supabase_auth_admin;

REVOKE EXECUTE ON FUNCTION public.custom_access_token_hook FROM authenticated, anon, public;

GRANT SELECT ON TABLE public.profiles TO supabase_auth_admin;

CREATE POLICY "Allow auth admin to read profiles"
ON public.profiles
AS PERMISSIVE FOR SELECT
TO supabase_auth_admin
USING (true);

GRANT USAGE ON SCHEMA stripe TO supabase_auth_admin;
GRANT SELECT ON TABLE stripe.active_entitlements TO supabase_auth_admin;
