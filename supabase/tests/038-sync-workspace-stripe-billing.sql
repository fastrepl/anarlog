begin;
select plan(12);

select tests.create_supabase_user('billing_owner', 'billing-owner@example.com');

create temporary table workspace_billing_sync_test_state (
  name text primary key,
  workspace_id uuid
);

grant all on workspace_billing_sync_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('billing_owner');

select ok(
  has_function_privilege(
    'service_role',
    'public.sync_workspace_stripe_billing(uuid,text,integer,boolean)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'public.sync_workspace_stripe_billing(uuid,text,integer,boolean)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.sync_workspace_stripe_billing(uuid,text,integer,boolean)',
      'EXECUTE'
    ),
  'Only the service role can synchronize workspace billing'
);

select tests.authenticate_as_hyprnote_pro('billing_owner');

insert into workspace_billing_sync_test_state (name, workspace_id)
select 'hq', workspace_id from public.create_workspace('Billing HQ');

select throws_ok(
  $$
    select * from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_team123',
      null,
      false
    )
  $$,
  '42501',
  'permission denied for function sync_workspace_stripe_billing',
  'Workspace managers cannot impersonate the billing service'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_team123',
      null,
      false
    )
  $$,
  $$values ('cus_team123'::text, null::integer)$$,
  'The service binds a Stripe customer without inventing a seat quantity'
);

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_team123',
      3,
      true
    )
  $$,
  $$values ('cus_team123'::text, 3)$$,
  'A matching subscription reconciles its positive seat quantity'
);

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_other456',
      9,
      true
    )
  $$,
  $$values ('cus_team123'::text, 3)$$,
  'A different Stripe customer cannot reassign or resize the workspace'
);

select throws_ok(
  $$
    select * from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_team123',
      0,
      true
    )
  $$,
  '22023',
  'invalid workspace Stripe billing update',
  'Stripe cannot reduce the workspace to a non-positive seat quantity'
);

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      tests.get_supabase_uid('billing_owner'),
      'cus_personal123',
      2,
      true
    )
  $$,
  $$values (null::text, null::integer)$$,
  'Personal workspaces cannot become Team billing owners'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('billing_owner');

select results_eq(
  $$
    select seat_limit, used_seats, is_billed
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq')
    )
  $$,
  $$values (3, 1, false)$$,
  'A customer binding alone does not report an active Team subscription'
);

select tests.clear_authentication();
reset role;

insert into stripe.customers (id)
values ('cus_team123')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_team123', 'cus_team123', 'active'::stripe.subscription_status)
on conflict (id) do nothing;

select tests.authenticate_as_hyprnote_pro('billing_owner');

select results_eq(
  $$
    select seat_limit, used_seats, is_billed
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq')
    )
  $$,
  $$values (3, 1, true)$$,
  'An active Team subscription reports the workspace as billed'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq'),
      'cus_team123',
      null,
      true
    )
  $$,
  $$values ('cus_team123'::text, null::integer)$$,
  'A canceled Team subscription clears the purchased seat limit'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('billing_owner');

select results_eq(
  $$
    select seat_limit, used_seats, is_billed
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_sync_test_state where name = 'hq')
    )
  $$,
  $$values (null::integer, 1, true)$$,
  'Clearing the limit removes the cap without changing subscription truth'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select assigned_customer_id, seat_limit
    from public.sync_workspace_stripe_billing(
      '00000000-0000-4000-8000-000000000999'::uuid,
      'cus_missing123',
      null,
      false
    )
  $$,
  $$values (null::text, null::integer)$$,
  'A missing workspace is not provisioned implicitly'
);

select * from finish();
rollback;
