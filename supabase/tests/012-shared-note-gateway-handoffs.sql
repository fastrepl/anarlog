begin;
select plan(37);

select tests.create_supabase_user('gateway_owner', 'gateway-owner@example.com');

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('gateway_owner');

create temporary table shared_note_gateway_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid,
  secret text,
  slug text,
  expires_at timestamptz
);

grant all on shared_note_gateway_test_state to anon, authenticated, service_role;

insert into shared_note_gateway_test_state (name, workspace_id)
values ('workspace', gen_random_uuid());

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('gateway_owner'),
  'shared',
  'Gateway workspace'
from shared_note_gateway_test_state
where name = 'workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('gateway_owner'),
  'owner'
from shared_note_gateway_test_state
where name = 'workspace';

select tests.clear_authentication();
reset role;

select ok(
  (
    select class.relrowsecurity
    from pg_class as class
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'private'
      and class.relname = 'session_share_handoffs'
  )
    and not exists (
      select 1
      from information_schema.table_privileges as privilege
      where privilege.table_schema = 'private'
        and privilege.table_name = 'session_share_handoffs'
        and privilege.grantee in ('PUBLIC', 'anon', 'authenticated', 'service_role')
    ),
  'The private handoff table has RLS and no direct client or service privileges'
);

select ok(
  (
    select count(*) = 10
    from pg_constraint as constraint_record
    join pg_class as class
      on class.oid = constraint_record.conrelid
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'private'
      and class.relname = 'session_share_handoffs'
      and constraint_record.conname in (
        'session_share_handoffs_pkey',
        'session_share_handoffs_request_hash_check',
        'session_share_handoffs_slot_check',
        'session_share_handoffs_access_kind_check',
        'session_share_handoffs_link_check',
        'session_share_handoffs_access_version_check',
        'session_share_handoffs_ttl_check',
        'session_share_handoffs_source_hash_check',
        'session_share_handoffs_lease_check',
        'session_share_handoffs_share_slot_key'
      )
  )
    and (
      select is_nullable = 'NO'
        and data_type = 'smallint'
      from information_schema.columns
      where table_schema = 'private'
        and table_name = 'session_share_handoffs'
      and column_name = 'slot'
    )
    and (
      select count(*) = 3
      from information_schema.columns
      where table_schema = 'private'
        and table_name = 'session_share_handoffs'
        and column_name in ('lease_hash', 'leased_at', 'lease_expires_at')
        and is_nullable = 'YES'
    )
    and (
      select is_nullable = 'NO'
        and data_type = 'bytea'
      from information_schema.columns
      where table_schema = 'private'
        and table_name = 'session_share_handoffs'
        and column_name = 'source_hash'
    )
    and (
      select pg_get_constraintdef(constraint_record.oid) like '%31%'
      from pg_constraint as constraint_record
      join pg_class as class
        on class.oid = constraint_record.conrelid
      join pg_namespace as namespace
        on namespace.oid = class.relnamespace
      where namespace.nspname = 'private'
        and class.relname = 'session_share_handoffs'
        and constraint_record.conname = 'session_share_handoffs_slot_check'
    )
    and exists (
      select 1
      from pg_indexes
      where schemaname = 'private'
        and tablename = 'session_share_handoffs'
        and indexname = 'session_share_handoffs_lease_hash_key'
    ),
  'Handoffs use opaque source digests, exact TTLs, and a 32-row per-share pool'
);

