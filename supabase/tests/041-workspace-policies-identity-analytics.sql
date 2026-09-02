begin;
select plan(14);

select tests.create_supabase_user('policy_owner', 'policy-owner@example.com');
select tests.create_supabase_user('policy_member', 'policy-member@example.com');
select tests.create_supabase_user('policy_outsider', 'policy-outsider@example.com');

create temporary table workspace_policy_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid
);

grant all on workspace_policy_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('policy_owner'),
  tests.get_supabase_uid('policy_member'),
  tests.get_supabase_uid('policy_outsider')
);

select tests.authenticate_as_hyprnote_pro('policy_owner');

select lives_ok(
  $$
    insert into workspace_policy_test_state (name, workspace_id)
    select 'hq', workspace_id from public.create_workspace('Policy HQ')
  $$,
  'The owner creates a shared workspace'
);

select tests.enable_workspace_plan(
  (select workspace_id from workspace_policy_test_state where name = 'hq'),
  'enterprise'
);

select results_eq(
  $$
    select default_share_scope, retention_days, model_training_opt_out
    from public.get_workspace_policy(
      (select workspace_id from workspace_policy_test_state where name = 'hq')
    )
  $$,
  $$values ('restricted'::text, null::integer, true)$$,
  'A new workspace has default-off public sharing extras and training opt-out'
);

select lives_ok(
  $$
    select * from public.set_workspace_policy(
      (select workspace_id from workspace_policy_test_state where name = 'hq'),
      array['restricted', 'workspace']::text[],
      'restricted',
      30,
      true,
      true,
      false
    )
  $$,
  'An admin can disable public and link sharing and set a retention window'
);

select lives_ok(
  $$
    insert into workspace_policy_test_state (name, share_id)
    select 'share', share_id
    from public.create_session_share(
      (select workspace_id from workspace_policy_test_state where name = 'hq'),
      'session-policy-1'
    )
  $$,
  'The owner can still create a restricted share'
);

select throws_ok(
  $$
    select * from public.set_session_share_scope(
      (select share_id from workspace_policy_test_state where name = 'share'),
      'public',
      null
    )
  $$,
  '42501',
  'workspace policy forbids this share scope',
  'Public sharing is rejected after the org policy disables it'
);

select throws_ok(
  $$
    select * from public.enable_session_share_link(
      (select share_id from workspace_policy_test_state where name = 'share')
    )
  $$,
  '42501',
  'workspace policy forbids this share scope',
  'Link sharing is rejected after the org policy disables it'
);

select results_eq(
  $$
    select member_count, used_seats
    from public.get_workspace_usage_overview(
      (select workspace_id from workspace_policy_test_state where name = 'hq')
    )
  $$,
  $$values (1, 1)$$,
  'Admins see member and seat counts without reading note content'
);

select tests.clear_authentication();
select tests.authenticate_as('policy_outsider');

select throws_ok(
  $$
    select * from public.get_workspace_policy(
      (select workspace_id from workspace_policy_test_state where name = 'hq')
    )
  $$,
  '42501',
  'workspace policy operation not permitted',
  'Non-members cannot read workspace policies'
);

select tests.clear_authentication();
reset role;

select lives_ok(
  $$
    insert into public.workspace_memberships (workspace_id, user_id, role)
    values (
      (select workspace_id from workspace_policy_test_state where name = 'hq'),
      tests.get_supabase_uid('policy_member'),
      'member'
    )
  $$,
  'The owner can add a member before checking client policy reads'
);

select tests.clear_authentication();
select tests.authenticate_as('policy_member');

select results_eq(
  $$
    select allowed_share_scopes
    from public.get_workspace_policy(
      (select workspace_id from workspace_policy_test_state where name = 'hq')
    )
  $$,
  $$values (array['restricted', 'workspace']::text[])$$,
  'Members can read allowed share scopes so clients honor org policy'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('policy_owner');

select lives_ok(
  $$
    select * from public.rotate_workspace_scim_token(
      (select workspace_id from workspace_policy_test_state where name = 'hq'),
      'example.com',
      'scim-token-0123456789abcdef0123456789abcdef'
    )
  $$,
  'An admin can install a SCIM token for the workspace'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as_service_role();

select lives_ok(
  $$
    select * from public.scim_apply_user(
      'scim-token-0123456789abcdef0123456789abcdef',
      'policy-member@example.com',
      true
    )
  $$,
  'SCIM provisioning adds the IdP user to the workspace'
);

select lives_ok(
  $$
    select * from public.scim_apply_user(
      'scim-token-0123456789abcdef0123456789abcdef',
      'policy-member@example.com',
      false
    )
  $$,
  'SCIM deprovisioning revokes workspace membership'
);

select is(
  (
    select count(*)
    from public.workspace_memberships
    where workspace_id = (select workspace_id from workspace_policy_test_state where name = 'hq')
      and user_id = tests.get_supabase_uid('policy_member')
      and deleted_at is null
  ),
  0::bigint,
  'Deprovisioned members have no active workspace membership'
);

select * from finish();
rollback;
