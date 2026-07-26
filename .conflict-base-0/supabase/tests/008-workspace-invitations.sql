begin;
select plan(36);

select tests.create_supabase_user('invite_owner', 'invite-owner@example.com');
select tests.create_supabase_user('invite_recipient', 'invite-recipient@example.com');
select tests.create_supabase_user('invite_other', 'invite-other@example.com');

create temporary table workspace_invitation_test_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz,
  membership_id uuid,
  was_created boolean
);

grant all on workspace_invitation_test_state to authenticated, service_role;

insert into workspace_invitation_test_state (name, workspace_id)
values
  ('shared_workspace', gen_random_uuid()),
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
  jsonb_build_object('test_identifier', 'invite_anonymous'),
  '{}'::jsonb,
  true,
  now(),
  now()
from workspace_invitation_test_state
where name = 'anonymous_user';

select tests.authenticate_as('invite_anonymous');

select results_eq(
  $$select count(*) from public.workspaces$$,
  array[0::bigint],
  'Anonymous Auth users do not receive workspace authority'
);

select tests.clear_authentication();
reset role;

update auth.users
set
  email = 'invite-converted@example.com',
  email_confirmed_at = now(),
  is_anonymous = false,
  updated_at = now()
where id = (
  select workspace_id
  from workspace_invitation_test_state
  where name = 'anonymous_user'
);

select tests.authenticate_as('invite_anonymous');

select results_eq(
  $$
    select count(*)
    from public.workspaces as workspace
    join public.workspace_memberships as membership
      on membership.workspace_id = workspace.id
    where workspace.id = auth.uid()
      and workspace.kind = 'personal'
      and membership.user_id = auth.uid()
      and membership.role = 'owner'
      and membership.deleted_at is null
  $$,
  array[1::bigint],
  'Converting an anonymous account provisions its personal workspace'
);

select tests.clear_authentication();
reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('invite_owner'),
  tests.get_supabase_uid('invite_recipient'),
  tests.get_supabase_uid('invite_other')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('invite_owner'),
  'shared',
  'Invitation test workspace'
from workspace_invitation_test_state
where name = 'shared_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('invite_owner'),
  'owner'
from workspace_invitation_test_state
where name = 'shared_workspace';

select results_eq(
  $$
    select kind
    from public.workspaces
    where id = (
      select workspace_id
      from workspace_invitation_test_state
      where name = 'shared_workspace'
    )
  $$,
  array['shared'::text],
  'Trusted service code can provision a shared workspace'
);

select tests.clear_authentication();
reset role;

select ok(
  not has_table_privilege('authenticated', 'public.workspaces', 'INSERT')
    and not has_table_privilege(
      'authenticated',
      'public.workspace_invitations',
      'SELECT'
    )
    and not has_table_privilege(
      'authenticated',
      'public.workspace_invitations',
      'INSERT'
    )
    and not has_table_privilege(
      'authenticated',
      'public.workspace_invitations',
      'UPDATE'
    ),
  'Clients cannot create shared workspaces or access invitation rows directly'
);

