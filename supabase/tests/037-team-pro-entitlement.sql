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

select lives_ok(
  $$
    insert into team_pro_test_state (name, workspace_id)
    select 'hq', workspace_id
    from public.create_workspace('Team checkout shell')
  $$,
  'A free account can create a Team checkout shell'
);

select results_eq(
  $$
    select workspace_tier
    from public.get_workspace_access(
      (select workspace_id from team_pro_test_state where name = 'hq')
    )
  $$,
  array['free'::text],
  'A new checkout shell has no paid workspace capabilities'
);

select throws_ok(
  $$
    select *
    from public.rename_workspace(
      (select workspace_id from team_pro_test_state where name = 'hq'),
      'Free Rename'
    )
  $$,
  '42501',
  'workspace capability required: team.manage_workspace',
  'An unpaid workspace cannot use Team management'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('team_pro_owner');

select throws_ok(
  $$
    select *
    from public.create_workspace_invitation(
      (select workspace_id from team_pro_test_state where name = 'hq'),
      'team-free-member@example.com'
    )
  $$,
  '42501',
  'workspace capability required: team.manage_members',
  'Personal Pro cannot invite members into an unpaid workspace'
);

select tests.clear_authentication();
reset role;

select tests.enable_workspace_plan(
  (select workspace_id from team_pro_test_state where name = 'hq')
);

select tests.authenticate_as('team_pro_owner');

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
  'The workspace Team plan lets a free owner invite members'
);

select tests.clear_authentication();
select tests.authenticate_as('team_pro_owner');

select lives_ok(
  $$
    with resent as (
      select *
      from public.resend_workspace_invitation(
        (select invitation_id from team_pro_test_state where name = 'member_invite')
      )
    )
    update team_pro_test_state as state
    set
      invitation_id = resent.invitation_id,
      invite_token = resent.invite_token
    from resent
    where state.name = 'member_invite'
  $$,
  'The workspace Team plan lets a free owner resend invitations'
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
  'Resend replaces the original pending invitation atomically'
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
reset role;

update stripe.subscriptions
set status = 'canceled'::stripe.subscription_status
where customer = (
  select workspace.stripe_customer_id
  from public.workspaces as workspace
  where workspace.id = (select workspace_id from team_pro_test_state where name = 'hq')
);

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
