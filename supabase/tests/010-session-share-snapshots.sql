begin;
select plan(55);

select tests.create_supabase_user('snapshot_owner', 'snapshot-owner@example.com');
select tests.create_supabase_user('snapshot_admin', 'snapshot-admin@example.com');
select tests.create_supabase_user('snapshot_named', 'snapshot-named@example.com');
select tests.create_supabase_user(
  'snapshot_workspace_member',
  'snapshot-workspace-member@example.com'
);
select tests.create_supabase_user(
  'snapshot_pending_invitee',
  'snapshot-pending-invitee@example.com'
);
select tests.create_supabase_user(
  'snapshot_pending_requester',
  'snapshot-pending-requester@example.com'
);
select tests.create_supabase_user('snapshot_other', 'snapshot-other@example.com');

create temporary table session_snapshot_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid,
  secret text,
  slug text
);

grant all on session_snapshot_test_state to anon, authenticated, service_role;

insert into session_snapshot_test_state (name, workspace_id)
values
  ('source_workspace', gen_random_uuid()),
  ('target_workspace', gen_random_uuid()),
  ('anonymous_user', gen_random_uuid());

insert into auth.users (
  id,
  raw_user_meta_data,
  raw_app_meta_data,
  is_anonymous,
  created_at,
  updated_at
)
select
  workspace_id,
  jsonb_build_object('test_identifier', 'snapshot_anonymous'),
  '{}'::jsonb,
  true,
  now(),
  now()
from session_snapshot_test_state
where name = 'anonymous_user';

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('snapshot_owner'),
  tests.get_supabase_uid('snapshot_admin'),
  tests.get_supabase_uid('snapshot_named'),
  tests.get_supabase_uid('snapshot_workspace_member'),
  tests.get_supabase_uid('snapshot_pending_invitee'),
  tests.get_supabase_uid('snapshot_pending_requester'),
  tests.get_supabase_uid('snapshot_other')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('snapshot_owner'),
  'shared',
  'Snapshot source workspace'
from session_snapshot_test_state
where name = 'source_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('snapshot_owner'),
  'shared',
  'Snapshot target workspace'
from session_snapshot_test_state
where name = 'target_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('snapshot_owner'),
  'owner'
from session_snapshot_test_state
where name = 'source_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('snapshot_admin'),
  'admin'
from session_snapshot_test_state
where name = 'source_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('snapshot_owner'),
  'owner'
from session_snapshot_test_state
where name = 'target_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('snapshot_workspace_member'),
  'member'
from session_snapshot_test_state
where name = 'target_workspace';

select tests.clear_authentication();
reset role;

select ok(
  (
    select relrowsecurity
    from pg_class as class
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname = 'session_share_snapshots'
  ),
  'The snapshot table has row-level security enabled'
);

select ok(
  not exists (
    select 1
    from information_schema.table_privileges as privilege
    where privilege.table_schema = 'public'
      and privilege.table_name = 'session_share_snapshots'
      and privilege.grantee in ('PUBLIC', 'anon', 'authenticated')
  )
    and has_table_privilege(
      'service_role',
      'public.session_share_snapshots',
      'SELECT, INSERT, UPDATE, DELETE'
    ),
  'Only trusted service code has direct snapshot table privileges'
);

select ok(
  (
    select count(*) = 1
      and bool_and(policyname = 'session_share_snapshots_service_all')
      and bool_and(roles = array['service_role'::name])
      and bool_and(cmd = 'ALL')
      and bool_and(qual = 'true')
      and bool_and(with_check = 'true')
    from pg_policies
    where schemaname = 'public'
      and tablename = 'session_share_snapshots'
  ),
  'The snapshot table policy is restricted to service role'
);

select ok(
  (
    select count(*) = 6
      and bool_and(
        has_function_privilege('service_role', proc.oid, 'EXECUTE')
          = (proc.proname = 'publish_session_share_snapshot')
      )
      and bool_and(
        has_function_privilege('authenticated', proc.oid, 'EXECUTE')
          = (proc.proname in (
            'read_my_session_share_snapshot',
            'list_my_session_share_snapshots',
            'list_my_session_share_snapshot_page'
          ))
      )
      and bool_and(
        not has_function_privilege('anon', proc.oid, 'EXECUTE')
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'publish_session_share_snapshot',
        'read_my_session_share_snapshot',
        'read_session_share_link_snapshot',
        'read_public_session_share_snapshot',
        'list_my_session_share_snapshots',
        'list_my_session_share_snapshot_page'
      )
  ),
  'Snapshot RPC grants keep general-access reads behind the service gateway'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'publish_session_share_snapshot',
        'read_my_session_share_snapshot',
        'read_session_share_link_snapshot',
        'read_public_session_share_snapshot',
        'list_my_session_share_snapshots',
        'list_my_session_share_snapshot_page'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  )
    and not exists (
      select 1
      from pg_proc as proc
      join pg_namespace as namespace
        on namespace.oid = proc.pronamespace
      where namespace.nspname = 'private'
        and proc.proname in (
          'publish_session_share_snapshot',
          'read_my_session_share_snapshot',
          'read_session_share_link_snapshot',
          'read_public_session_share_snapshot',
          'list_my_session_share_snapshots'
        )
        and (
          not proc.prosecdef
          or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
        )
    ),
  'Public wrappers are invokers and private implementations are hardened definers'
);

