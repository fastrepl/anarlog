DO $$
DECLARE
  table_name text;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'checkpoint_migrations',
    'checkpoints',
    'checkpoint_blobs',
    'checkpoint_writes'
  ]
  LOOP
    IF to_regclass(format('public.%I', table_name)) IS NOT NULL THEN
      EXECUTE format('ALTER TABLE public.%I ENABLE ROW LEVEL SECURITY', table_name);
      EXECUTE format('REVOKE ALL ON TABLE public.%I FROM anon, authenticated', table_name);
      EXECUTE format('GRANT ALL ON TABLE public.%I TO service_role', table_name);
      EXECUTE format('DROP POLICY IF EXISTS checkpoint_internal_only ON public.%I', table_name);
      EXECUTE format(
        'CREATE POLICY checkpoint_internal_only ON public.%I AS RESTRICTIVE FOR ALL TO anon, authenticated USING (false) WITH CHECK (false)',
        table_name
      );
    END IF;
  END LOOP;
END;
$$;

DO $$
BEGIN
  IF to_regclass('public.media_assets') IS NOT NULL THEN
    ALTER TABLE public.media_assets ENABLE ROW LEVEL SECURITY;

    REVOKE ALL ON TABLE public.media_assets FROM anon, authenticated;
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.media_assets TO authenticated;
    GRANT ALL ON TABLE public.media_assets TO service_role;

    DROP POLICY IF EXISTS media_assets_admin_all ON public.media_assets;
    -- This mirrors the server-side admin allowlist so existing admin media routes keep working under RLS.
    CREATE POLICY media_assets_admin_all
      ON public.media_assets
      AS PERMISSIVE
      FOR ALL
      TO authenticated
      USING (
        (SELECT lower(auth.jwt() ->> 'email')) = ANY (ARRAY[
          'yujonglee@hyprnote.com',
          'yujonglee.dev@gmail.com',
          'john@hyprnote.com',
          'marketing@hyprnote.com',
          'yunhyungjo@yonsei.ac.kr',
          'goranmoomin@daum.net',
          'artem@hyprnote.com',
          'stua@fastmail.com',
          'thestua@gmail.com'
        ])
      )
      WITH CHECK (
        (SELECT lower(auth.jwt() ->> 'email')) = ANY (ARRAY[
          'yujonglee@hyprnote.com',
          'yujonglee.dev@gmail.com',
          'john@hyprnote.com',
          'marketing@hyprnote.com',
          'yunhyungjo@yonsei.ac.kr',
          'goranmoomin@daum.net',
          'artem@hyprnote.com',
          'stua@fastmail.com',
          'thestua@gmail.com'
        ])
      );
  END IF;
END;
$$;

DO $$
BEGIN
  IF to_regprocedure('public.set_updated_at()') IS NOT NULL THEN
    ALTER FUNCTION public.set_updated_at() SET search_path = '';
  END IF;

  IF to_regprocedure('public.handle_new_user()') IS NOT NULL THEN
    ALTER FUNCTION public.handle_new_user() SET search_path = '';
    REVOKE EXECUTE ON FUNCTION public.handle_new_user() FROM PUBLIC, anon, authenticated;
    GRANT EXECUTE ON FUNCTION public.handle_new_user() TO supabase_auth_admin;
  END IF;

  IF to_regprocedure('public.handle_user_email_update()') IS NOT NULL THEN
    ALTER FUNCTION public.handle_user_email_update() SET search_path = '';
    REVOKE EXECUTE ON FUNCTION public.handle_user_email_update() FROM PUBLIC, anon, authenticated;
    GRANT EXECUTE ON FUNCTION public.handle_user_email_update() TO supabase_auth_admin;
  END IF;

  IF to_regprocedure('public.custom_access_token_hook(jsonb)') IS NOT NULL THEN
    ALTER FUNCTION public.custom_access_token_hook(jsonb) SET search_path = '';
  END IF;
END;
$$;