select ok(
  (
    select count(*) = 5
      and bool_and(has_function_privilege('service_role', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('authenticated', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'gateway_read_session_share_link_snapshot',
        'gateway_read_public_session_share_snapshot',
        'gateway_create_session_share_link_handoff',
        'gateway_create_public_session_share_handoff',
        'gateway_lease_session_share_handoff'
      )
  ),
  'Only service role can execute public gateway wrappers'
);

select ok(
  (
    select count(*) = 5
      and bool_and(has_function_privilege('service_role', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('authenticated', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'private'
      and proc.proname in (
        'gateway_read_session_share_link_snapshot',
        'gateway_read_public_session_share_snapshot',
        'gateway_create_session_share_link_handoff',
        'gateway_create_public_session_share_handoff',
        'gateway_lease_session_share_handoff'
      )
  )
    and not has_function_privilege(
      'service_role',
      'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)',
      'EXECUTE'
    )
    and to_regprocedure(
      'public.gateway_claim_session_share_handoff(text)'
    ) is null
    and to_regprocedure(
      'private.gateway_claim_session_share_handoff(text)'
    ) is null
    and to_regprocedure(
      'public.gateway_claim_session_share_handoff_v2(text)'
    ) is null
    and to_regprocedure(
      'private.gateway_claim_session_share_handoff_v2(text)'
    ) is null,
  'Service role reaches leased gateways and destructive claim RPCs are removed'
);

select ok(
  not has_schema_privilege('anon', 'private', 'USAGE')
    and not exists (
      select 1
      from pg_proc as proc
      join pg_namespace as namespace
        on namespace.oid = proc.pronamespace
      where namespace.nspname in ('public', 'private')
        and proc.proname in (
          'resolve_session_share_link',
          'resolve_public_session_share',
          'read_session_share_link_snapshot',
          'read_public_session_share_snapshot'
        )
        and (
          has_function_privilege('anon', proc.oid, 'EXECUTE')
          or has_function_privilege('authenticated', proc.oid, 'EXECUTE')
        )
    ),
  'Legacy general-access RPCs and the private schema are closed to clients'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where proc.proname like 'gateway_%session_share%'
      and (
        (namespace.nspname = 'public' and proc.prosecdef)
        or (namespace.nspname = 'private' and not proc.prosecdef)
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  ),
  'Gateway wrappers are invokers and private implementations are hardened definers'
);

select ok(
  (
    select lower(pg_get_functiondef(
      'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
    )) not like '%delete from private.session_share_handoffs%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
      )) like '%on conflict (share_id, slot) do update%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
      )) like '%pg_advisory_xact_lock%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
      )) like '%generate_series(0, 31)%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
      )) like '%v_source_active_count >= 4%'
  )
    and lower(pg_get_functiondef(
      'private.gateway_create_session_share_link_handoff(uuid,text,text)'::regprocedure
    )) not like '%for update%'
    and lower(pg_get_functiondef(
      'private.gateway_create_public_session_share_handoff(text,text)'::regprocedure
    )) not like '%for update%'
    and lower(pg_get_functiondef(
      'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
    )) like '%for share of share%'
    and lower(pg_get_functiondef(
      'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
    )) like '%pg_advisory_xact_lock%'
    and strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'select handoff.share_id'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'pg_catalog.pg_advisory_xact_lock'
    )
    and strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'pg_catalog.pg_advisory_xact_lock'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'for update'
    )
    and strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'for update'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
      )),
      'for share of share'
    )
    and lower(pg_get_functiondef(
      'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
    )) not like '%request_hash in%'
    and lower(pg_get_functiondef(
      'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
    )) like '%private.session_share_attachment_manifest(share.id) as attachments_json%'
    and lower(pg_get_functiondef(
      'private.gateway_lease_session_share_handoff(text,text)'::regprocedure
    )) not like '%private.session_share_attachment_manifest(v_snapshot.share_id%',
  'Issuance is source-bounded and leasing serializes before locking handoff and share rows'
);

select tests.authenticate_as_hyprnote_pro('gateway_owner');

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, slug)
    select 'public_share', share_id, public_slug
    from public.create_session_share(
      (select workspace_id from shared_note_gateway_test_state where name = 'workspace'),
      'gateway-public-session'
    )
  $query$,
  'The owner can create the public gateway fixture'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, slug)
    select 'link_share', share_id, public_slug
    from public.create_session_share(
      (select workspace_id from shared_note_gateway_test_state where name = 'workspace'),
      'gateway-link-session'
    )
  $query$,
  'The owner can create the link gateway fixture'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, entity_id, secret)
    select 'active_link', share_id, link_id, link_token
    from public.enable_session_share_link(
      (select share_id from shared_note_gateway_test_state where name = 'link_share')
    )
  $query$,
  'The link fixture has an active bearer token'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      'public'
    )
  $$,
  'The public fixture is enabled by slug'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $$
    select *
    from public.publish_session_share_snapshot(
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      tests.get_supabase_uid('gateway_owner'),
      'Public gateway snapshot',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Public"}]}]}'::jsonb
    )
  $$,
  'Trusted publication creates the public gateway snapshot'
);

select lives_ok(
  $$
    select *
    from public.publish_session_share_snapshot(
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      tests.get_supabase_uid('gateway_owner'),
      'Link gateway snapshot',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Link"}]}]}'::jsonb
    )
  $$,
  'Trusted publication creates the link gateway snapshot'
);