select ok(
  (
    select count(*) = 4
    from pg_constraint as constraint_record
    join pg_class as class
      on class.oid = constraint_record.conrelid
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname = 'session_share_snapshots'
      and constraint_record.conname in (
        'session_share_snapshots_schema_version_check',
        'session_share_snapshots_content_revision_check',
        'session_share_snapshots_title_check',
        'session_share_snapshots_body_check'
      )
  ),
  'Snapshot rows constrain schema, revision, title, and document shape'
);

select tests.authenticate_as_hyprnote_pro('snapshot_owner');

select throws_ok(
  $$select count(*) from public.session_share_snapshots$$,
  '42501',
  'permission denied for table session_share_snapshots',
  'Authenticated clients cannot bypass snapshot RPCs with direct reads'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, slug)
    select 'restricted_share', share_id, public_slug
    from public.create_session_share(
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'source_workspace'
      ),
      'snapshot-restricted-session'
    )
  $query$,
  'A manager can create the restricted snapshot fixture'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, slug)
    select 'workspace_share', share_id, public_slug
    from public.create_session_share(
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'source_workspace'
      ),
      'snapshot-workspace-session'
    )
  $query$,
  'A manager can create the workspace snapshot fixture'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, slug)
    select 'link_share', share_id, public_slug
    from public.create_session_share(
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'source_workspace'
      ),
      'snapshot-link-session'
    )
  $query$,
  'A manager can create the link snapshot fixture'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, slug)
    select 'public_share', share_id, public_slug
    from public.create_session_share(
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'source_workspace'
      ),
      'snapshot-public-session'
    )
  $query$,
  'A manager can create the public snapshot fixture'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'workspace_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'target_workspace'
      )
    )
  $$,
  'The workspace fixture grants inherited Viewer access'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, entity_id, secret)
    select 'active_link', share_id, link_id, link_token
    from public.enable_session_share_link(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'link_share'
      )
    )
  $query$,
  'The link fixture has an active bearer token'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'public_share'
      ),
      'public'
    )
  $$,
  'The public fixture is discoverable by slug'
);

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, entity_id, secret)
    select
      'pending_invitation',
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      invitation_id,
      invite_token
    from public.create_session_access_invitation(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      'snapshot-pending-invitee@example.com',
      'viewer'
    )
  $query$,
  'A pending invitation exists without granting snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_named');

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, entity_id)
    select
      'named_request',
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      request_id
    from public.request_session_access(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      'commenter'
    )
  $query$,
  'The named reader can request Commenter access'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_pending_requester');

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, entity_id)
    select
      'pending_request',
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      request_id
    from public.request_session_access(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      'viewer'
    )
  $query$,
  'A pending request exists without granting snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('snapshot_owner');

select lives_ok(
  $query$
    insert into session_snapshot_test_state (name, share_id, entity_id)
    select
      'named_grant',
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      grant_id
    from public.review_session_access_request(
      (
        select entity_id
        from session_snapshot_test_state
        where name = 'named_request'
      ),
      'approved',
      'commenter'
    )
  $query$,
  'A manager can approve durable named snapshot access'
);

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      auth.uid(),
      'Forbidden direct publication',
      '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
    )
  $query$,
  '42501',
  'permission denied for function publish_session_share_snapshot',
  'Authenticated managers cannot call the service publication RPC directly'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $query$
    select schema_version, content_revision, title, body_json
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      tests.get_supabase_uid('snapshot_owner'),
      '  Restricted snapshot  ',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"First"}]}]}'::jsonb
    )
  $query$,
  $query$
    values (
      1::smallint,
      1::bigint,
      'Restricted snapshot'::text,
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"First"}]}]}'::jsonb
    )
  $query$,
  'The service publishes a normalized revision-one owner snapshot'
);

select results_eq(
  $query$
    select content_revision, title, body_json
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      tests.get_supabase_uid('snapshot_admin'),
      'Restricted snapshot v2',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Second"}]}]}'::jsonb
    )
  $query$,
  $query$
    values (
      2::bigint,
      'Restricted snapshot v2'::text,
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Second"}]}]}'::jsonb
    )
  $query$,
  'A source workspace admin can publish the next snapshot revision'
);

