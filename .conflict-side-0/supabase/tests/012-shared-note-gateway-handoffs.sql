begin;
select plan(35);

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
    select count(*) = 8
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
    ),
  'Handoffs constrain secrets, audience, version, TTL, and four per-share slots'
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
        'gateway_claim_session_share_handoff'
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
        'gateway_claim_session_share_handoff'
      )
  )
    and not has_function_privilege(
      'service_role',
      'private.issue_session_share_handoff(uuid,text,uuid,bigint)',
      'EXECUTE'
    ),
  'Service role reaches hardened gateway implementations but not issuance internals'
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
      'private.issue_session_share_handoff(uuid,text,uuid,bigint)'::regprocedure
    )) not like '%delete from private.session_share_handoffs%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint)'::regprocedure
      )) like '%on conflict (share_id, slot) do update%'
      and lower(pg_get_functiondef(
        'private.issue_session_share_handoff(uuid,text,uuid,bigint)'::regprocedure
      )) like '%pg_advisory_xact_lock%'
  )
    and lower(pg_get_functiondef(
      'private.gateway_create_session_share_link_handoff(uuid,text)'::regprocedure
    )) not like '%for update%'
    and lower(pg_get_functiondef(
      'private.gateway_create_public_session_share_handoff(text)'::regprocedure
    )) not like '%for update%'
    and lower(pg_get_functiondef(
      'private.gateway_claim_session_share_handoff(text)'::regprocedure
    )) not like '%for share%'
    and lower(pg_get_functiondef(
      'private.gateway_claim_session_share_handoff(text)'::regprocedure
    )) not like '%request_hash in%',
  'Issuance and claim avoid cross-row cleanup and share-to-handoff lock inversion'
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
    ),
  'Gateway inputs are length-bounded and canonical before lookup or hashing'
);

select lives_ok(
  $query$
    insert into shared_note_gateway_test_state (name, share_id, secret, expires_at)
    select 'public_handoff',
      (select share_id from shared_note_gateway_test_state where name = 'public_share'),
      request_id,
      expires_at
    from public.gateway_create_public_session_share_handoff(
      (select slug from shared_note_gateway_test_state where name = 'public_share')
    )
  $query$,
  'The gateway can create a public one-time handoff'
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
  'Handoff capabilities are stored only as SHA-256 digests'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select title
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_handoff')
    )
  $$,
  array['Public gateway snapshot'::text],
  'A valid public handoff atomically returns the latest snapshot'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_handoff')
    )
  $$,
  array[0::bigint],
  'A claimed public handoff cannot be replayed'
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
      (select secret from shared_note_gateway_test_state where name = 'active_link')
    )
  $query$,
  'The gateway can create a bearer-link one-time handoff'
);

select results_eq(
  $$
    select title
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_handoff')
    )
  $$,
  array['Link gateway snapshot'::text],
  'A valid bearer-link handoff returns the sanitized snapshot'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_handoff')
    )
  $$,
  array[0::bigint],
  'A claimed bearer-link handoff cannot be replayed'
);

select lives_ok(
  $query$
    do $$
    begin
      for handoff_index in 1..6 loop
        perform *
        from public.gateway_create_public_session_share_handoff(
          (select slug from shared_note_gateway_test_state where name = 'public_share')
        );
      end loop;
    end
    $$
  $query$,
  'Repeated creation is accepted without an unbounded per-share queue'
);

select tests.clear_authentication();
reset role;

select results_eq(
  $$
    select count(*), count(distinct slot), min(slot), max(slot)
    from private.session_share_handoffs
    where share_id = (
      select share_id from shared_note_gateway_test_state where name = 'public_share'
    )
  $$,
  $$values (4::bigint, 4::bigint, 0::smallint, 3::smallint)$$,
  'Four database-enforced slots bound handoffs for each share'
);

select lives_ok(
  $$
    update private.session_share_handoffs as handoff
    set
      request_hash = extensions.digest(
        'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
        'sha256'
      ),
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
    from public.gateway_claim_session_share_handoff(
      'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa'
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
  array[0::bigint],
  'Claim removes its expired handoff without scanning unrelated rows'
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
      (select slug from shared_note_gateway_test_state where name = 'public_share')
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
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'public_revocation_handoff')
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
      (select secret from shared_note_gateway_test_state where name = 'active_link')
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
    from public.gateway_claim_session_share_handoff(
      (select secret from shared_note_gateway_test_state where name = 'link_revocation_handoff')
    )
  $$,
  array[0::bigint],
  'Claim revalidates the active link identity and access version'
);

select * from finish();
rollback;
