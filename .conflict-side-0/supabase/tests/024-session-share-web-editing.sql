begin;
select plan(62);

select tests.create_supabase_user('webedit_owner', 'webedit-owner@example.com');
select tests.create_supabase_user('webedit_manager', 'webedit-manager@example.com');
select tests.create_supabase_user('webedit_editor', 'webedit-editor@example.com');
select tests.create_supabase_user('webedit_viewer', 'webedit-viewer@example.com');
select tests.create_supabase_user('webedit_commenter', 'webedit-commenter@example.com');
select tests.create_supabase_user('webedit_revoked', 'webedit-revoked@example.com');
select tests.create_supabase_user('webedit_other', 'webedit-other@example.com');

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('webedit_owner'),
  tests.get_supabase_uid('webedit_manager'),
  tests.get_supabase_uid('webedit_editor'),
  tests.get_supabase_uid('webedit_viewer'),
  tests.get_supabase_uid('webedit_commenter'),
  tests.get_supabase_uid('webedit_revoked'),
  tests.get_supabase_uid('webedit_other')
);

insert into auth.users (
  id,
  raw_user_meta_data,
  raw_app_meta_data,
  is_anonymous,
  created_at,
  updated_at
) values (
  '30000000-0000-4000-8000-000000000001',
  '{"test_identifier":"webedit_anonymous"}'::jsonb,
  '{}'::jsonb,
  true,
  now(),
  now()
);

select tests.authenticate_as_service_role();

insert into public.workspaces (
  id,
  owner_user_id,
  kind,
  name
) values (
  '20000000-0000-4000-8000-000000000001',
  tests.get_supabase_uid('webedit_owner'),
  'shared',
  'Web editing workspace'
);

insert into public.workspace_memberships (
  workspace_id,
  user_id,
  role
) values
  (
    '20000000-0000-4000-8000-000000000001',
    tests.get_supabase_uid('webedit_owner'),
    'owner'
  ),
  (
    '20000000-0000-4000-8000-000000000001',
    tests.get_supabase_uid('webedit_manager'),
    'admin'
  );

insert into public.session_shares (
  id,
  workspace_id,
  session_id,
  created_by_user_id,
  general_scope
) values
  (
    '20000000-0000-4000-8000-000000000101',
    '20000000-0000-4000-8000-000000000001',
    'web-edit-main',
    tests.get_supabase_uid('webedit_owner'),
    'link'
  ),
  (
    '20000000-0000-4000-8000-000000000102',
    '20000000-0000-4000-8000-000000000001',
    'web-edit-bootstrap',
    tests.get_supabase_uid('webedit_owner'),
    'restricted'
  ),
  (
    '20000000-0000-4000-8000-000000000120',
    '20000000-0000-4000-8000-000000000001',
    'web-edit-legacy',
    tests.get_supabase_uid('webedit_owner'),
    'restricted'
  );

insert into public.session_share_links (
  id,
  share_id,
  token_hash,
  created_by_user_id
) values (
  '20000000-0000-4000-8000-000000000111',
  '20000000-0000-4000-8000-000000000101',
  extensions.digest(repeat('l', 43), 'sha256'),
  tests.get_supabase_uid('webedit_owner')
);

insert into public.session_share_snapshots (
  share_id,
  content_revision,
  title,
  body_json,
  published_by_user_id,
  web_editable
) values (
  '20000000-0000-4000-8000-000000000101',
  5,
  'Base title',
  '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Base body"}]}]}'::jsonb,
  tests.get_supabase_uid('webedit_owner'),
  true
);

