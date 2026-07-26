begin;
select plan(106);

select tests.create_supabase_user('share_owner', 'share-owner@example.com');
select tests.create_supabase_user('share_recipient', 'share-recipient@example.com');
select tests.create_supabase_user('share_other', 'share-other@example.com');
select tests.create_supabase_user(
  'share_workspace_member',
  'share-workspace-member@example.com'
);
select tests.create_supabase_user(
  'share_request_approved',
  'share-request-approved@example.com'
);
select tests.create_supabase_user(
  'share_request_denied',
  'share-request-denied@example.com'
);
select tests.create_supabase_user(
  'share_request_cancelled',
  'share-request-cancelled@example.com'
);

create temporary table session_sharing_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid,
  related_id uuid,
  secret text,
  slug text,
  capability text,
  access_version bigint,
  was_created boolean
);

grant all on session_sharing_test_state to anon, authenticated, service_role;

insert into session_sharing_test_state (name, workspace_id)
values
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
  jsonb_build_object('test_identifier', 'share_anonymous'),
  '{}'::jsonb,
  true,
  now(),
  now()
from session_sharing_test_state
where name = 'anonymous_user';

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('share_owner'),
  tests.get_supabase_uid('share_recipient'),
  tests.get_supabase_uid('share_other'),
  tests.get_supabase_uid('share_workspace_member'),
  tests.get_supabase_uid('share_request_approved'),
  tests.get_supabase_uid('share_request_denied'),
  tests.get_supabase_uid('share_request_cancelled')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('share_owner'),
  'shared',
  'Session sharing target workspace'
from session_sharing_test_state
where name = 'target_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('share_owner'),
  'owner'
from session_sharing_test_state
where name = 'target_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('share_workspace_member'),
  'member'
from session_sharing_test_state
where name = 'target_workspace'
union all
select
  workspace_id,
  tests.get_supabase_uid('share_recipient'),
  'member'
from session_sharing_test_state
where name = 'target_workspace';

select tests.clear_authentication();
reset role;

select ok(
  (
    select count(*) = 6 and bool_and(class.relrowsecurity)
    from pg_class as class
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname in (
        'session_shares',
        'session_share_links',
        'session_access_grants',
        'session_access_invitations',
        'session_access_requests',
        'session_access_events'
      )
  ),
  'Every session-sharing authority table has row-level security enabled'
);

select ok(
  not exists (
    select 1
    from information_schema.table_privileges as privilege
    where privilege.table_schema = 'public'
      and privilege.table_name in (
        'session_shares',
        'session_share_links',
        'session_access_grants',
        'session_access_invitations',
        'session_access_requests',
        'session_access_events'
      )
      and privilege.grantee in ('anon', 'authenticated')
  )
    and has_table_privilege(
      'service_role',
      'public.session_shares',
      'SELECT'
    )
    and has_table_privilege(
      'service_role',
      'public.session_access_events',
      'INSERT'
    ),
  'Authority rows are hidden from clients and writable by trusted service code'
);

select ok(
  (
    select count(*) = 19
      and bool_and(
        has_function_privilege('authenticated', proc.oid, 'EXECUTE')
          = (proc.proname NOT IN (
            'resolve_session_share_link',
            'resolve_public_session_share'
          ))
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share',
        'get_session_share_management',
        'set_session_share_scope',
        'enable_session_share_link',
        'rotate_session_share_link',
        'create_session_access_invitation',
        'resend_session_access_invitation',
        'accept_session_access_invitation',
        'revoke_session_access_invitation',
        'update_session_access_grant',
        'revoke_session_access_grant',
        'request_session_access',
        'cancel_session_access_request',
        'review_session_access_request',
        'resolve_my_session_access',
        'resolve_session_share_link',
        'resolve_public_session_share',
        'list_my_accessible_sessions',
        'list_session_share_access'
      )
  ),
  'Permanent authenticated users cannot bypass gateway-only general access'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share',
        'get_session_share_management',
        'set_session_share_scope',
        'enable_session_share_link',
        'rotate_session_share_link',
        'create_session_access_invitation',
        'resend_session_access_invitation',
        'accept_session_access_invitation',
        'revoke_session_access_invitation',
        'update_session_access_grant',
        'revoke_session_access_grant',
        'request_session_access',
        'cancel_session_access_request',
        'review_session_access_request',
        'resolve_my_session_access',
        'resolve_session_share_link',
        'resolve_public_session_share',
        'list_my_accessible_sessions',
        'list_session_share_access'
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
          'require_permanent_user',
          'is_session_share_manager',
          'require_session_share_manager',
          'write_session_access_event',
          'create_session_share',
          'get_session_share_management',
          'set_session_share_scope',
          'issue_session_share_link',
          'upsert_session_access_grant',
          'create_session_access_invitation',
          'resend_session_access_invitation',
          'accept_session_access_invitation',
          'revoke_session_access_invitation',
          'update_session_access_grant',
          'revoke_session_access_grant',
          'request_session_access',
          'cancel_session_access_request',
          'review_session_access_request',
          'resolve_my_session_access',
          'resolve_session_share_link',
          'resolve_public_session_share',
          'list_my_accessible_sessions',
          'list_session_share_access'
        )
        and (
          not proc.prosecdef
          or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
        )
    ),
  'Public wrappers are invokers and privileged implementations use empty search paths'
);

