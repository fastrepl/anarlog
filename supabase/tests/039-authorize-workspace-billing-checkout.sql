begin;
select plan(8);

select tests.create_supabase_user('checkout_owner', 'checkout-owner@example.com');
select tests.create_supabase_user('checkout_admin', 'checkout-admin@example.com');
select tests.create_supabase_user('checkout_member', 'checkout-member@example.com');
select tests.create_supabase_user('checkout_outsider', 'checkout-outsider@example.com');

create temporary table workspace_checkout_test_state (
  workspace_id uuid primary key
);

grant all on workspace_checkout_test_state to authenticated;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('checkout_owner'),
  tests.get_supabase_uid('checkout_admin'),
  tests.get_supabase_uid('checkout_member'),
  tests.get_supabase_uid('checkout_outsider')
);

select ok(
  has_function_privilege(
    'authenticated',
    'public.get_workspace_billing_checkout_context(uuid)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'anon',
      'public.get_workspace_billing_checkout_context(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'public.get_workspace_billing_checkout_context(uuid)',
      'EXECUTE'
    ),
  'Only authenticated users can request a workspace checkout context'
);

select tests.authenticate_as_hyprnote_pro('checkout_owner');

insert into workspace_checkout_test_state (workspace_id)
select workspace_id from public.create_workspace('Checkout HQ');

select tests.clear_authentication();
reset role;

insert into public.workspace_memberships (workspace_id, user_id, role)
values
  (
    (select workspace_id from workspace_checkout_test_state),
    tests.get_supabase_uid('checkout_admin'),
    'admin'
  ),
  (
    (select workspace_id from workspace_checkout_test_state),
    tests.get_supabase_uid('checkout_member'),
    'member'
  );

update public.workspaces
set stripe_customer_id = 'cus_checkout123'
where id = (select workspace_id from workspace_checkout_test_state);

select tests.authenticate_as_hyprnote_pro('checkout_owner');

select results_eq(
  $$
    select workspace_name, stripe_customer_id, used_seats
    from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  $$values ('Checkout HQ'::text, 'cus_checkout123'::text, 3)$$,
  'The owner receives the workspace billing identity and live seat floor'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('checkout_admin');

select results_eq(
  $$
    select workspace_name, stripe_customer_id, used_seats
    from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  $$values ('Checkout HQ'::text, 'cus_checkout123'::text, 3)$$,
  'An active workspace admin can start billing checkout'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('checkout_member');

select throws_ok(
  $$
    select * from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'A regular member cannot start workspace billing checkout'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('checkout_outsider');

select throws_ok(
  $$
    select * from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'A non-member cannot read workspace billing context'
);

select throws_ok(
  $$
    select * from public.get_workspace_billing_checkout_context(
      tests.get_supabase_uid('checkout_outsider')
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'A personal workspace cannot be used for Team checkout'
);

select tests.clear_authentication();
reset role;

update public.workspace_memberships
set deleted_at = now()
where workspace_id = (select workspace_id from workspace_checkout_test_state)
  and user_id = tests.get_supabase_uid('checkout_admin');

select tests.authenticate_as_hyprnote_pro('checkout_admin');

select throws_ok(
  $$
    select * from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'A removed admin immediately loses checkout authority'
);

select tests.clear_authentication();
reset role;

update public.workspaces
set deleted_at = now()
where id = (select workspace_id from workspace_checkout_test_state);

select tests.authenticate_as_hyprnote_pro('checkout_owner');

select throws_ok(
  $$
    select * from public.get_workspace_billing_checkout_context(
      (select workspace_id from workspace_checkout_test_state)
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'A deleted workspace cannot start checkout'
);

select * from finish();
rollback;