insert into public.session_share_attachment_objects (
  id,
  share_id,
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  filename,
  content_type,
  size_bytes,
  sha256,
  state,
  reservation_expires_at,
  cleanup_not_before,
  finalized_at,
  created_at,
  updated_at
) values
  (
    '20000000-0000-4000-8000-000000000201',
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_owner'),
    repeat('a', 43),
    repeat('b', 43),
    tests.get_supabase_uid('webedit_owner')::text
      || '/20000000-0000-4000-8000-000000000101/'
      || '20000000-0000-4000-8000-000000000201.sna1',
    'one.png',
    'image/png',
    10,
    repeat('a', 64),
    'ready',
    now() + interval '15 minutes',
    now() + interval '2 days',
    now(),
    now(),
    now()
  ),
  (
    '20000000-0000-4000-8000-000000000202',
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_owner'),
    repeat('c', 43),
    repeat('d', 43),
    tests.get_supabase_uid('webedit_owner')::text
      || '/20000000-0000-4000-8000-000000000101/'
      || '20000000-0000-4000-8000-000000000202.sna1',
    'two.png',
    'image/png',
    20,
    repeat('b', 64),
    'ready',
    now() + interval '15 minutes',
    now() + interval '2 days',
    now(),
    now(),
    now()
  );

insert into public.session_share_snapshot_attachments (
  share_id,
  attachment_id,
  position
) values (
  '20000000-0000-4000-8000-000000000101',
  '20000000-0000-4000-8000-000000000201',
  0
);

insert into public.session_access_grants (
  share_id,
  grantee_user_id,
  capability,
  granted_by_user_id,
  revoked_by_user_id,
  revoked_at
) values
  (
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_editor'),
    'editor',
    tests.get_supabase_uid('webedit_owner'),
    null,
    null
  ),
  (
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_viewer'),
    'viewer',
    tests.get_supabase_uid('webedit_owner'),
    null,
    null
  ),
  (
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_commenter'),
    'commenter',
    tests.get_supabase_uid('webedit_owner'),
    null,
    null
  ),
  (
    '20000000-0000-4000-8000-000000000101',
    tests.get_supabase_uid('webedit_revoked'),
    'editor',
    tests.get_supabase_uid('webedit_owner'),
    tests.get_supabase_uid('webedit_owner'),
    now()
  ),
  (
    '20000000-0000-4000-8000-000000000102',
    tests.get_supabase_uid('webedit_editor'),
    'editor',
    tests.get_supabase_uid('webedit_owner'),
    null,
    null
  );

select tests.clear_authentication();
reset role;

select ok(
  (
    select count(*) = 4
    from information_schema.columns
    where table_schema = 'public'
      and table_name = 'session_share_snapshots'
      and column_name in (
        'web_editable',
        'last_mutation_id',
        'last_mutation_base_revision',
        'last_mutation_fingerprint'
      )
  )
    and exists (
      select 1
      from pg_constraint
      where conrelid = 'public.session_share_snapshots'::regclass
        and conname = 'session_share_snapshots_last_mutation_check'
    ),
  'Snapshots carry web-edit and idempotency state'
);

select ok(
  (
    select class.relrowsecurity
    from pg_class as class
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname = 'session_share_pending_web_edits'
  ),
  'Pending web edit state has row-level security enabled'
);

select ok(
  not exists (
    select 1
    from information_schema.table_privileges as privilege
    where privilege.table_schema = 'public'
      and privilege.table_name = 'session_share_pending_web_edits'
      and privilege.grantee in ('PUBLIC', 'anon', 'authenticated')
  )
    and has_table_privilege(
      'service_role',
      'public.session_share_pending_web_edits',
      'SELECT, INSERT, UPDATE, DELETE'
    ),
  'Pending web edit state is closed to clients'
);

select ok(
  (
    select count(*) = 5
    from pg_constraint
    where conrelid = 'public.session_share_pending_web_edits'::regclass
      and conname in (
        'session_share_pending_web_edits_pkey',
        'session_share_pending_web_edits_share_id_fkey',
        'session_share_pending_web_edits_revision_check',
        'session_share_pending_web_edits_title_check',
        'session_share_pending_web_edits_body_check'
      )
  )
    and exists (
      select 1
      from pg_constraint
      where conrelid = 'public.session_share_pending_web_edits'::regclass
        and conname = 'session_share_pending_web_edits_share_id_fkey'
        and confdeltype = 'c'
    ),
  'Pending web edits are bounded to one constrained row per live share'
);

select ok(
  (
    select count(*) = 5
      and bool_and(not proc.prosecdef)
      and bool_and(
        'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'publish_session_share_snapshot_cas',
        'edit_session_share_snapshot_cas',
        'read_my_session_share_snapshot_v2',
        'list_my_session_share_snapshot_page_v2',
        'acknowledge_session_share_web_edits'
      )
  ),
  'Public web editing wrappers are hardened security invokers'
);