select is(
  (
    select
      to_jsonb(snapshot) ?& array[
        'share_id',
        'schema_version',
        'content_revision',
        'title',
        'body_json',
        'published_at'
      ]
      and not to_jsonb(snapshot) ?| array[
        'workspace_id',
        'session_id',
        'access_version',
        'capability',
        'manage_access'
      ]
      and snapshot.title = 'Public gateway snapshot'
    from public.gateway_read_public_session_share_snapshot(
      (select slug from shared_note_gateway_test_state where name = 'public_share')
    ) AS snapshot
  ),
  true,
  'Public gateway reads expose only the minimal sanitized snapshot projection'
);

select is(
  (
    select
      not to_jsonb(snapshot) ?| array[
        'workspace_id',
        'session_id',
        'capability',
        'manage_access'
      ]
      and snapshot.title = 'Link gateway snapshot'
    from public.gateway_read_session_share_link_snapshot(
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      (select secret from shared_note_gateway_test_state where name = 'active_link')
    ) AS snapshot
  ),
  true,
  'Bearer gateway reads expose no workspace or access-management metadata'
);

select ok(
  (
    select count(*) = 0
    from public.gateway_read_public_session_share_snapshot(repeat('s', 10000))
  )
    and (
      select count(*) = 0
      from public.gateway_read_public_session_share_snapshot('S_00000000000000000000000000000000')
    )
    and (
      select count(*) = 0
      from public.gateway_read_session_share_link_snapshot(
        (select share_id from shared_note_gateway_test_state where name = 'link_share'),
        repeat('A', 10000)
      )
    )
    and (
      select count(*) = 0
      from public.gateway_read_session_share_link_snapshot(
        (select share_id from shared_note_gateway_test_state where name = 'link_share'),
        'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA!'
      )
    )
    and (
      select count(*) = 0
      from public.gateway_create_public_session_share_handoff(
        (select slug from shared_note_gateway_test_state where name = 'public_share'),
        repeat('A', 64)
      )
    ),
  'Gateway inputs and source digests are length-bounded and canonical before lookup'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, secret, expires_at)
    select 'public_handoff',
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      request_id,
      expires_at
    from public.gateway_create_public_session_share_handoff(
      (select slug from shared_note_gateway_test_state where name = 'public_share'),
      repeat('a', 64)
    )
  $query$,
  'The gateway can create a public handoff request'
);

select tests.clear_authentication();
reset role;

select ok(
  (
    select secret ~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
      and expires_at > clock_timestamp()
    from shared_note_gateway_test_state
    where name = 'public_handoff'
  )
    and (
      select expires_at = created_at + interval '60 seconds'
      from private.session_share_handoffs
      where request_hash = extensions.digest(
        (select secret from shared_note_gateway_test_state where name = 'public_handoff'),
        'sha256'
      )
    ),
  'Handoff IDs are canonical UUID v4 capabilities with an exact 60-second TTL'
);

select ok(
  exists (
    select 1
    from private.session_share_handoffs
    where request_hash = extensions.digest(
      (select secret from shared_note_gateway_test_state where name = 'public_handoff'),
      'sha256'
    )
  )
    and not exists (
      select 1
      from information_schema.columns
      where table_schema = 'private'
        and table_name = 'session_share_handoffs'
        and column_name in ('request_id', 'request_token', 'link_token', 'secret')
    ),
  'Handoff capabilities and source identities are stored only as opaque digests'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select title
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_handoff'),
      'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
    )
  $$,
  array['Public gateway snapshot'::text],
  'A valid public handoff atomically creates a lease and returns the snapshot'
);

select results_eq(
  $$
    select title
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_handoff'),
      'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb'
    )
  $$,
  array['Public gateway snapshot'::text],
  'Retrying the same request and lease pair is idempotent'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, secret, expires_at)
    select 'link_handoff',
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      request_id,
      expires_at
    from public.gateway_create_session_share_link_handoff(
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      (select secret from shared_note_gateway_test_state where name = 'active_link'),
      repeat('b', 64)
    )
  $query$,
  'The gateway can create a bearer-link handoff request'
);

select results_eq(
  $$
    select title
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_handoff'),
      'cccccccc-cccc-4ccc-8ccc-cccccccccccc'
    )
  $$,
  array['Link gateway snapshot'::text],
  'A valid bearer-link handoff returns the sanitized snapshot'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_handoff'),
      'dddddddd-dddd-4ddd-8ddd-dddddddddddd'
    )
  $$,
  array[0::bigint],
  'A different lease cannot take over a claimed handoff'
);

