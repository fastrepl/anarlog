begin;
select plan(18);

select tests.create_supabase_user(
  'cloud_api_owner',
  'cloud-api-owner@example.com'
);
select tests.authenticate_as_service_role();

select ok(
  (
    select bool_and(class.relrowsecurity)
    from pg_class as class
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname in (
        'api_cloud_settings',
        'api_session_snapshots',
        'api_keys'
      )
  ),
  'Cloud API tables have row-level security enabled'
);

select ok(
  not exists (
    select 1
    from information_schema.table_privileges as privilege
    where privilege.table_schema = 'public'
      and privilege.table_name in (
        'api_cloud_settings',
        'api_session_snapshots',
        'api_keys'
      )
      and privilege.grantee in ('PUBLIC', 'anon', 'authenticated')
  ),
  'Untrusted roles cannot access Cloud API tables directly'
);

select ok(
  (
      select count(*) = 11
      and bool_and(has_function_privilege('service_role', proc.oid, 'EXECUTE'))
      and bool_and(
        not has_function_privilege('authenticated', proc.oid, 'EXECUTE')
      )
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'get_cloud_api_settings',
        'set_cloud_api_enabled',
        'publish_cloud_api_snapshot',
        'delete_cloud_api_snapshot',
        'list_cloud_api_snapshots',
        'read_cloud_api_snapshot',
        'create_cloud_api_key',
        'list_cloud_api_keys',
        'revoke_cloud_api_key',
        'verify_cloud_api_key',
        'verify_cloud_api_user'
      )
  ),
  'Only service code can execute Cloud API RPCs'
);

select results_eq(
  format(
    $$
      select enabled
      from public.get_cloud_api_settings(%L::uuid)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array[false],
  'Cloud API is disabled by default'
);

select throws_ok(
  format(
    $$
      select *
      from public.set_cloud_api_enabled(%L::uuid, true)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  '42501',
  'hyprnote pro entitlement required',
  'A free account cannot enable Cloud API'
);

select tests.clear_authentication();
reset role;

update public.profiles
set stripe_customer_id = 'cus_cloud_api_owner'
where id = tests.get_supabase_uid('cloud_api_owner');

insert into stripe.active_entitlements (
  id,
  customer,
  lookup_key
) values (
  'ent_cloud_api_owner',
  'cus_cloud_api_owner',
  'hyprnote_pro'
);

select tests.authenticate_as_service_role();

select throws_ok(
  format(
    $$
      select *
      from public.publish_cloud_api_snapshot(
        %L::uuid,
        'meeting-before-opt-in',
        '{
          "id": "meeting-before-opt-in",
          "transcripts": []
        }'::jsonb
      )
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  '42501',
  'cloud API not enabled',
  'A Pro account cannot publish readable content before opt-in'
);

select results_eq(
  format(
    $$
      select enabled
      from public.set_cloud_api_enabled(%L::uuid, true)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array[true],
  'A Pro account can explicitly enable Cloud API'
);

select lives_ok(
  format(
    $$
      select *
      from public.publish_cloud_api_snapshot(
        %L::uuid,
        'meeting-1',
        '{
          "id": "meeting-1",
          "title": "Planning",
          "series_id": "series-1",
          "started_at": "2026-07-28T00:00:00Z",
          "updated_at": "2026-07-28T01:00:00Z",
          "transcripts": []
        }'::jsonb
      )
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  'An opted-in Pro account can publish a readable snapshot'
);

select results_eq(
  format(
    $$
      select content_json ->> 'title'
      from public.read_cloud_api_snapshot(%L::uuid, 'meeting-1')
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array['Planning'::text],
  'Published snapshots are readable while opted in'
);

select lives_ok(
  format(
    $$
      select *
      from public.create_cloud_api_key(
        %L::uuid,
        '00000000-0000-0000-0000-000000000187'::uuid,
        'Claude Code',
        repeat('a', 64),
        'anl_aaaaaaaa'
      )
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  'An opted-in Pro account can create a hashed API key'
);

select results_eq(
  $$
    select status
    from public.verify_cloud_api_key(repeat('a', 64))
  $$,
  array['ok'::text],
  'An active Pro key verifies successfully'
);

select results_eq(
  format(
    $$
      select status
      from public.verify_cloud_api_user(%L::uuid)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array['ok'::text],
  'An opted-in Pro user verifies successfully for OAuth access'
);

select tests.clear_authentication();
reset role;

delete from stripe.active_entitlements
where id = 'ent_cloud_api_owner';

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select status
    from public.verify_cloud_api_key(repeat('a', 64))
  $$,
  array['subscription_required'::text],
  'An expired Pro key returns subscription_required'
);

select results_eq(
  format(
    $$
      select status
      from public.verify_cloud_api_user(%L::uuid)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array['subscription_required'::text],
  'An expired Pro user returns subscription_required for OAuth access'
);

select results_eq(
  format(
    $$
      select enabled
      from public.set_cloud_api_enabled(%L::uuid, false)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array[false],
  'Opt-out remains available after Pro expires'
);

select results_eq(
  format(
    $$
      select count(*)
      from public.api_session_snapshots
      where user_id = %L::uuid
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array[0::bigint],
  'Opt-out atomically purges every server-readable snapshot'
);

select tests.clear_authentication();
reset role;

insert into stripe.active_entitlements (
  id,
  customer,
  lookup_key
) values (
  'ent_cloud_api_owner',
  'cus_cloud_api_owner',
  'hyprnote_pro'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select status
    from public.verify_cloud_api_key(repeat('a', 64))
  $$,
  array['cloud_api_not_enabled'::text],
  'A valid key reports cloud_api_not_enabled after opt-out'
);

select results_eq(
  format(
    $$
      select status
      from public.verify_cloud_api_user(%L::uuid)
    $$,
    tests.get_supabase_uid('cloud_api_owner')
  ),
  array['cloud_api_not_enabled'::text],
  'An opted-out user reports cloud_api_not_enabled for OAuth access'
);

select * from finish();
rollback;