select results_eq(
  $query$
    select published_by_user_id, content_revision
    from public.session_share_snapshots
    where share_id = (
      select share_id
      from session_snapshot_test_state
      where name = 'restricted_share'
    )
  $query$,
  $$
    values (
      tests.get_supabase_uid('snapshot_admin'),
      2::bigint
    )
  $$,
  'The snapshot records the latest authorized publisher and revision'
);

select results_eq(
  $query$
    select previous_value, new_value
    from public.session_access_events
    where share_id = (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
      and event_type = 'snapshot_published'
    order by new_value::bigint
  $query$,
  $$
    values
      (null::text, '1'::text),
      ('1'::text, '2'::text)
  $$,
  'Every publication records its revision transition in the audit log'
);

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      tests.get_supabase_uid('snapshot_other'),
      'Unauthorized',
      '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
    )
  $query$,
  '42501',
  'session snapshot publication not permitted',
  'The service RPC rechecks the claimed actor against source management'
);

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      tests.get_supabase_uid('snapshot_owner'),
      'Invalid body',
      '{"type":"paragraph"}'::jsonb
    )
  $query$,
  '22023',
  'invalid session share snapshot',
  'Publication rejects payloads that are not document roots'
);

select throws_ok(
  $query$
    update public.session_share_snapshots
    set content_revision = 0
    where share_id = (
      select share_id
      from session_snapshot_test_state
      where name = 'restricted_share'
    )
  $query$,
  '23514',
  'new row for relation "session_share_snapshots" violates check constraint "session_share_snapshots_content_revision_check"',
  'The table rejects nonpositive content revisions'
);

select results_eq(
  $query$
    select schema_version, content_revision, title, body_json
    from public.session_share_snapshots
    where share_id = (
      select share_id
      from session_snapshot_test_state
      where name = 'restricted_share'
    )
  $query$,
  $query$
    values (
      1::smallint,
      2::bigint,
      'Restricted snapshot v2'::text,
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Second"}]}]}'::jsonb
    )
  $query$,
  'Trusted service code sees the complete latest persisted snapshot'
);

select results_eq(
  $query$
    select name, content_revision
    from (
      values
        (
          'workspace_share'::text,
          (
            select content_revision
            from public.publish_session_share_snapshot(
              (
                select share_id
                from session_snapshot_test_state
                where name = 'workspace_share'
              ),
              tests.get_supabase_uid('snapshot_owner'),
              'Workspace snapshot',
              '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
            )
          )
        ),
        (
          'link_share'::text,
          (
            select content_revision
            from public.publish_session_share_snapshot(
              (
                select share_id
                from session_snapshot_test_state
                where name = 'link_share'
              ),
              tests.get_supabase_uid('snapshot_owner'),
              'Link snapshot',
              '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
            )
          )
        ),
        (
          'public_share'::text,
          (
            select content_revision
            from public.publish_session_share_snapshot(
              (
                select share_id
                from session_snapshot_test_state
                where name = 'public_share'
              ),
              tests.get_supabase_uid('snapshot_owner'),
              'Public snapshot',
              '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
            )
          )
        )
    ) as published(name, content_revision)
    order by name
  $query$,
  $$
    values
      ('link_share'::text, 1::bigint),
      ('public_share'::text, 1::bigint),
      ('workspace_share'::text, 1::bigint)
  $$,
  'The remaining access-mode fixtures each receive a first snapshot'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('snapshot_owner');

select results_eq(
  $$
    select session_id, capability, manage_access
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  $$values ('snapshot-restricted-session'::text, 'editor'::text, true)$$,
  'The source owner reads the restricted snapshot as a managing Editor'
);

select results_eq(
  $$
    select count(*)
    from public.list_my_session_share_snapshots()
  $$,
  array[4::bigint],
  'A source manager lists every published source-workspace snapshot'
);

select results_eq(
  $$
    select count(*)
    from public.list_my_session_share_snapshot_page(null::uuid, 2)
  $$,
  array[2::bigint],
  'The durable snapshot page enforces its requested result limit'
);

select results_eq(
  $$
    select count(*)
    from public.list_my_session_share_snapshot_page(
      (
        select share_id
        from public.list_my_session_share_snapshots()
        order by share_id
        limit 1
      ),
      100
    )
  $$,
  array[3::bigint],
  'The durable snapshot page continues after an immutable share cursor'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('snapshot_admin');

select results_eq(
  $$
    select capability, manage_access
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  $$values ('editor'::text, true)$$,
  'A source admin reads snapshots with management authority'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_named');

select results_eq(
  $$
    select title, capability, manage_access
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  $$values ('Restricted snapshot v2'::text, 'commenter'::text, false)$$,
  'An approved named grantee reads the snapshot at the granted capability'
);

select results_eq(
  $$
    select session_id, capability
    from public.list_my_session_share_snapshots()
  $$,
  $$values ('snapshot-restricted-session'::text, 'commenter'::text)$$,
  'A named grant appears in the durable shared-snapshot list'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_workspace_member');

select results_eq(
  $$
    select title, capability, manage_access
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  $$values ('Workspace snapshot'::text, 'viewer'::text, false)$$,
  'A target workspace member reads the inherited Viewer snapshot'
);

select results_eq(
  $$
    select session_id, capability
    from public.list_my_session_share_snapshots()
  $$,
  $$values ('snapshot-workspace-session'::text, 'viewer'::text)$$,
  'Workspace-inherited access appears in the durable shared-snapshot list'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_pending_invitee');

select results_eq(
  $$
    select count(*)
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  array[0::bigint],
  'A pending invitation does not grant snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_pending_requester');

select results_eq(
  $$
    select count(*)
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  array[0::bigint],
  'A pending access request does not grant snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_other');

select results_eq(
  $$
    select count(*)
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  array[0::bigint],
  'An unrelated user cannot read a restricted snapshot'
);

select results_eq(
  $$select count(*) from public.list_my_session_share_snapshots()$$,
  array[0::bigint],
  'A public snapshot is not silently added to an unrelated user durable list'
);

select tests.clear_authentication();
set local role anon;

select throws_ok(
  $$
    select *
    from public.read_session_share_link_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_snapshot_test_state
        where name = 'active_link'
      )
    )
  $$,
  '42501',
  'permission denied for function read_session_share_link_snapshot',
  'Unauthenticated visitors cannot bypass the snapshot gateway'
);

reset role;
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select title
    from public.gateway_read_session_share_link_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_snapshot_test_state
        where name = 'active_link'
      )
    )
  $$,
  array['Link snapshot'::text],
  'The trusted gateway can read a valid bearer-link snapshot'
);

