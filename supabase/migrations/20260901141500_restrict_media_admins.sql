DO $$
BEGIN
  IF to_regclass('public.media_assets') IS NOT NULL THEN
    DROP POLICY IF EXISTS media_assets_admin_all ON public.media_assets;

    CREATE POLICY media_assets_admin_all
      ON public.media_assets
      AS PERMISSIVE
      FOR ALL
      TO authenticated
      USING (
        (SELECT lower(auth.jwt() ->> 'email')) = ANY (ARRAY[
          'john@fastrepl.com',
          'artem@fastrepl.com'
        ])
      )
      WITH CHECK (
        (SELECT lower(auth.jwt() ->> 'email')) = ANY (ARRAY[
          'john@fastrepl.com',
          'artem@fastrepl.com'
        ])
      );
  END IF;
END;
$$;