select ok(
  (
    select count(*) filter (
      where has_function_privilege('anon', proc.oid, 'EXECUTE')
    ) = 0
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share',
        'get_session_share_management',
        'set_session_share_scope',
        'enable_session_share_link',
        'rotate_session_share_link',
        'create_session_access_invitation',
        'resend_session_access_invitation',
        'accept_session_access_invitation',
        'revoke_session_access_invitation',
        'update_session_access_grant',
        'revoke_session_access_grant',
        'request_session_access',
        'cancel_session_access_request',
        'review_session_access_request',
        'resolve_my_session_access',
        'resolve_session_share_link',
        'resolve_public_session_share',
        'list_my_accessible_sessions',
        'list_session_share_access'
      )
  ),
  'Anonymous callers cannot bypass the shared-note gateway'
);

select tests.authenticate_as_hyprnote_pro('share_owner');

select throws_ok(
  $$select count(*) from public.session_shares$$,
  '42501',
  'permission denied for table session_shares',
  'Authenticated clients cannot bypass the RPCs with direct table reads'
);

select tests.clear_authentication();
select tests.authenticate_as('share_anonymous');

select throws_ok(
  $$
    select *
    from public.create_session_share(
      tests.get_supabase_uid('share_owner'),
      'anonymous-session'
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth users cannot create sharing authority'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      slug,
      capability,
      access_version,
      was_created
    )
    select
      'restricted_share',
      share_id,
      public_slug,
      general_scope,
      access_version,
      was_created
    from public.create_session_share(auth.uid(), '  restricted-session  ')
  $$,
  'A workspace owner can create session sharing authority'
);

select ok(
  (
    select capability = 'restricted'
      and slug ~ '^s_[0-9a-f]{32}$'
      and access_version = 1
      and was_created
    from session_sharing_test_state
    where name = 'restricted_share'
  )
    and (
      select session_id = 'restricted-session'
      from public.get_session_share_management(
        (
          select share_id
          from session_sharing_test_state
          where name = 'restricted_share'
        )
      )
    ),
  'Share creation normalizes the session id and starts restricted at version one'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      slug,
      capability,
      access_version,
      was_created
    )
    select
      'restricted_share_duplicate',
      share_id,
      public_slug,
      general_scope,
      access_version,
      was_created
    from public.create_session_share(auth.uid(), 'restricted-session')
  $$,
  'Creating authority for the same workspace session is idempotent'
);

select ok(
  (
    select duplicate.share_id = original.share_id
      and duplicate.slug = original.slug
      and duplicate.access_version = original.access_version
      and not duplicate.was_created
    from session_sharing_test_state as duplicate
    join session_sharing_test_state as original
      on original.name = 'restricted_share'
    where duplicate.name = 'restricted_share_duplicate'
  ),
  'Idempotent share creation preserves identity, slug, and access version'
);

select throws_ok(
  $$select * from public.create_session_share(auth.uid(), E'bad\nsession')$$,
  '22023',
  'invalid session id',
  'Control characters are rejected from session ids'
);

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  $$values ('editor'::text, true)$$,
  'Source workspace managers always resolve to editor with access management'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select throws_ok(
  $$
    select *
    from public.create_session_share(
      tests.get_supabase_uid('share_owner'),
      'forbidden-session'
    )
  $$,
  '42501',
  'session access operation not permitted',
  'A nonmanager cannot create authority in another workspace'
);

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  array[0::bigint],
  'Restricted shares grant no access to unrelated permanent users'
);