select ok(
  has_function_privilege(
    'authenticated',
    'public.create_workspace_invitation(uuid,text)',
    'EXECUTE'
  )
    and has_function_privilege(
      'authenticated',
      'public.accept_workspace_invitation(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.revoke_workspace_invitation(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.revoke_workspace_membership(uuid,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.list_workspace_invitations(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.list_workspace_memberships(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.create_workspace_invitation(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.accept_workspace_invitation(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.revoke_workspace_invitation(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.revoke_workspace_membership(uuid,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.list_workspace_invitations(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.list_workspace_memberships(uuid)',
      'EXECUTE'
    ),
  'Only authenticated clients can execute invitation RPC wrappers'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where (
      namespace.nspname = 'private'
      and proc.proname in (
        'create_workspace_invitation',
        'accept_workspace_invitation',
        'revoke_workspace_invitation',
        'revoke_workspace_membership',
        'list_workspace_invitations',
        'list_workspace_memberships'
      )
      and (
        not proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
    or (
      namespace.nspname = 'public'
      and proc.proname in (
        'create_workspace_invitation',
        'accept_workspace_invitation',
        'revoke_workspace_invitation',
        'revoke_workspace_membership',
        'list_workspace_invitations',
        'list_workspace_memberships'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
  ),
  'Privileged implementations are private and every RPC uses an empty search path'
);

select tests.authenticate_as('invite_owner');

select lives_ok(
  $$
    insert into workspace_invitation_test_state (
      name,
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    )
    select
      'first_invite',
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    from public.create_workspace_invitation(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      '  Invite-Recipient@Example.com  '
    )
  $$,
  'An owner can create a pending member invitation'
);

select results_eq(
  $$
    select invitee_email
    from public.list_workspace_invitations(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      )
    )
  $$,
  array['invite-recipient@example.com'::text],
  'Invitation email normalization is visible through the safe manager projection'
);

select ok(
  (
    select was_created
      and length(invite_token) = 43
      and invitation_expires_at > now() + interval '29 days'
      and invitation_expires_at <= now() + interval '30 days 1 minute'
    from workspace_invitation_test_state
    where name = 'first_invite'
  ),
  'The RPC returns a 256-bit URL-safe token once with a finite expiry'
);

insert into workspace_invitation_test_state (
  name,
  invitation_id,
  invite_token,
  invitation_expires_at,
  was_created
)
select
  'duplicate_invite',
  invitation_id,
  invite_token,
  invitation_expires_at,
  was_created
from public.create_workspace_invitation(
  (
    select workspace_id
    from workspace_invitation_test_state
    where name = 'shared_workspace'
  ),
  'invite-recipient@example.com'
);

select ok(
  (
    select duplicate.invitation_id = original.invitation_id
      and duplicate.invite_token is null
      and not duplicate.was_created
    from workspace_invitation_test_state as duplicate
    join workspace_invitation_test_state as original
      on original.name = 'first_invite'
    where duplicate.name = 'duplicate_invite'
  ),
  'A duplicate pending invite preserves its id and does not rotate its token'
);

select results_eq(
  $$
    select invitee_user_id
    from public.list_workspace_invitations(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      )
    )
  $$,
  array[tests.get_supabase_uid('invite_recipient')],
  'An existing recipient account is bound when the invitation is issued'
);

select throws_ok(
  $$
    select *
    from public.create_workspace_invitation(
      tests.get_supabase_uid('invite_owner'),
      'invite-other@example.com'
    )
  $$,
  '42501',
  'workspace invitation operation not permitted',
  'Personal workspaces reject workspace invitations'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_other');

select throws_ok(
  $$
    select *
    from public.create_workspace_invitation(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      'someone@example.com'
    )
  $$,
  '42501',
  'workspace invitation operation not permitted',
  'A nonmember cannot invite someone to a shared workspace'
);

select throws_ok(
  $$
    select *
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'first_invite'
      )
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'A valid token cannot be accepted by an account with the wrong email'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_recipient');

select throws_ok(
  $$
    select *
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      ),
      'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'A recipient cannot accept with the wrong token'
);

select lives_ok(
  $$
    insert into workspace_invitation_test_state (
      name,
      workspace_id,
      membership_id
    )
    select
      'accepted_membership',
      workspace_id,
      membership_id
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'first_invite'
      )
    )
  $$,
  'The intended recipient can atomically accept the invitation'
);

select results_eq(
  $$
    select role
    from public.workspace_memberships
    where workspace_id = (
      select workspace_id
      from workspace_invitation_test_state
      where name = 'shared_workspace'
    )
      and user_id = auth.uid()
  $$,
  array['member'::text],
  'Acceptance grants only the member role'
);

select results_eq(
  $$
    select count(*)
    from public.workspaces
    where id = (
      select workspace_id
      from workspace_invitation_test_state
      where name = 'shared_workspace'
    )
  $$,
  array[1::bigint],
  'The accepted shared workspace immediately appears in the recipient projection'
);

select results_eq(
  $$
    select workspace_id, membership_id
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'first_invite'
      )
    )
  $$,
  $$
    select workspace_id, membership_id
    from workspace_invitation_test_state
    where name = 'accepted_membership'
  $$,
  'Repeating acceptance returns the same active membership'
);

select throws_ok(
  $$
    select *
    from public.list_workspace_memberships(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      )
    )
  $$,
  '42501',
  'workspace membership operation not permitted',
  'Members cannot enumerate workspace access'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_owner');