select results_eq(
  $$
    select title
    from public.gateway_read_public_session_share_snapshot(
      (
        select slug
        from session_snapshot_test_state
        where name = 'public_share'
      )
    )
  $$,
  array['Public snapshot'::text],
  'The trusted gateway can read a public snapshot by slug'
);

select results_eq(
  $$select count(*) from public.gateway_read_public_session_share_snapshot('s_00000000000000000000000000000000')$$,
  array[0::bigint],
  'An unknown public slug resolves no snapshot'
);

select tests.clear_authentication();
reset role;
set local role anon;

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      ),
      (
        select workspace_id
        from session_snapshot_test_state
        where name = 'anonymous_user'
      ),
      'Anonymous publication',
      '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
    )
  $query$,
  '42501',
  'permission denied for function publish_session_share_snapshot',
  'Anonymous database callers cannot invoke snapshot publication'
);

reset role;
select tests.authenticate_as('snapshot_anonymous');

select throws_ok(
  $query$
    select *
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $query$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth users cannot use named snapshot reads'
);

select throws_ok(
  $$select * from public.list_my_session_share_snapshots()$$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth users cannot list durable snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('snapshot_owner');

select lives_ok(
  $$
    select *
    from public.revoke_session_access_grant(
      (
        select entity_id
        from session_snapshot_test_state
        where name = 'named_grant'
      )
    )
  $$,
  'A manager can revoke the named snapshot grant'
);

select tests.clear_authentication();
select tests.authenticate_as('snapshot_named');

select results_eq(
  $$
    select count(*)
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  array[0::bigint],
  'Grant revocation immediately removes named snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspace_memberships
set deleted_at = now(), updated_at = now()
where workspace_id = (
    select workspace_id
    from session_snapshot_test_state
    where name = 'target_workspace'
  )
  and user_id = tests.get_supabase_uid('snapshot_workspace_member');

select tests.clear_authentication();
select tests.authenticate_as('snapshot_workspace_member');

select results_eq(
  $$
    select count(*)
    from public.read_my_session_share_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  array[0::bigint],
  'Workspace membership revocation immediately removes inherited snapshot access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('snapshot_owner');

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'link_share'
      ),
      'restricted'
    )
  $$,
  'A manager can revoke bearer-link scope'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'public_share'
      ),
      'restricted'
    )
  $$,
  'A manager can revoke public scope'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.gateway_read_session_share_link_snapshot(
      (
        select share_id
        from session_snapshot_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_snapshot_test_state
        where name = 'active_link'
      )
    )
  $$,
  array[0::bigint],
  'Scope revocation immediately invalidates bearer-link snapshot reads'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_read_public_session_share_snapshot(
      (
        select slug
        from session_snapshot_test_state
        where name = 'public_share'
      )
    )
  $$,
  array[0::bigint],
  'Scope revocation immediately invalidates public snapshot reads'
);

select * from finish();
rollback;