select throws_ok(
  $$
    select *
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_test_state
        where name = 'restricted_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Only source workspace managers can read management state'
);

select tests.clear_authentication();
select tests.authenticate_as('share_anonymous');

select throws_ok(
  $$
    select *
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'restricted_share'
      ),
      'viewer'
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth users cannot request named access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (name, share_id, slug)
    select 'workspace_share', share_id, public_slug
    from public.create_session_share(auth.uid(), 'workspace-session')
  $$,
  'An owner can create an independent workspace-scoped share'
);

select results_eq(
  $$
    select general_scope, general_workspace_id
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_sharing_test_state
        where name = 'target_workspace'
      )
    )
  $$,
  $$
    select 'workspace'::text, workspace_id
    from session_sharing_test_state
    where name = 'target_workspace'
  $$,
  'General workspace access can target a shared workspace the manager belongs to'
);

select tests.clear_authentication();
select tests.authenticate_as('share_workspace_member');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  $$values ('viewer'::text, false)$$,
  'General workspace access is always Viewer for an active member'
);

select results_eq(
  $$select session_id, capability from public.list_my_accessible_sessions()$$,
  $$values ('workspace-session'::text, 'viewer'::text)$$,
  'Workspace-shared sessions appear in a member accessible-session list'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  array[0::bigint],
  'Workspace scope does not grant access to nonmembers'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  $$values ('editor'::text, true)$$,
  'General workspace Viewer access never lowers manager access'
);

select throws_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      ),
      'workspace',
      tests.get_supabase_uid('share_other')
    )
  $$,
  '42501',
  'general workspace not available',
  'General workspace scope rejects unavailable personal workspaces'
);

insert into session_sharing_test_state (
  name,
  share_id,
  access_version
)
select
  'workspace_version_before_revoke',
  share_id,
  access_version
from public.get_session_share_management(
  (
    select share_id
    from session_sharing_test_state
    where name = 'workspace_share'
  )
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspace_memberships
set deleted_at = now(), updated_at = now()
where workspace_id = (
    select workspace_id
    from session_sharing_test_state
    where name = 'target_workspace'
  )
  and user_id = tests.get_supabase_uid('share_workspace_member');

select tests.clear_authentication();
select tests.authenticate_as('share_workspace_member');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  array[0::bigint],
  'Revoking target workspace membership immediately removes inherited access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select current_share.access_version
      = previous_share.access_version + 1
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    ) as current_share
    cross join session_sharing_test_state as previous_share
    where previous_share.name = 'workspace_version_before_revoke'
  $$,
  array[true],
  'Workspace membership revocation advances the affected share version'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspace_memberships
set deleted_at = null, updated_at = now()
where workspace_id = (
    select workspace_id
    from session_sharing_test_state
    where name = 'target_workspace'
  )
  and user_id = tests.get_supabase_uid('share_workspace_member');

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select current_share.access_version
      = previous_share.access_version + 2
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    ) as current_share
    cross join session_sharing_test_state as previous_share
    where previous_share.name = 'workspace_version_before_revoke'
  $$,
  array[true],
  'Workspace membership restoration advances the share version again'
);

select tests.clear_authentication();
select tests.authenticate_as('share_workspace_member');

select results_eq(
  $$
    select capability
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_share'
      )
    )
  $$,
  array['viewer'::text],
  'Restoring target workspace membership restores Viewer access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

insert into session_sharing_test_state (name, workspace_id)
values ('ephemeral_target_workspace', gen_random_uuid());

select tests.clear_authentication();
select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('share_owner'),
  'shared',
  'Ephemeral sharing target'
from session_sharing_test_state
where name = 'ephemeral_target_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('share_owner'),
  'owner'
from session_sharing_test_state
where name = 'ephemeral_target_workspace';

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (name, share_id)
    select 'workspace_cleanup_share', share_id
    from public.create_session_share(auth.uid(), 'workspace-cleanup-session')
  $$,
  'An owner can create a share for target workspace cleanup coverage'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_cleanup_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_sharing_test_state
        where name = 'ephemeral_target_workspace'
      )
    )
  $$,
  'A share can target an active shared workspace before cleanup'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspaces
set deleted_at = now(), updated_at = now()
where id = (
  select workspace_id
  from session_sharing_test_state
  where name = 'ephemeral_target_workspace'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select general_scope, general_workspace_id
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_test_state
        where name = 'workspace_cleanup_share'
      )
    )
  $$,
  $$values ('restricted'::text, null::uuid)$$,
  'An unavailable target workspace transitions affected shares to Restricted'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.session_access_events
    where share_id = (
      select share_id
      from session_sharing_test_state
      where name = 'workspace_cleanup_share'
    )
      and event_type = 'scope_changed'
      and previous_value = 'workspace:' || (
        select workspace_id::text
        from session_sharing_test_state
        where name = 'ephemeral_target_workspace'
      )
      and new_value = 'restricted'
  $$,
  array[1::bigint],
  'Automatic target workspace cleanup remains auditable'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (name, share_id, slug)
    select 'link_share', share_id, public_slug
    from public.create_session_share(auth.uid(), 'link-session')
  $$,
  'An owner can create an independent bearer-link share'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version,
      was_created
    )
    select
      'first_link',
      share_id,
      link_id,
      link_token,
      access_version,
      was_created
    from public.enable_session_share_link(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  'A manager can enable a bearer link'
);

select ok(
  (
    select secret ~ '^[A-Za-z0-9_-]{43}$'
      and access_version = 2
      and was_created
    from session_sharing_test_state
    where name = 'first_link'
  ),
  'Link enablement returns a one-time 256-bit URL-safe token and advances access version'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version,
      was_created
    )
    select
      'duplicate_link',
      share_id,
      link_id,
      link_token,
      access_version,
      was_created
    from public.enable_session_share_link(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  'Enabling an already active link is idempotent'
);

select ok(
  (
    select duplicate.entity_id = original.entity_id
      and duplicate.secret is null
      and duplicate.access_version = original.access_version
      and not duplicate.was_created
    from session_sharing_test_state as duplicate
    join session_sharing_test_state as original
      on original.name = 'first_link'
    where duplicate.name = 'duplicate_link'
  ),
  'Idempotent link enablement never discloses the existing bearer token'
);

select throws_ok(
  $$
    select *
    from public.resolve_session_share_link(
      (select share_id from session_sharing_test_state where name = 'link_share'),
      'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    )
  $$,
  '42501',
  'permission denied for function resolve_session_share_link',
  'Authenticated clients cannot call the legacy bearer resolver'
);

select tests.clear_authentication();
set local role anon;

select throws_ok(
  $$
    select *
    from public.resolve_session_share_link(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_sharing_test_state
        where name = 'first_link'
      )
    )
  $$,
  '42501',
  'permission denied for function resolve_session_share_link',
  'Unauthenticated visitors cannot call the legacy bearer resolver'
);

reset role;
select tests.authenticate_as('share_anonymous');

select throws_ok(
  $$
    select *
    from public.resolve_session_share_link(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_sharing_test_state
        where name = 'first_link'
      )
    )
  $$,
  '42501',
  'permission denied for function resolve_session_share_link',
  'Anonymous Auth users cannot call the legacy bearer resolver'
);

select throws_ok(
  $$
    select *
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth users cannot enter the named-access resolver'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version,
      was_created
    )
    select
      'rotated_link',
      share_id,
      link_id,
      link_token,
      access_version,
      was_created
    from public.rotate_session_share_link(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  'A manager can rotate the active bearer link'
);

select ok(
  (
    select rotated.entity_id <> original.entity_id
      and rotated.secret <> original.secret
      and rotated.secret ~ '^[A-Za-z0-9_-]{43}$'
      and rotated.access_version = original.access_version + 1
      and rotated.was_created
    from session_sharing_test_state as rotated
    join session_sharing_test_state as original
      on original.name = 'first_link'
    where rotated.name = 'rotated_link'
  ),
  'Rotation creates a fresh bearer and advances access version'
);

select throws_ok(
  $$
    select *
    from public.resolve_session_share_link(
      (select share_id from session_sharing_test_state where name = 'link_share'),
      (select secret from session_sharing_test_state where name = 'first_link')
    )
  $$,
  '42501',
  'permission denied for function resolve_session_share_link',
  'Managers cannot bypass the gateway after bearer rotation'
);

select results_eq(
  $$
    select capability
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  array['editor'::text],
  'A manager keeps their higher named identity capability'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      ),
      'restricted'
    )
  $$,
  'A manager can return link access to restricted'
);

select results_eq(
  $$
    select has_active_link
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  array[false],
  'Changing away from link scope revokes the active bearer'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      ),
      'public'
    )
  $$,
  'A manager can make a session publicly viewable'
);

