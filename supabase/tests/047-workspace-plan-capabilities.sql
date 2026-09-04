begin;
select plan(18);

select tests.create_supabase_user(
  'capability_owner',
  'capability-owner@example.com'
);

create temporary table workspace_capability_test_state (
  workspace_id uuid primary key,
  share_id uuid
);

grant all on workspace_capability_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('capability_owner');

select tests.authenticate_as_hyprnote_pro('capability_owner');

insert into workspace_capability_test_state (workspace_id)
select workspace_id from public.create_workspace('Capability workspace');

select results_eq(
  $$
    select workspace_tier
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  array['free'::text],
  'Personal Pro does not change the workspace tier'
);

select throws_ok(
  $$
    select * from public.rename_workspace(
      (select workspace_id from workspace_capability_test_state),
      'Personal Pro workspace'
    )
  $$,
  '42501',
  'workspace capability required: team.manage_workspace',
  'Personal Pro cannot manage an unpaid shared workspace'
);

select throws_ok(
  $$
    select * from public.create_session_share(
      (select workspace_id from workspace_capability_test_state),
      'session-personal-pro'
    )
  $$,
  '42501',
  'workspace capability required: team.shared_notes',
  'Personal Pro cannot publish from an unpaid shared workspace'
);

select tests.clear_authentication();
reset role;

insert into stripe.customers (id)
values ('cus_capability_workspace')
on conflict (id) do nothing;

update public.workspaces
set
  stripe_customer_id = 'cus_capability_workspace',
  seat_limit = 3
where id = (select workspace_id from workspace_capability_test_state);

insert into stripe.subscriptions (id, customer, status)
values (
  'sub_capability_workspace',
  'cus_capability_workspace',
  'active'::stripe.subscription_status
)
on conflict (id) do nothing;

insert into stripe.active_entitlements (id, customer, lookup_key)
values (
  'ent_capability_pro',
  'cus_capability_workspace',
  'hyprnote_pro'
)
on conflict (customer, lookup_key) do nothing;

select tests.authenticate_as('capability_owner');

select results_eq(
  $$
    select workspace_tier
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  array['free'::text],
  'A generic Pro feature on the workspace is not a Team entitlement'
);

select throws_ok(
  $$
    select * from public.rename_workspace(
      (select workspace_id from workspace_capability_test_state),
      'Generic Pro workspace'
    )
  $$,
  '42501',
  'workspace capability required: team.manage_workspace',
  'An active subscription without the Team feature stays locked'
);

select tests.clear_authentication();
reset role;

insert into stripe.active_entitlements (id, customer, lookup_key)
values (
  'ent_capability_team',
  'cus_capability_workspace',
  'hyprnote_team'
)
on conflict (customer, lookup_key) do nothing;

select tests.authenticate_as('capability_owner');

select results_eq(
  $$
    select
      workspace_tier,
      'team.manage_members' = any (capabilities),
      'team.manage_policies' = any (capabilities),
      'team.view_usage' = any (capabilities),
      'team.custom_subdomain' = any (capabilities),
      'enterprise.sso' = any (capabilities),
      seat_limit,
      used_seats
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  $$ values ('team'::text, true, false, false, false, false, 3, 1) $$,
  'Team returns collaboration capabilities without Enterprise Admin controls'
);

select results_eq(
  $$
    select workspace_name from public.rename_workspace(
      (select workspace_id from workspace_capability_test_state),
      'Paid Team workspace'
    )
  $$,
  array['Paid Team workspace'::text],
  'Team capability unlocks workspace management'
);

select lives_ok(
  $$
    select * from public.create_session_share(
      (select workspace_id from workspace_capability_test_state),
      'session-team'
    )
  $$,
  'Team capability unlocks shared note publication'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update workspace_capability_test_state
set share_id = (
  select id
  from public.session_shares
  where session_id = 'session-team'
);

select tests.clear_authentication();
select tests.authenticate_as('capability_owner');

select throws_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_capability_test_state),
      'team-domain'
    )
  $$,
  '42501',
  'workspace capability required: team.custom_subdomain',
  'Team cannot claim a workspace sharing subdomain'
);

