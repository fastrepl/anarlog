begin;
select plan(5);

select tests.create_supabase_user('team_split_owner', 'team-split-owner@example.com');

create temporary table team_plan_split_test_state (
  workspace_id uuid primary key
);

grant all on team_plan_split_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('team_split_owner');

select tests.authenticate_as('team_split_owner');

select lives_ok(
  $$
    insert into team_plan_split_test_state (workspace_id)
    select workspace_id from public.create_workspace('Team checkout shell')
  $$,
  'A signed-in account can create an unbilled Team workspace'
);

select throws_ok(
  $$
    select * from public.rename_workspace(
      (select workspace_id from team_plan_split_test_state),
      'Still unbilled'
    )
  $$,
  '42501',
  'active Team subscription required',
  'An unbilled workspace cannot use Team management controls'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('team_split_owner');

select throws_ok(
  $$
    select * from public.rename_workspace(
      (select workspace_id from team_plan_split_test_state),
      'Personal Pro is separate'
    )
  $$,
  '42501',
  'active Team subscription required',
  'Personal Pro does not unlock Team management controls'
);

select tests.clear_authentication();
reset role;

update public.workspaces
set stripe_customer_id = 'cus_team_split', seat_limit = 1
where id = (select workspace_id from team_plan_split_test_state);

insert into stripe.customers (id)
values ('cus_team_split')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_team_split', 'cus_team_split', 'active'::stripe.subscription_status)
on conflict (id) do nothing;

select tests.authenticate_as('team_split_owner');

select results_eq(
  $$
    select workspace_name from public.rename_workspace(
      (select workspace_id from team_plan_split_test_state),
      'Paid Team'
    )
  $$,
  array['Paid Team'::text],
  'An active Team subscription unlocks workspace management without personal Pro'
);

select throws_ok(
  $$
    select * from public.create_workspace_invitation(
      (select workspace_id from team_plan_split_test_state),
      'second-seat@example.com'
    )
  $$,
  '22023',
  'workspace seat limit reached',
  'Team management still enforces the purchased seat count'
);

select * from finish();
rollback;