select tests.clear_authentication();
set local role anon;

select throws_ok(
  $$
    select *
    from public.resolve_public_session_share(
      (
        select slug
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'permission denied for function resolve_public_session_share',
  'Unauthenticated visitors cannot call the legacy public resolver'
);

reset role;
select tests.authenticate_as('share_anonymous');

select throws_ok(
  $$
    select *
    from public.resolve_public_session_share(
      (
        select slug
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'permission denied for function resolve_public_session_share',
  'Anonymous Auth users cannot call the legacy public resolver'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  $$values ('viewer'::text, false)$$,
  'A permanent user resolving a public share still receives Viewer only'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select capability
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'link_share'
      )
    )
  $$,
  array['editor'::text],
  'A manager opening a public share keeps their higher identity capability'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select results_eq(
  $$
    select count(*)
    from public.list_my_accessible_sessions()
    where share_id = (
      select share_id
      from session_sharing_test_state
      where name = 'link_share'
    )
  $$,
  array[0::bigint],
  'Undiscovered public shares do not automatically appear in a personal access list'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (name, share_id, slug)
    select 'invitation_share', share_id, public_slug
    from public.create_session_share(auth.uid(), 'invitation-session')
  $$,
  'An owner can create an invitation-scoped test share'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version,
      was_created
    )
    select
      'recipient_invitation',
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      invitation_id,
      invite_token,
      extract(epoch from invitation_expires_at)::bigint,
      was_created
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      '  SHARE-RECIPIENT@EXAMPLE.COM  ',
      'viewer'
    )
  $$,
  'A manager can invite a named Viewer by normalized email'
);

select results_eq(
  $$
    select entry_type, user_email, capability, status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$
    values (
      'invitation'::text,
      'share-recipient@example.com'::text,
      'viewer'::text,
      'pending'::text
    )
  $$,
  'Managers see a normalized pending invitation through the safe projection'
);

select ok(
  (
    select secret ~ '^[A-Za-z0-9_-]{43}$'
      and to_timestamp(access_version) > now() + interval '29 days'
      and to_timestamp(access_version) <= now() + interval '30 days 1 minute'
      and was_created
    from session_sharing_test_state
    where name = 'recipient_invitation'
  ),
  'Invitations return a one-time token with a finite 30-day expiry'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      secret,
      was_created
    )
    select
      'recipient_invitation_duplicate',
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      invitation_id,
      invite_token,
      was_created
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      'share-recipient@example.com',
      'viewer'
    )
  $$,
  'Creating the same pending invitation is idempotent'
);

select ok(
  (
    select duplicate.entity_id = original.entity_id
      and duplicate.secret is null
      and not duplicate.was_created
    from session_sharing_test_state as duplicate
    join session_sharing_test_state as original
      on original.name = 'recipient_invitation'
    where duplicate.name = 'recipient_invitation_duplicate'
  ),
  'Idempotent invitation creation does not disclose or rotate its token'
);

select throws_ok(
  $$
    select *
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      'share-other@example.com',
      'owner'
    )
  $$,
  '22023',
  'invalid session access invitation',
  'Invitations cannot grant a role above Editor'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select throws_ok(
  $$
    select *
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_invitation'
      ),
      (
        select secret
        from session_sharing_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  '22023',
  'session access invitation is invalid or unavailable',
  'A valid invitation token cannot be accepted by the wrong account'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select throws_ok(
  $$
    select *
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_invitation'
      ),
      'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    )
  $$,
  '22023',
  'session access invitation is invalid or unavailable',
  'The intended recipient cannot accept an incorrect invitation token'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability,
      was_created
    )
    select
      'recipient_editor_request',
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      request_id,
      requested_capability,
      was_created
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      'editor'
    )
  $$,
  'An invitee can request stronger named access before accepting'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability
    )
    select
      'recipient_grant',
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      grant_id,
      capability
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_editor_request'
      ),
      'approved',
      'editor'
    )
  $$,
  'A manager can approve a named Editor request'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$
    select grant_id, capability
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_invitation'
      ),
      (
        select secret
        from session_sharing_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  $$
    select entity_id, 'editor'::text
    from session_sharing_test_state
    where name = 'recipient_grant'
  $$,
  'Accepting a lower Viewer invitation never reduces an existing Editor grant'
);

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$values ('editor'::text, false)$$,
  'A named Editor resolves effective Editor access without management authority'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select entry_type, user_email, capability, status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$
    values (
      'grant'::text,
      'share-recipient@example.com'::text,
      'editor'::text,
      'active'::text
    )
  $$,
  'Manager access lists collapse accepted invitations and reviewed requests to the active grant'
);

