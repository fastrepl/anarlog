begin;
select plan(13);

select tests.create_supabase_user('workspace_owner', 'workspace-owner@example.com');
select tests.create_supabase_user('workspace_other', 'workspace-other@example.com');

select tests.authenticate_as('workspace_owner');

select results_eq(
  $$
    select id
    from public.workspaces
    where id = auth.uid()
      and owner_user_id = auth.uid()
      and kind = 'personal'
  $$,
  array[tests.get_supabase_uid('workspace_owner')],
  'Signup creates a personal workspace whose id matches the user id'
);

select results_eq(
  $$
    select role
    from public.workspace_memberships
    where workspace_id = auth.uid()
      and user_id = auth.uid()
  $$,
  array['owner'::text],
  'Signup creates the personal workspace owner membership'
);

select results_eq(
  $$select count(*) from public.workspaces$$,
  array[1::bigint],
  'A user initially sees only their personal workspace'
);

select results_eq(
  $$select count(*) from public.workspace_memberships$$,
  array[1::bigint],
  'A user initially sees only their own active membership'
);

select ok(
  not has_table_privilege('authenticated', 'public.workspaces', 'INSERT')
    and not has_table_privilege('authenticated', 'public.workspaces', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.workspaces', 'DELETE'),
  'Authenticated clients cannot mutate workspaces'
);

select ok(
  not has_table_privilege('authenticated', 'public.workspace_memberships', 'INSERT')
    and not has_table_privilege('authenticated', 'public.workspace_memberships', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.workspace_memberships', 'DELETE'),
  'Authenticated clients cannot mutate workspace memberships'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  format(
    $$
      update public.workspaces
      set name = 'Renamed personal workspace', updated_at = now()
      where id = %L
    $$,
    tests.get_supabase_uid('workspace_owner')
  ),
  'Service role can mutate workspace authority rows'
);

select tests.clear_authentication();
select tests.authenticate_as('workspace_owner');

select results_eq(
  $$select name from public.workspaces where id = auth.uid()$$,
  array['Renamed personal workspace'::text],
  'The owner can read the updated personal workspace projection'
);

select tests.clear_authentication();
select tests.authenticate_as('workspace_other');

select results_eq(
  $$select count(*) from public.workspaces$$,
  array[1::bigint],
  'Another user cannot read a workspace they have not joined'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspace_memberships
set deleted_at = now(), updated_at = now()
where user_id = tests.get_supabase_uid('workspace_owner')
  and workspace_id = tests.get_supabase_uid('workspace_owner');

select tests.clear_authentication();
select tests.authenticate_as('workspace_owner');

select results_eq(
  $$select count(*) from public.workspaces$$,
  array[0::bigint],
  'A soft-deleted membership immediately hides its workspace'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.workspace_memberships
set deleted_at = null, updated_at = now()
where user_id = tests.get_supabase_uid('workspace_owner')
  and workspace_id = tests.get_supabase_uid('workspace_owner');

select tests.clear_authentication();
reset role;

select results_eq(
  $$
    select count(*)
    from auth.users AS auth_user
    where not exists (
      select 1
      from public.workspaces AS workspace
      join public.workspace_memberships AS membership
        on membership.workspace_id = workspace.id
      where workspace.id = auth_user.id
        and workspace.owner_user_id = auth_user.id
        and workspace.kind = 'personal'
        and membership.user_id = auth_user.id
        and membership.role = 'owner'
    )
  $$,
  array[0::bigint],
  'Every existing auth user has a personal workspace and owner membership'
);

select tests.create_supabase_user('workspace_delete', 'workspace-delete@example.com');
create temporary table workspace_delete_user AS
select tests.get_supabase_uid('workspace_delete') AS id;

delete from auth.users
where id = (select id from workspace_delete_user);

select results_eq(
  $$
    select count(*)
    from (
      select id
      from public.workspaces
      where id = (select id from workspace_delete_user)
      union all
      select id
      from public.workspace_memberships
      where user_id = (select id from workspace_delete_user)
    ) AS workspace_authority_rows
  $$,
  array[0::bigint],
  'Deleting an auth user cascades their personal workspace authority rows'
);

select ok(
  to_regprocedure('private.handle_new_user_workspace()') is not null
    and not has_function_privilege(
      'authenticated',
      'private.handle_new_user_workspace()',
      'EXECUTE'
    ),
  'The signup trigger function is private and unavailable to clients'
);

select * from finish();
rollback;
