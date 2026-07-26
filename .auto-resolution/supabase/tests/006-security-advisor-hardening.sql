begin;
select plan(10);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public'
      and c.relname in (
        'checkpoint_migrations',
        'checkpoints',
        'checkpoint_blobs',
        'checkpoint_writes',
        'media_assets'
      )
      and not c.relrowsecurity
  ),
  'RLS is enabled on internal public tables'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join unnest(array['SELECT', 'INSERT', 'UPDATE', 'DELETE']) privilege
    where n.nspname = 'public'
      and c.relname in (
        'checkpoint_migrations',
        'checkpoints',
        'checkpoint_blobs',
        'checkpoint_writes'
      )
      and has_table_privilege('anon', c.oid, privilege)
  ),
  'Anonymous users cannot access checkpoint tables'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join unnest(array['SELECT', 'INSERT', 'UPDATE', 'DELETE']) privilege
    where n.nspname = 'public'
      and c.relname in (
        'checkpoint_migrations',
        'checkpoints',
        'checkpoint_blobs',
        'checkpoint_writes'
      )
      and has_table_privilege('authenticated', c.oid, privilege)
  ),
  'Authenticated users cannot access checkpoint tables'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public'
      and c.relname in (
        'checkpoint_migrations',
        'checkpoints',
        'checkpoint_blobs',
        'checkpoint_writes'
      )
      and not exists (
        select 1
        from pg_policies p
        where p.schemaname = n.nspname
          and p.tablename = c.relname
          and p.policyname = 'checkpoint_internal_only'
          and p.permissive = 'RESTRICTIVE'
          and p.cmd = 'ALL'
      )
  ),
  'Checkpoint tables have an explicit deny policy for API roles'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join unnest(array['SELECT', 'INSERT', 'UPDATE', 'DELETE']) privilege
    where n.nspname = 'public'
      and c.relname = 'media_assets'
      and has_table_privilege('anon', c.oid, privilege)
  ),
  'Anonymous users cannot access media assets'
);

select ok(
  to_regclass('public.media_assets') is null
    or exists (
      select 1
      from pg_policies
      where schemaname = 'public'
        and tablename = 'media_assets'
        and policyname = 'media_assets_admin_all'
        and roles = array['authenticated']::name[]
        and cmd = 'ALL'
    ),
  'Media assets have an authenticated admin policy'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join unnest(array['SELECT', 'INSERT', 'UPDATE', 'DELETE']) privilege
    where n.nspname = 'public'
      and c.relname in (
        'checkpoint_migrations',
        'checkpoints',
        'checkpoint_blobs',
        'checkpoint_writes',
        'media_assets'
      )
      and not has_table_privilege('service_role', c.oid, privilege)
  ),
  'Service role access is preserved'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.proname in (
        'set_updated_at',
        'handle_new_user',
        'handle_user_email_update',
        'custom_access_token_hook'
      )
      and not ('search_path=""' = any(coalesce(p.proconfig, array[]::text[])))
  ),
  'Security-sensitive functions use an immutable search path'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.proname in ('handle_new_user', 'handle_user_email_update')
      and has_function_privilege('anon', p.oid, 'EXECUTE')
  ),
  'Anonymous users cannot execute auth trigger functions'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.proname in ('handle_new_user', 'handle_user_email_update')
      and has_function_privilege('authenticated', p.oid, 'EXECUTE')
  ),
  'Authenticated users cannot execute auth trigger functions'
);

select * from finish();
rollback;
