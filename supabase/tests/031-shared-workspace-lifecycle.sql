begin;
select plan(27);

select tests.create_supabase_user('lifecycle_owner', 'lifecycle-owner@example.com');
select tests.create_supabase_user('lifecycle_member', 'lifecycle-member@example.com');
select tests.create_supabase_user('lifecycle_outsider', 'lifecycle-outsider@example.com');
select tests.create_supabase_user('lifecycle_unconfirmed', 'lifecycle-unconfirmed@example.com');

create temporary table workspace_lifecycle_test_state (
  name text primary key,
  workspace_id uuid,
  membership_id uuid,
  invitation_id uuid,
  invite_token text,
  left_at timestamptz
);

grant all on workspace_lifecycle_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('lifecycle_owner'),
  tests.get_supabase_uid('lifecycle_member'),
  tests.get_supabase_uid('lifecycle_outsider')
);

update auth.users
set email_confirmed_at = null
where id = tests.get_supabase_uid('lifecycle_unconfirmed');

select ok(
  has_function_privilege('authenticated', 'public.create_workspace(text)', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.rename_workspace(uuid,text)', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.set_workspace_membership_role(uuid,uuid,text)', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.transfer_workspace_ownership(uuid,uuid)', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.leave_workspace(uuid)', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.delete_workspace(uuid)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.create_workspace(text)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.rename_workspace(uuid,text)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.set_workspace_membership_role(uuid,uuid,text)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.transfer_workspace_ownership(uuid,uuid)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.leave_workspace(uuid)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.delete_workspace(uuid)', 'EXECUTE'),
  'Only authenticated clients can execute workspace lifecycle RPC wrappers'
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
        'create_workspace',
        'rename_workspace',
        'set_workspace_membership_role',
        'transfer_workspace_ownership',
        'leave_workspace',
        'delete_workspace'
      )
      and (
        not proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
    or (
      namespace.nspname = 'public'
      and proc.proname in (
        'create_workspace',
        'rename_workspace',
        'set_workspace_membership_role',
        'transfer_workspace_ownership',
        'leave_workspace',
        'delete_workspace'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
  ),
  'Privileged lifecycle implementations are private and every RPC uses an empty search path'
);

select tests.authenticate_as('lifecycle_unconfirmed');

select throws_ok(
  $$select * from public.create_workspace('Shadow Org')$$,
  '42501',
  'workspace operation not permitted',
  'An unconfirmed account cannot create shared workspaces'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_owner');

select lives_ok(
  $$
    insert into workspace_lifecycle_test_state (name, workspace_id, membership_id)
    select 'hq', workspace_id, membership_id
    from public.create_workspace('  Lifecycle HQ  ')
  $$,
  'A confirmed user can create a shared workspace'
);

select ok(
  exists (
    select 1
    from public.workspaces as workspace
    join public.workspace_memberships as membership
      on membership.workspace_id = workspace.id
    where workspace.id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and workspace.kind = 'shared'
      and workspace.name = 'Lifecycle HQ'
      and workspace.owner_user_id = auth.uid()
      and workspace.deleted_at is null
      and membership.user_id = auth.uid()
      and membership.role = 'owner'
      and membership.deleted_at is null
  ),
  'Creation trims the name and grants the creator an owner membership'
);

select throws_ok(
  $$select * from public.create_workspace('   ')$$,
  '22023',
  'invalid workspace name',
  'Blank workspace names are rejected'
);

select lives_ok(
  $$
    insert into workspace_lifecycle_test_state (name, invitation_id, invite_token)
    select 'member_invite', invitation_id, invite_token
    from public.create_workspace_invitation(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      'lifecycle-member@example.com'
    )
  $$,
  'The owner can invite a member into the new workspace'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_member');

select lives_ok(
  $$
    insert into workspace_lifecycle_test_state (name, workspace_id, membership_id)
    select 'member_membership', workspace_id, membership_id
    from public.accept_workspace_invitation(
      (select invitation_id from workspace_lifecycle_test_state where name = 'member_invite'),
      (select invite_token from workspace_lifecycle_test_state where name = 'member_invite')
    )
  $$,
  'The invited member can accept and join'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_owner');

select results_eq(
  $$
    select membership_role
    from public.set_workspace_membership_role(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_member'),
      'admin'
    )
  $$,
  array['admin'::text],
  'The owner can promote a member to admin'
);

select throws_ok(
  $$
    select * from public.set_workspace_membership_role(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_owner'),
      'member'
    )
  $$,
  '42501',
  'workspace membership operation not permitted',
  'The owner role cannot be changed through role management'
);

select throws_ok(
  $$
    select * from public.set_workspace_membership_role(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_member'),
      'owner'
    )
  $$,
  '22023',
  'invalid workspace role',
  'Ownership cannot be granted through role management'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_member');

select results_eq(
  $$
    select workspace_name
    from public.rename_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      'Lifecycle Core'
    )
  $$,
  array['Lifecycle Core'::text],
  'A promoted admin can rename the workspace'
);

select throws_ok(
  $$
    select * from public.set_workspace_membership_role(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_member'),
      'member'
    )
  $$,
  '42501',
  'workspace membership operation not permitted',
  'Admins cannot demote an admin, including themselves'
);

select throws_ok(
  $$
    select * from public.transfer_workspace_ownership(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_member')
    )
  $$,
  '42501',
  'workspace ownership operation not permitted',
  'Admins cannot transfer ownership to themselves'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_owner');

select throws_ok(
  $$
    select * from public.transfer_workspace_ownership(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_outsider')
    )
  $$,
  '42501',
  'workspace ownership operation not permitted',
  'Ownership cannot be transferred to a non-member'
);

select lives_ok(
  $$
    select * from public.transfer_workspace_ownership(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      tests.get_supabase_uid('lifecycle_member')
    )
  $$,
  'The owner can transfer ownership to an active member'
);

select tests.clear_authentication();
reset role;

select ok(
  exists (
    select 1
    from public.workspaces as workspace
    where workspace.id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and workspace.owner_user_id = tests.get_supabase_uid('lifecycle_member')
  )
  and exists (
    select 1
    from public.workspace_memberships as membership
    where membership.workspace_id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and membership.user_id = tests.get_supabase_uid('lifecycle_owner')
      and membership.role = 'admin'
      and membership.deleted_at is null
  )
  and exists (
    select 1
    from public.workspace_memberships as membership
    where membership.workspace_id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and membership.user_id = tests.get_supabase_uid('lifecycle_member')
      and membership.role = 'owner'
      and membership.deleted_at is null
  ),
  'Transfer swaps the owner column and both membership roles atomically'
);

select tests.authenticate_as('lifecycle_owner');

select throws_ok(
  $$
    select * from public.delete_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq')
    )
  $$,
  '42501',
  'workspace operation not permitted',
  'A demoted previous owner cannot delete the workspace'
);

select lives_ok(
  $$
    update workspace_lifecycle_test_state
    set left_at = departure.left_at
    from public.leave_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq')
    ) as departure
    where name = 'hq'
  $$,
  'A non-owner can leave the workspace'
);

select results_eq(
  $$
    select count(*)
    from public.workspaces
    where id = (
      select workspace_id from workspace_lifecycle_test_state where name = 'hq'
    )
  $$,
  array[0::bigint],
  'Leaving immediately removes the workspace from the member projection'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_member');

select throws_ok(
  $$
    select * from public.leave_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq')
    )
  $$,
  '22023',
  'owner must transfer ownership before leaving',
  'The owner must hand off the workspace before leaving'
);

select lives_ok(
  $$
    insert into workspace_lifecycle_test_state (name, invitation_id)
    select 'pending_invite', invitation_id
    from public.create_workspace_invitation(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      'lifecycle-outsider@example.com'
    )
  $$,
  'The new owner can still invite before deletion'
);

select lives_ok(
  $$
    select * from public.delete_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq')
    )
  $$,
  'The owner can soft delete the workspace'
);

select tests.clear_authentication();
reset role;

select ok(
  exists (
    select 1
    from public.workspaces as workspace
    where workspace.id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and workspace.deleted_at is not null
  )
  and not exists (
    select 1
    from public.workspace_invitations as invitation
    where invitation.workspace_id = (
        select workspace_id from workspace_lifecycle_test_state where name = 'hq'
      )
      and invitation.accepted_at is null
      and invitation.revoked_at is null
  ),
  'Deletion soft deletes the workspace and revokes its pending invitations'
);

select tests.authenticate_as('lifecycle_outsider');

select throws_ok(
  $$
    select * from public.rename_workspace(
      (select workspace_id from workspace_lifecycle_test_state where name = 'hq'),
      'Hijacked'
    )
  $$,
  '42501',
  'workspace operation not permitted',
  'Deleted workspaces reject lifecycle operations'
);

select tests.clear_authentication();
select tests.authenticate_as('lifecycle_owner');

select lives_ok(
  $$
    select count(*)
    from generate_series(1, 20) as bulk(i),
    lateral public.create_workspace('Bulk ' || bulk.i::text)
  $$,
  'A user can own up to twenty active shared workspaces'
);

select throws_ok(
  $$select * from public.create_workspace('One Too Many')$$,
  '22023',
  'workspace limit reached',
  'The owned-workspace cap rejects the twenty-first active workspace'
);

select * from finish();
rollback;
