begin;
select plan(10);

select tests.create_supabase_user('team_pro_owner', 'team-pro-owner@example.com');
select tests.create_supabase_user('team_free_member', 'team-free-member@example.com');

create temporary table team_pro_test_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text
);

grant all on team_pro_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('team_pro_owner'),
  tests.get_supabase_uid('team_free_member')
);

select ok(
  not has_function_privilege(
    'authenticated',
    'private.create_workspace(text)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'private.rename_workspace(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.create_workspace_invitation(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.resend_workspace_invitation(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.set_workspace_membership_role(uuid,uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.transfer_workspace_ownership(uuid,uuid)',
      'EXECUTE'
    ),
  'Authenticated clients cannot bypass the protected Team RPC wrappers'
);

select tests.authenticate_as('team_pro_owner');

select throws_ok(
  $$select * from public.create_workspace('Free Team')$$,
  '42501',
  'hyprnote pro entitlement required',
  'A free account cannot create a Team workspace'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('team_pro_owner');

select lives_ok(
  $$
    insert into team_pro_test_state (name, workspace_id)
    select 'hq', workspace_id
    from public.create_workspace('Pro Team')
  $$,
  'A Pro account can create a Team workspace'
);

select tests.clear_authentication();
select tests.authenticate_as('team_pro_owner');

select throws_ok(
  $$
    select *
    from public.rename_workspace(
      (select workspace_id from team_pro_test_state where name = 'hq'),
      'Free Rename'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'A free account cannot manage its existing Team workspace'
);

select throws_ok(
  $$
    select *
    from public.create_workspace_invitation(
      (select workspace_id from team_pro_test_state where name = 'hq'),
      'team-free-member@example.com'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'A free account cannot invite Team members'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('team_pro_owner');

select lives_ok(
  $$
    insert into team_pro_test_state (
      name,
      invitation_id,
      invite_token
    )
    select 'member_invite', invitation_id, invite_token
    from public.create_workspace_invitation(
      (select workspace_id from team_pro_test_state where name = 'hq'),
      'team-free-member@example.com'
    )
  $$,
  'A Pro account can invite a Team member'
);

select tests.clear_authentication();
select tests.authenticate_as('team_pro_owner');

select throws_ok(
  $$
    select *
    from public.resend_workspace_invitation(
      (select invitation_id from team_pro_test_state where name = 'member_invite')
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'A free account cannot resend a Team invitation'
);

select results_eq(
  $$
    select invitation_id
    from public.list_workspace_invitations(
      (select workspace_id from team_pro_test_state where name = 'hq')
    )
    where accepted_at is null and revoked_at is null
  $$,
  $$
    select invitation_id
    from team_pro_test_state
    where name = 'member_invite'
  $$,
  'Rejected resend preserves the original pending invitation'
);

select tests.clear_authentication();
select tests.authenticate_as('team_free_member');

select lives_ok(
  $$
    select *
    from public.accept_workspace_invitation(
      (select invitation_id from team_pro_test_state where name = 'member_invite'),
      (select invite_token from team_pro_test_state where name = 'member_invite')
    )
  $$,
  'A free invitee can accept a Team invitation'
);

select tests.clear_authentication();
select tests.authenticate_as('team_pro_owner');

select lives_ok(
  $$
    select *
    from public.delete_workspace(
      (select workspace_id from team_pro_test_state where name = 'hq')
    )
  $$,
  'A lapsed owner can still delete a Team workspace'
);

select * from finish();
rollback;