select throws_ok(
  $$
    select *
    from public.revoke_session_access_invitation(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  '22023',
  'accepted invitations require grant revocation',
  'Accepted invitations must be revoked through their resulting grant'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select throws_ok(
  $$
    select *
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_grant'
      ),
      'commenter'
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Grantees cannot update their own capability'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select capability
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_grant'
      ),
      'commenter'
    )
  $$,
  array['commenter'::text],
  'A manager can change a named grant to Commenter'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_sharing_test_state
        where name = 'target_workspace'
      )
    )
  $$,
  'A named grant can coexist with general workspace Viewer access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$values ('commenter'::text, false)$$,
  'Effective access chooses Commenter over inherited workspace Viewer'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select capability
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_grant'
      ),
      'viewer'
    )
  $$,
  array['viewer'::text],
  'A manager can explicitly lower a named grant to Viewer'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$values ('viewer'::text, false)$$,
  'Equal direct and workspace capabilities resolve to Viewer without management'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$values ('editor'::text, true)$$,
  'Managers keep Editor and management access across all general and direct grants'
);

select lives_ok(
  $$
    select *
    from public.revoke_session_access_grant(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_grant'
      )
    )
  $$,
  'A manager can revoke a named grant'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  $$values ('viewer'::text, false)$$,
  'Revoking a direct grant leaves valid workspace Viewer access intact'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      ),
      'restricted'
    )
  $$,
  'A manager can remove the remaining general workspace access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'invitation_share'
      )
    )
  $$,
  array[0::bigint],
  'Restricted scope plus a revoked direct grant resolves no access'
);

select throws_ok(
  $$
    select *
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'recipient_invitation'
      ),
      (
        select secret
        from session_sharing_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  '22023',
  'session access invitation is invalid or unavailable',
  'An accepted invitation token cannot restore a revoked grant'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select lives_ok(
  $$
    insert into session_sharing_test_state (name, share_id, slug)
    select 'request_share', share_id, public_slug
    from public.create_session_share(auth.uid(), 'request-session')
  $$,
  'An owner can create an independent access-request share'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_approved');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability,
      was_created
    )
    select
      'approved_request',
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      request_id,
      requested_capability,
      was_created
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      'commenter'
    )
  $$,
  'A permanent user can request Commenter access'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability,
      was_created
    )
    select
      'approved_request_duplicate',
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      request_id,
      requested_capability,
      was_created
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      'editor'
    )
  $$,
  'A duplicate pending access request is idempotent'
);

select ok(
  (
    select duplicate.entity_id = original.entity_id
      and duplicate.capability = 'commenter'
      and not duplicate.was_created
    from session_sharing_test_state as duplicate
    join session_sharing_test_state as original
      on original.name = 'approved_request'
    where duplicate.name = 'approved_request_duplicate'
  ),
  'A duplicate request preserves the original requested capability'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_denied');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability,
      was_created
    )
    select
      'denied_request',
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      request_id,
      requested_capability,
      was_created
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      'viewer'
    )
  $$,
  'A second permanent user can request Viewer access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_cancelled');

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability,
      was_created
    )
    select
      'cancelled_request',
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      request_id,
      requested_capability,
      was_created
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      'editor'
    )
  $$,
  'A third permanent user can request Editor access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select entry_type, capability, status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
    order by capability
  $$,
  $$
    values
      ('request'::text, 'commenter'::text, 'pending'::text),
      ('request'::text, 'editor'::text, 'pending'::text),
      ('request'::text, 'viewer'::text, 'pending'::text)
  $$,
  'Managers can list all pending requests without direct table access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_denied');

select throws_ok(
  $$
    select *
    from public.cancel_session_access_request(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'approved_request'
      )
    )
  $$,
  '22023',
  'session access request is unavailable',
  'A requester cannot cancel another user request'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_cancelled');

select results_eq(
  $$
    select status
    from public.cancel_session_access_request(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'cancelled_request'
      )
    )
  $$,
  array['cancelled'::text],
  'A requester can cancel their own pending request'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select status, grant_id, capability
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'denied_request'
      ),
      'denied'
    )
  $$,
  $$values ('denied'::text, null::uuid, null::text)$$,
  'A manager can deny a pending request without creating a grant'
);