select lives_ok(
  $query$
    do $$
    begin
      for handoff_index in 1..6 loop
        perform *
        from public.gateway_create_public_session_share_handoff(
          (select slug from shared_note_gateway_test_state where name = 'public_share'),
          repeat('a', 64)
        );
      end loop;
    end
    $$
  $query$,
  'Repeated creation is accepted without an unbounded per-share queue'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_create_public_session_share_handoff(
      (select slug from shared_note_gateway_test_state where name = 'public_share'),
      repeat('a', 64)
    )
  $$,
  array[0::bigint],
  'One source cannot exceed four active handoffs for a share'
);

select tests.clear_authentication();
reset role;

select results_eq(
  $$
    select
      count(*),
      count(distinct slot),
      min(slot),
      max(slot),
      exists (
        select 1
        from private.session_share_handoffs as leased
        where leased.request_hash = extensions.digest(
          (select secret from shared_note_gateway_test_state where name = 'public_handoff'),
          'sha256'
        )
          and leased.lease_hash = extensions.digest(
            'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
            'sha256'
          )
          and leased.lease_expires_at = leased.leased_at + interval '20 minutes'
      )
    from private.session_share_handoffs
    where share_id = (
      select share_id from shared_note_gateway_test_state where name = 'public_share'
    )
  $$,
  $$values (4::bigint, 4::bigint, 0::smallint, 3::smallint, true)$$,
  'Four database slots stay bounded without overwriting an active lease'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select leased.title
    from public.gateway_create_public_session_share_handoff(
      (select slug from shared_note_gateway_test_state where name = 'public_share'),
      repeat('e', 64)
    ) as handoff
    cross join lateral public.gateway_lease_session_share_handoff(
      handoff.request_id,
      'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee'
    ) as leased
  $$,
  array['Public gateway snapshot'::text],
  'A distinct source can issue and lease while another source is at quota'
);

select tests.clear_authentication();
reset role;

select lives_ok(
  $$
    update private.session_share_handoffs as handoff
    set
      request_hash = extensions.digest(
        'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        'sha256'
      ),
      lease_hash = null,
      leased_at = null,
      lease_expires_at = null,
      created_at = timing.instant - interval '61 seconds',
      expires_at = timing.instant - interval '1 second'
    from (select clock_timestamp() as instant) as timing
    where handoff.share_id = (
      select share_id from shared_note_gateway_test_state where name = 'public_share'
    )
      and handoff.slot = 0
  $$,
  'An expired fixture can satisfy the exact TTL constraint'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee'
    )
  $$,
  array[0::bigint],
  'Expired handoffs cannot be claimed'
);

select tests.clear_authentication();
reset role;

select results_eq(
  $$
    select count(*)
    from private.session_share_handoffs
    where request_hash = extensions.digest(
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      'sha256'
    )
  $$,
  array[1::bigint],
  'An expired handoff remains bounded and available for slot reuse'
);

select tests.authenticate_as_service_role();

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, secret, expires_at)
    select 'public_revocation_handoff',
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      request_id,
      expires_at
    from public.gateway_create_public_session_share_handoff(
      (select slug from shared_note_gateway_test_state where name = 'public_share'),
      repeat('c', 64)
    )
  $query$,
  'A public handoff exists before scope revocation'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('gateway_owner');

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      'restricted'
    )
  $$,
  'The owner can revoke public scope before handoff claim'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_revocation_handoff'),
      'ffffffff-ffff-4fff-8fff-ffffffffffff'
    )
  $$,
  array[0::bigint],
  'Claim revalidates public scope and access version'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, secret, expires_at)
    select 'link_revocation_handoff',
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      request_id,
      expires_at
    from public.gateway_create_session_share_link_handoff(
      (select share_id from shared_note_gateway_test_state where name = 'link_share'),
      (select secret from shared_note_gateway_test_state where name = 'active_link'),
      repeat('d', 64)
    )
  $query$,
  'A bearer handoff exists before link rotation'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('gateway_owner');

select lives_ok(
  $$
    select *
    from public.rotate_session_share_link(
      (select share_id from shared_note_gateway_test_state where name = 'link_share')
    )
  $$,
  'The owner can rotate the bearer before handoff claim'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_revocation_handoff'),
      '99999999-9999-4999-8999-999999999999'
    )
  $$,
  array[0::bigint],
  'Claim revalidates the active link identity and access version'
);

select * from finish();
rollback;