select results_eq(
  $$
    select user_email
    from public.list_workspace_memberships(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      )
    )
    where role = 'member' and deleted_at is null
  $$,
  array['invite-recipient@example.com'::text],
  'Managers can list active workspace access without exposing auth tables'
);

select results_eq(
  $$
    select count(*)
    from public.list_workspace_invitations(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      )
    )
    where accepted_at is not null and revoked_at is null
  $$,
  array[1::bigint],
  'Managers can see that an invitation was accepted without seeing its token hash'
);

select throws_ok(
  $$
    select *
    from public.revoke_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      )
    )
  $$,
  '22023',
  'accepted invitations require membership revocation',
  'Accepted invitations cannot be cancelled as pending invitations'
);

select lives_ok(
  $$
    select *
    from public.revoke_workspace_membership(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      tests.get_supabase_uid('invite_recipient')
    )
  $$,
  'An owner can revoke an active member'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_recipient');

select results_eq(
  $$
    select count(*)
    from public.workspaces
    where id = (
      select workspace_id
      from workspace_invitation_test_state
      where name = 'shared_workspace'
    )
  $$,
  array[0::bigint],
  'Revocation immediately removes the workspace from the member projection'
);

select throws_ok(
  $$
    select *
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'first_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'first_invite'
      )
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'An accepted token cannot restore access after membership revocation'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_owner');

select lives_ok(
  $$
    insert into workspace_invitation_test_state (
      name,
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    )
    select
      'regrant_invite',
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    from public.create_workspace_invitation(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      'invite-recipient@example.com'
    )
  $$,
  'Regranting access requires a fresh invitation'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_recipient');

select lives_ok(
  $$
    update workspace_invitation_test_state
    set membership_id = accepted.membership_id,
        workspace_id = accepted.workspace_id
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'regrant_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'regrant_invite'
      )
    ) as accepted
    where name = 'regrant_invite'
  $$,
  'The recipient can accept the fresh regrant invitation'
);

select ok(
  (
    select regrant.membership_id = original.membership_id
    from workspace_invitation_test_state as regrant
    join workspace_invitation_test_state as original
      on original.name = 'accepted_membership'
    where regrant.name = 'regrant_invite'
  ),
  'A fresh invitation reactivates the existing member row'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_owner');

select lives_ok(
  $$
    insert into workspace_invitation_test_state (
      name,
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    )
    select
      'cancelled_invite',
      invitation_id,
      invite_token,
      invitation_expires_at,
      was_created
    from public.create_workspace_invitation(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      'invite-other@example.com'
    )
  $$,
  'An owner can create another pending invitation'
);

select lives_ok(
  $$
    select *
    from public.revoke_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'cancelled_invite'
      )
    )
  $$,
  'An owner can cancel a pending invitation'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_other');

select throws_ok(
  $$
    select *
    from public.accept_workspace_invitation(
      (
        select invitation_id
        from workspace_invitation_test_state
        where name = 'cancelled_invite'
      ),
      (
        select invite_token
        from workspace_invitation_test_state
        where name = 'cancelled_invite'
      )
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'A cancelled invitation can never be accepted'
);

select tests.clear_authentication();
select tests.authenticate_as('invite_owner');

select throws_ok(
  $$
    select *
    from public.revoke_workspace_membership(
      (
        select workspace_id
        from workspace_invitation_test_state
        where name = 'shared_workspace'
      ),
      tests.get_supabase_uid('invite_owner')
    )
  $$,
  '42501',
  'workspace membership operation not permitted',
  'The canonical owner cannot be revoked'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select octet_length(token_hash)
    from public.workspace_invitations
    order by created_at, id
  $$,
  array[32, 32, 32]::integer[],
  'Only fixed-length SHA-256 token digests are stored'
);

select results_eq(
  $$
    select count(*)
    from public.workspace_invitations
    where role <> 'member'
  $$,
  array[0::bigint],
  'Invitation acceptance cannot request an elevated workspace role'
);

select tests.clear_authentication();
reset role;

select ok(
  to_regprocedure('public.create_shared_workspace(text)') is null,
  'Client shared-workspace creation remains disabled pending ownership lifecycle rules'
);

select * from finish();
rollback;