select lives_ok(
  $$
    insert into session_sharing_test_state (
      name,
      share_id,
      entity_id,
      capability
    )
    select
      'approved_request_grant',
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      grant_id,
      capability
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_test_state
        where name = 'approved_request'
      ),
      'approved',
      'editor'
    )
  $$,
  'A manager can approve a request with an explicit Editor grant'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_approved');

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
  $$,
  $$values ('editor'::text, false)$$,
  'An approved request immediately resolves its named capability'
);

select throws_ok(
  $$
    select *
    from public.request_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      ),
      'viewer'
    )
  $$,
  '22023',
  'session access request not needed',
  'A user cannot request a capability lower than existing effective access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_denied');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
  $$,
  array[0::bigint],
  'A denied request grants no access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_cancelled');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
  $$,
  array[0::bigint],
  'A cancelled request grants no access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('share_owner');

select results_eq(
  $$
    select count(*)
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
    where entry_type = 'request'
  $$,
  array[0::bigint],
  'Reviewed and cancelled requests disappear from the pending management list'
);

select results_eq(
  $$
    select count(*), bool_and(capability = 'editor' and manage_access)
    from public.list_my_accessible_sessions()
  $$,
  $$values (6::bigint, true)$$,
  'Managers list every session in their source workspace as manageable Editor access'
);

select tests.clear_authentication();
select tests.authenticate_as('share_workspace_member');

select results_eq(
  $$select session_id, capability from public.list_my_accessible_sessions()$$,
  $$values ('workspace-session'::text, 'viewer'::text)$$,
  'Workspace members list only sessions shared to their workspace'
);

select tests.clear_authentication();
select tests.authenticate_as('share_recipient');

select results_eq(
  $$select session_id, capability from public.list_my_accessible_sessions()$$,
  $$values ('workspace-session'::text, 'viewer'::text)$$,
  'Revoked named recipients retain only independently valid workspace shares'
);

select tests.clear_authentication();
select tests.authenticate_as('share_request_approved');

select results_eq(
  $$select session_id, capability from public.list_my_accessible_sessions()$$,
  $$values ('request-session'::text, 'editor'::text)$$,
  'Approved named grants appear in the recipient accessible-session list'
);

select tests.clear_authentication();
select tests.authenticate_as('share_other');

select results_eq(
  $$select count(*) from public.list_my_accessible_sessions()$$,
  array[0::bigint],
  'Public-only discovery never adds a session to an unrelated user list'
);

select throws_ok(
  $$
    select *
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_test_state
        where name = 'request_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Nonmanagers cannot enumerate invitations, grants, or requests'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select count(*) = 2 and bool_and(octet_length(token_hash) = 32)
    from public.session_share_links
  )
    and (
      select count(*) = 1 and bool_and(octet_length(token_hash) = 32)
      from public.session_access_invitations
    ),
  'Only fixed-length SHA-256 digests are stored for every emitted secret'
);

select ok(
  not exists (
    select 1
    from public.session_share_links as link
    join session_sharing_test_state as state
      on state.entity_id = link.id
    where state.name in ('first_link', 'rotated_link')
      and link.token_hash <> extensions.digest(state.secret, 'sha256')
  )
    and exists (
      select 1
      from public.session_access_invitations as invitation
      join session_sharing_test_state as state
        on state.entity_id = invitation.id
      where state.name = 'recipient_invitation'
        and invitation.token_hash = extensions.digest(state.secret, 'sha256')
    ),
  'Stored digests match emitted secrets without storing plaintext tokens'
);

select ok(
  not exists (
    select 1
    from public.session_access_events as event
    cross join session_sharing_test_state as state
    where state.secret is not null
      and state.secret <> ''
      and (
        event.previous_value = state.secret
        or event.new_value = state.secret
      )
  ),
  'Audit events never record link or invitation bearer secrets'
);

select ok(
  not exists (
    select 1
    from information_schema.columns as column_info
    where column_info.table_schema = 'public'
      and column_info.table_name in (
        'session_share_links',
        'session_access_invitations',
        'session_access_events'
      )
      and column_info.column_name like '%token%'
      and column_info.column_name <> 'token_hash'
  ),
  'Persistent authority and audit schemas contain no plaintext token column'
);

select * from finish();
rollback;