select ok(
  (
    select count(*) = 7
      and bool_and(proc.prosecdef)
      and bool_and(
        'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'private'
      and proc.proname in (
        'require_session_share_editor',
        'apply_session_share_snapshot_cas',
        'publish_session_share_snapshot_cas',
        'edit_session_share_snapshot_cas',
        'read_my_session_share_snapshot_v2',
        'list_my_session_share_snapshot_page_v2',
        'acknowledge_session_share_web_edits'
      )
  ),
  'Private web editing functions are hardened security definers'
);

select ok(
  (
    select count(*) = 2
      and bool_and(has_function_privilege('service_role', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('authenticated', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'publish_session_share_snapshot_cas',
        'edit_session_share_snapshot_cas'
      )
  ),
  'Only service code can call explicit-actor snapshot mutations'
);

select ok(
  (
    select count(*) = 3
      and bool_and(has_function_privilege('authenticated', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'read_my_session_share_snapshot_v2',
        'list_my_session_share_snapshot_page_v2',
        'acknowledge_session_share_web_edits'
      )
  ),
  'Authenticated clients can only read and acknowledge through actorless wrappers'
);

select ok(
  not has_function_privilege(
    'authenticated',
    'private.apply_session_share_snapshot_cas(uuid,uuid,bigint,uuid,text,jsonb,uuid[],boolean,boolean)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'service_role',
      'private.apply_session_share_snapshot_cas(uuid,uuid,bigint,uuid,text,jsonb,uuid[],boolean,boolean)',
      'EXECUTE'
    ),
  'The unauthenticated mutation core is not directly executable'
);

select ok(
  obj_description(
    'public.publish_session_share_snapshot(uuid,uuid,text,jsonb)'::regprocedure,
    'pg_proc'
  ) ilike 'Legacy service-only writer%'
    and obj_description(
      'public.publish_session_share_snapshot_with_attachments(uuid,uuid,text,jsonb,uuid[])'::regprocedure,
      'pg_proc'
    ) ilike 'Legacy service-only writer%'
    and not has_function_privilege(
      'authenticated',
      'public.publish_session_share_snapshot(uuid,uuid,text,jsonb)',
      'EXECUTE'
    ),
  'Legacy writers are marked for runtime retirement and remain client-inaccessible'
);

select tests.authenticate_as_service_role();

insert into public.session_shares (
  id,
  workspace_id,
  session_id,
  created_by_user_id,
  general_scope
)
select
  (
    '20000000-0000-4000-8000-'
    || lpad(page_number::text, 12, '0')
  )::uuid,
  '20000000-0000-4000-8000-000000000001'::uuid,
  'web-edit-page-' || page_number,
  tests.get_supabase_uid('webedit_owner'),
  'restricted'
from generate_series(103, 111) as page(page_number);

insert into public.session_share_snapshots (
  share_id,
  content_revision,
  title,
  body_json,
  published_by_user_id,
  web_editable
)
select
  (
    '20000000-0000-4000-8000-'
    || lpad(page_number::text, 12, '0')
  )::uuid,
  1,
  'Page ' || page_number,
  '{"type":"doc"}'::jsonb,
  tests.get_supabase_uid('webedit_owner'),
  true
from generate_series(103, 111) as page(page_number);

select tests.authenticate_as('webedit_owner');

select throws_ok(
  $$select count(*) from public.session_share_pending_web_edits$$,
  '42501',
  'permission denied for table session_share_pending_web_edits',
  'Authenticated clients cannot read pending web edit rows directly'
);

select tests.clear_authentication();
set local role anon;

select throws_ok(
  $$
    select *
    from public.publish_session_share_snapshot_cas(
      '20000000-0000-4000-8000-000000000101',
      '20000000-0000-4000-8000-000000000001',
      5,
      '20000000-0000-4000-8000-000000001000',
      'Blocked',
      '{"type":"doc"}'::jsonb,
      array['20000000-0000-4000-8000-000000000201'::uuid],
      true
    )
  $$,
  '42501',
  'permission denied for function publish_session_share_snapshot_cas',
  'Anonymous callers cannot invoke service snapshot mutations'
);

reset role;
select tests.authenticate_as_service_role();

select results_eq(
  format(
    $sql$
      select content_revision, title
      from public.publish_session_share_snapshot(
        '20000000-0000-4000-8000-000000000120',
        %L::uuid,
        'Legacy one',
        '{"type":"doc"}'::jsonb
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values (1::bigint, 'Legacy one'::text)$$,
  'A pre-CAS legacy client can create a snapshot'
);

select results_eq(
  format(
    $sql$
      select content_revision, title
      from public.publish_session_share_snapshot(
        '20000000-0000-4000-8000-000000000120',
        %L::uuid,
        'Legacy two',
        '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values (2::bigint, 'Legacy two'::text)$$,
  'A pre-CAS legacy client can update its snapshot'
);

select results_eq(
  $$
    select
      web_editable,
      last_mutation_id is null,
      last_mutation_base_revision is null,
      last_mutation_fingerprint is null
    from public.session_share_snapshots
    where share_id = '20000000-0000-4000-8000-000000000120'
  $$,
  $$values (false, true, true, true)$$,
  'Legacy writes stay view-only and do not enter CAS mode'
);

delete from public.session_shares
where id = '20000000-0000-4000-8000-000000000120';

select throws_ok(
  $$
    update public.session_share_snapshots
    set
      last_mutation_id = '20000000-0000-4000-8000-000000001099',
      last_mutation_base_revision = null,
      last_mutation_fingerprint = decode(repeat('ab', 32), 'hex')
    where share_id = '20000000-0000-4000-8000-000000000101'
  $$,
  '23514',
  'new row for relation "session_share_snapshots" violates check constraint "session_share_snapshots_last_mutation_check"',
  'Snapshot mutation state rejects a missing base revision'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001001',
        'Viewer edit',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_viewer')
  ),
  '42501',
  'session snapshot edit not permitted',
  'A named Viewer cannot edit snapshots'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001002',
        'Commenter edit',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_commenter')
  ),
  '42501',
  'session snapshot edit not permitted',
  'A named Commenter cannot edit snapshots'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001003',
        'Revoked edit',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_revoked')
  ),
  '42501',
  'session snapshot edit not permitted',
  'A revoked Editor cannot edit snapshots'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001004',
        'Link edit',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_other')
  ),
  '42501',
  'session snapshot edit not permitted',
  'Bearer-link viewing does not grant edit access'
);

update public.session_shares
set general_scope = 'public'
where id = '20000000-0000-4000-8000-000000000101';

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001005',
        'Public edit',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_other')
  ),
  '42501',
  'session snapshot edit not permitted',
  'Public viewing does not grant edit access'
);

select throws_ok(
  $$
    select * from public.edit_session_share_snapshot_cas(
      '20000000-0000-4000-8000-000000000101',
      '30000000-0000-4000-8000-000000000001',
      5,
      '20000000-0000-4000-8000-000000001006',
      'Anonymous edit',
      '{"type":"doc"}'::jsonb,
      array['20000000-0000-4000-8000-000000000201'::uuid]
    )
  $$,
  '42501',
  'session snapshot edit not permitted',
  'An anonymous account cannot edit snapshots'
);

select throws_ok(
  format(
    $sql$
      select * from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        0,
        '20000000-0000-4000-8000-000000001007',
        'Editor bootstrap',
        '{"type":"doc"}'::jsonb,
        array[]::uuid[],
        true
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '42501',
  'session attachment operation not permitted',
  'An Editor cannot use the manager publication path'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        0,
        '20000000-0000-4000-8000-000000001008',
        'Editor bootstrap',
        '{"type":"doc"}'::jsonb,
        array[]::uuid[]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '42501',
  'session snapshot edit not permitted',
  'An Editor cannot bootstrap a missing snapshot'
);

select throws_ok(
  format(
    $sql$
      select * from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        1,
        '20000000-0000-4000-8000-000000001019',
        'Missing manager base',
        '{"type":"doc"}'::jsonb,
        array[]::uuid[],
        false
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  '40001',
  'session share snapshot is unavailable',
  'A nonzero manager base cannot target a missing snapshot'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision, title, web_editable
      from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        0,
        '20000000-0000-4000-8000-000000001009',
        'Manager bootstrap',
        '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb,
        array[]::uuid[],
        false
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('applied'::text, 1::bigint, 'Manager bootstrap'::text, false)$$,
  'A manager can bootstrap a snapshot without a Pro entitlement'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision
      from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        0,
        '20000000-0000-4000-8000-000000001009',
        'Manager bootstrap',
        '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb,
        array[]::uuid[],
        false
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('replayed'::text, 1::bigint)$$,
  'An exact bootstrap retry is replayed idempotently'
);

select results_eq(
  $$
    select count(*), max(new_value)
    from public.session_access_events
    where share_id = '20000000-0000-4000-8000-000000000102'
      and event_type = 'snapshot_published'
  $$,
  $$values (1::bigint, '1'::text)$$,
  'Bootstrap replay does not add a revision or audit event'
);

select throws_ok(
  format(
    $sql$
      select * from public.publish_session_share_snapshot(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        'Legacy overwrite',
        '{"type":"doc"}'::jsonb
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  '23514',
  'new row for relation "session_share_snapshots" violates check constraint "session_share_snapshots_last_mutation_check"',
  'A stale legacy writer cannot advance a snapshot after CAS cutover'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision, title
      from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000102',
        %L::uuid,
        0,
        '20000000-0000-4000-8000-000000001010',
        'Stale bootstrap',
        '{"type":"doc"}'::jsonb,
        array[]::uuid[],
        true
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('conflict'::text, 1::bigint, 'Manager bootstrap'::text)$$,
  'A second bootstrap mutation conflicts with the committed snapshot'
);

select results_eq(
  format(
    $sql$
      select
        outcome,
        content_revision,
        title,
        jsonb_array_length(attachments_json),
        attachments_json -> 0 ->> 'id',
        web_editable
      from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001011',
        'Web revision six',
        '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Web six"}]}]}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  $$
    values (
      'applied'::text,
      6::bigint,
      'Web revision six'::text,
      1,
      '20000000-0000-4000-8000-000000000201'::text,
      true
    )
  $$,
  'An explicit Editor applies a web edit and preserves attachments without Pro'
);

select results_eq(
  $$
    select
      base_content_revision,
      base_title,
      base_content_revision < snapshot.content_revision,
      pending.created_at = pending.updated_at
    from public.session_share_pending_web_edits as pending
    join public.session_share_snapshots as snapshot
      on snapshot.share_id = pending.share_id
    where pending.share_id = '20000000-0000-4000-8000-000000000101'
  $$,
  $$values (5::bigint, 'Base title'::text, true, true)$$,
  'The first web edit captures a bounded base older than the current snapshot'
);

select results_eq(
  $$
    select
      count(*),
      max(related_entity_id::text)::uuid,
      max(previous_value),
      max(new_value)
    from public.session_access_events
    where share_id = '20000000-0000-4000-8000-000000000101'
      and event_type = 'snapshot_published'
  $$,
  $$
    values (
      1::bigint,
      '20000000-0000-4000-8000-000000001011'::uuid,
      '5'::text,
      '6'::text
    )
  $$,
  'An applied web edit writes exactly one revision audit event'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision
      from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001011',
        'Web revision six',
        '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Web six"}]}]}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  $$values ('replayed'::text, 6::bigint)$$,
  'An exact web retry is replayed idempotently'
);

select results_eq(
  $$
    select
      snapshot.content_revision,
      count(event.id),
      bool_and(pending.created_at = pending.updated_at)
    from public.session_share_snapshots as snapshot
    join public.session_share_pending_web_edits as pending
      on pending.share_id = snapshot.share_id
    left join public.session_access_events as event
      on event.share_id = snapshot.share_id
      and event.event_type = 'snapshot_published'
    where snapshot.share_id = '20000000-0000-4000-8000-000000000101'
    group by snapshot.content_revision
  $$,
  $$values (6::bigint, 1::bigint, true)$$,
  'A replay has no snapshot, pending-base, or audit side effects'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001011',
        'Mutation reuse',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '22023',
  'session share mutation id is invalid',
  'A mutation ID cannot be reused with different content'
);

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        6,
        '20000000-0000-4000-8000-000000001012',
        'Drop attachment',
        '{"type":"doc"}'::jsonb,
        array[]::uuid[]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '22023',
  'web snapshot attachments must be preserved',
  'Web edits cannot change the ordered attachment manifest'
);

select results_eq(
  $$
    select
      snapshot.content_revision,
      pending.base_content_revision,
      count(binding.attachment_id),
      min(attachment.state),
      count(event.id)
    from public.session_share_snapshots as snapshot
    join public.session_share_pending_web_edits as pending
      on pending.share_id = snapshot.share_id
    join public.session_share_snapshot_attachments as binding
      on binding.share_id = snapshot.share_id
    join public.session_share_attachment_objects as attachment
      on attachment.id = binding.attachment_id
    left join public.session_access_events as event
      on event.share_id = snapshot.share_id
      and event.event_type = 'snapshot_published'
    where snapshot.share_id = '20000000-0000-4000-8000-000000000101'
    group by snapshot.content_revision, pending.base_content_revision
  $$,
  $$values (6::bigint, 5::bigint, 1::bigint, 'ready'::text, 1::bigint)$$,
  'Rejected mutations leave snapshot, pending base, attachments, and audit intact'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision, title
      from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        5,
        '20000000-0000-4000-8000-000000001013',
        'Stale desktop',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid],
        true
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('conflict'::text, 6::bigint, 'Web revision six'::text)$$,
  'A stale desktop publish conflicts after a web edit'
);

select results_eq(
  $$
    select snapshot.content_revision, pending.base_content_revision, count(event.id)
    from public.session_share_snapshots as snapshot
    join public.session_share_pending_web_edits as pending
      on pending.share_id = snapshot.share_id
    left join public.session_access_events as event
      on event.share_id = snapshot.share_id
      and event.event_type = 'snapshot_published'
    where snapshot.share_id = '20000000-0000-4000-8000-000000000101'
    group by snapshot.content_revision, pending.base_content_revision
  $$,
  $$values (6::bigint, 5::bigint, 1::bigint)$$,
  'A stale desktop conflict has no side effects'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision, title, web_editable
      from public.publish_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        6,
        '20000000-0000-4000-8000-000000001014',
        'Desktop revision seven',
        '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Desktop seven"}]}]}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid],
        true
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('applied'::text, 7::bigint, 'Desktop revision seven'::text, true)$$,
  'A manager applies the reconciled desktop revision without Pro'
);

select results_eq(
  $$
    select
      count(pending.share_id),
      count(event.id),
      max(event.new_value)
    from public.session_share_snapshots as snapshot
    left join public.session_share_pending_web_edits as pending
      on pending.share_id = snapshot.share_id
    left join public.session_access_events as event
      on event.share_id = snapshot.share_id
      and event.event_type = 'snapshot_published'
    where snapshot.share_id = '20000000-0000-4000-8000-000000000101'
  $$,
  $$values (0::bigint, 2::bigint, '7'::text)$$,
  'An applied manager publish clears pending web edits and audits once'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision, title
      from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        6,
        '20000000-0000-4000-8000-000000001015',
        'Stale web',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  $$values ('conflict'::text, 7::bigint, 'Desktop revision seven'::text)$$,
  'A stale web save conflicts after a desktop publish'
);

select results_eq(
  $$
    select snapshot.content_revision, count(pending.share_id), count(event.id)
    from public.session_share_snapshots as snapshot
    left join public.session_share_pending_web_edits as pending
      on pending.share_id = snapshot.share_id
    left join public.session_access_events as event
      on event.share_id = snapshot.share_id
      and event.event_type = 'snapshot_published'
    where snapshot.share_id = '20000000-0000-4000-8000-000000000101'
    group by snapshot.content_revision
  $$,
  $$values (7::bigint, 0::bigint, 2::bigint)$$,
  'A stale web conflict has no snapshot, pending, or audit side effects'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision
      from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        7,
        '20000000-0000-4000-8000-000000001016',
        'Web revision eight',
        '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Web eight"}]}]}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  $$values ('applied'::text, 8::bigint)$$,
  'An Editor can start a new pending web-edit sequence'
);

select results_eq(
  format(
    $sql$
      select outcome, content_revision
      from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        8,
        '20000000-0000-4000-8000-000000001017',
        'Web revision nine',
        '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Web nine"}]}]}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_manager')
  ),
  $$values ('applied'::text, 9::bigint)$$,
  'A manager also has effective Editor access on the web path'
);

select results_eq(
  $$
    select
      pending.base_content_revision,
      pending.base_title,
      pending.base_content_revision < snapshot.content_revision,
      pending.updated_at >= pending.created_at
    from public.session_share_pending_web_edits as pending
    join public.session_share_snapshots as snapshot
      on snapshot.share_id = pending.share_id
    where pending.share_id = '20000000-0000-4000-8000-000000000101'
  $$,
  $$values (7::bigint, 'Desktop revision seven'::text, true, true)$$,
  'Later web edits collapse into the first pending base'
);

select tests.authenticate_as('webedit_owner');

select results_eq(
  $$
    select
      content_revision,
      web_edit_base_content_revision,
      web_edit_base_title,
      web_edit_base_content_revision < content_revision,
      jsonb_array_length(attachments_json),
      manage_access
    from public.read_my_session_share_snapshot_v2(
      '20000000-0000-4000-8000-000000000101'
    )
  $$,
  $$
    values (
      9::bigint,
      7::bigint,
      'Desktop revision seven'::text,
      true,
      1,
      true
    )
  $$,
  'A manager reads the current snapshot and pending reconciliation base'
);

select tests.authenticate_as('webedit_editor');

select results_eq(
  $$
    select
      content_revision,
      web_editable,
      capability,
      manage_access,
      web_edit_base_content_revision is null,
      web_edit_base_title is null,
      web_edit_base_body_json is null,
      pending_created_at is null,
      pending_updated_at is null
    from public.read_my_session_share_snapshot_v2(
      '20000000-0000-4000-8000-000000000101'
    )
  $$,
  $$values (9::bigint, true, 'editor'::text, false, true, true, true, true, true)$$,
  'Non-managing Editors cannot read the pending manager reconciliation base'
);

select tests.authenticate_as_service_role();

delete from public.session_share_snapshots
where share_id = '20000000-0000-4000-8000-000000000102';

select tests.authenticate_as('webedit_owner');

select throws_ok(
  $$
    select *
    from public.list_my_session_share_snapshot_page_v2(null, 9)
  $$,
  '22023',
  'invalid session share snapshot page limit',
  'The public v2 feed rejects page sizes above the response bound'
);

select throws_ok(
  $$
    select *
    from private.list_my_session_share_snapshot_page_v2(null, 9)
  $$,
  '22023',
  'invalid session share snapshot page limit',
  'The private v2 feed rejects page sizes above the response bound'
);

select results_eq(
  $$
    select content_revision, web_edit_base_content_revision, web_edit_base_title
    from public.list_my_session_share_snapshot_page_v2(null, 8)
    where share_id = '20000000-0000-4000-8000-000000000101'
  $$,
  $$values (9::bigint, 7::bigint, 'Desktop revision seven'::text)$$,
  'The paginated v2 feed carries the manager reconciliation base'
);

select results_eq(
  $$
    select share_id
    from public.list_my_session_share_snapshot_page_v2(null, 8)
  $$,
  $$
    values
      ('20000000-0000-4000-8000-000000000101'::uuid),
      ('20000000-0000-4000-8000-000000000103'::uuid),
      ('20000000-0000-4000-8000-000000000104'::uuid),
      ('20000000-0000-4000-8000-000000000105'::uuid),
      ('20000000-0000-4000-8000-000000000106'::uuid),
      ('20000000-0000-4000-8000-000000000107'::uuid),
      ('20000000-0000-4000-8000-000000000108'::uuid),
      ('20000000-0000-4000-8000-000000000109'::uuid)
  $$,
  'An accessible share without a snapshot does not shorten the first v2 page'
);

select results_eq(
  $$
    select share_id
    from public.list_my_session_share_snapshot_page_v2(
      '20000000-0000-4000-8000-000000000109',
      8
    )
  $$,
  $$
    values
      ('20000000-0000-4000-8000-000000000110'::uuid),
      ('20000000-0000-4000-8000-000000000111'::uuid)
  $$,
  'The next v2 page starts after the last returned snapshot-backed share'
);

select throws_ok(
  $$
    select *
    from public.acknowledge_session_share_web_edits(
      '20000000-0000-4000-8000-000000000101',
      8
    )
  $$,
  '40001',
  'web edit acknowledgement conflicts',
  'A stale manager acknowledgement cannot clear pending web edits'
);

select results_eq(
  $$
    select share_id, acknowledged_content_revision, was_pending
    from public.acknowledge_session_share_web_edits(
      '20000000-0000-4000-8000-000000000101',
      9
    )
  $$,
  $$
    values (
      '20000000-0000-4000-8000-000000000101'::uuid,
      9::bigint,
      true
    )
  $$,
  'A current manager acknowledgement clears the pending base without editing content'
);

select results_eq(
  $$
    select acknowledged_content_revision, was_pending
    from public.acknowledge_session_share_web_edits(
      '20000000-0000-4000-8000-000000000101',
      9
    )
  $$,
  $$values (9::bigint, false)$$,
  'A repeated current acknowledgement is idempotent'
);

select tests.authenticate_as_service_role();

select results_eq(
  $$
    select access_version
    from public.session_shares
    where id = '20000000-0000-4000-8000-000000000101'
  $$,
  $$values (1::bigint)$$,
  'Content revisions do not change the access-version epoch'
);

insert into private.account_deletion_jobs (owner_user_id)
values (tests.get_supabase_uid('webedit_editor'));

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        9,
        '20000000-0000-4000-8000-000000001020',
        'Deletion pending',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '42501',
  'session snapshot edit not permitted',
  'A deletion-pending Editor cannot mutate a shared note'
);

delete from private.account_deletion_jobs
where owner_user_id = tests.get_supabase_uid('webedit_editor');

update public.session_access_grants
set
  revoked_by_user_id = tests.get_supabase_uid('webedit_owner'),
  revoked_at = now(),
  updated_at = now()
where share_id = '20000000-0000-4000-8000-000000000101'
  and grantee_user_id = tests.get_supabase_uid('webedit_editor');

update public.session_shares
set
  access_version = access_version + 1,
  updated_at = now()
where id = '20000000-0000-4000-8000-000000000101';

select throws_ok(
  format(
    $sql$
      select * from public.edit_session_share_snapshot_cas(
        '20000000-0000-4000-8000-000000000101',
        %L::uuid,
        9,
        '20000000-0000-4000-8000-000000001018',
        'Revoked after editing',
        '{"type":"doc"}'::jsonb,
        array['20000000-0000-4000-8000-000000000201'::uuid]
      )
    $sql$,
    tests.get_supabase_uid('webedit_editor')
  ),
  '42501',
  'session snapshot edit not permitted',
  'An active Editor loses write access immediately after revocation'
);

select results_eq(
  $$
    select count(*), min(previous_value), max(new_value)
    from public.session_access_events
    where share_id = '20000000-0000-4000-8000-000000000101'
      and event_type = 'snapshot_published'
  $$,
  $$values (4::bigint, '5'::text, '9'::text)$$,
  'Only four applied main-share writes produced audit events'
);

select tests.authenticate_as('webedit_owner');

select results_eq(
  $$
    select
      content_revision,
      access_version,
      jsonb_array_length(attachments_json),
      web_edit_base_content_revision is null,
      web_edit_base_title is null,
      web_edit_base_body_json is null
    from public.read_my_session_share_snapshot_v2(
      '20000000-0000-4000-8000-000000000101'
    )
  $$,
  $$values (9::bigint, 2::bigint, 1, true, true, true)$$,
  'Acknowledgement leaves the latest attachment-bearing snapshot and clears only pending state'
);

select * from finish();
rollback;