select throws_ok(
  $$
    select * from public.get_workspace_usage_overview(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  '42501',
  'workspace capability required: team.view_usage',
  'Team cannot read the Enterprise Admin usage report'
);

select results_eq(
  $$
    select workspace_share_slug
    from public.get_session_share_workspace_slug(
      (select share_id from workspace_capability_test_state)
    )
  $$,
  $$values (null::text)$$,
  'Team share links use the canonical apex instead of a retained subdomain'
);

select throws_ok(
  $$
    select * from public.claim_workspace_domain(
      (select workspace_id from workspace_capability_test_state),
      'example.com'
    )
  $$,
  '42501',
  'workspace capability required: enterprise.sso',
  'Team cannot use Enterprise SSO controls'
);

select throws_ok(
  $$
    select * from public.set_workspace_policy(
      (select workspace_id from workspace_capability_test_state),
      array['restricted', 'workspace'],
      'restricted',
      30,
      true,
      true,
      false
    )
  $$,
  '42501',
  'workspace capability required: team.manage_policies',
  'Team cannot change Enterprise Admin policies'
);

select tests.clear_authentication();
reset role;

insert into stripe.active_entitlements (id, customer, lookup_key)
values (
  'ent_capability_enterprise',
  'cus_capability_workspace',
  'hyprnote_enterprise'
)
on conflict (customer, lookup_key) do nothing;

select tests.authenticate_as('capability_owner');

select results_eq(
  $$
    select
      workspace_tier,
      'team.manage_workspace' = any (capabilities),
      'team.manage_policies' = any (capabilities),
      'team.view_usage' = any (capabilities),
      'team.custom_subdomain' = any (capabilities),
      'enterprise.capture' = any (capabilities),
      'enterprise.retention' = any (capabilities)
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  $$ values ('enterprise'::text, true, true, true, true, true, true) $$,
  'Enterprise inherits Team and adds every Admin capability'
);

select lives_ok(
  $$
    select * from public.claim_workspace_domain(
      (select workspace_id from workspace_capability_test_state),
      'example.com'
    );
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_capability_test_state),
      'enterprise-domain'
    );
    select * from public.get_workspace_usage_overview(
      (select workspace_id from workspace_capability_test_state)
    );
    select * from public.set_workspace_policy(
      (select workspace_id from workspace_capability_test_state),
      array['restricted', 'workspace'],
      'restricted',
      30,
      true,
      true,
      true
    )
  $$,
  'Enterprise unlocks sharing domain, usage, SSO, and policy controls'
);

select tests.clear_authentication();
reset role;

delete from stripe.active_entitlements
where id = 'ent_capability_enterprise';

select tests.authenticate_as('capability_owner');

select results_eq(
  $$
    select
      workspace_tier,
      public.workspace_share_slug_is_active('enterprise-domain')
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  $$ values ('team'::text, false) $$,
  'Downgrading to Team immediately deactivates the retained sharing domain'
);

select results_eq(
  $$
    select workspace_share_slug
    from public.get_session_share_workspace_slug(
      (select share_id from workspace_capability_test_state)
    )
  $$,
  $$values (null::text)$$,
  'Downgraded Team links fall back to the canonical apex'
);

select tests.clear_authentication();
reset role;

update stripe.subscriptions
set status = 'canceled'::stripe.subscription_status
where id = 'sub_capability_workspace';

select tests.authenticate_as('capability_owner');

select results_eq(
  $$
    select
      workspace_tier,
      public.email_requires_sso('person@example.com')
    from public.get_workspace_access(
      (select workspace_id from workspace_capability_test_state)
    )
  $$,
  $$ values ('free'::text, false) $$,
  'Canceled billing removes capabilities and stops Enterprise SSO enforcement'
);

select * from finish();
rollback;
