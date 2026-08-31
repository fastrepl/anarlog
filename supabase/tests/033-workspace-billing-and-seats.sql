begin;
select plan(14);

select tests.create_supabase_user('seat_owner', 'seat-owner@example.com');
select tests.create_supabase_user('seat_member', 'seat-member@example.com');
select tests.create_supabase_user('seat_extra', 'seat-extra@example.com');

create temporary table workspace_billing_test_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text
);

grant all on workspace_billing_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('seat_owner'),
  tests.get_supabase_uid('seat_member'),
  tests.get_supabase_uid('seat_extra')
);

select tests.authenticate_as_hyprnote_pro('seat_owner');

select lives_ok(
  $$
    insert into workspace_billing_test_state (name, workspace_id)
    select 'hq', workspace_id from public.create_workspace('Seat HQ')
  $$,
  'The owner creates a shared workspace'
);

select results_eq(
  $$
    select seat_limit, used_seats, is_billed
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_test_state where name = 'hq')
    )
  $$,
  $$values (null::integer, 1, false)$$,
  'A new workspace is unbilled, unlimited, and consumes one seat for its owner'
);

select tests.clear_authentication();
select tests.authenticate_as('seat_member');

select throws_ok(
  $$
    select * from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_test_state where name = 'hq')
    )
  $$,
  '42501',
  'workspace billing operation not permitted',
  'Non-members cannot read workspace seat usage'
);

select tests.clear_authentication();
reset role;

-- Billing is provisioned server-side after checkout completes.
update public.workspaces
set stripe_customer_id = 'cus_seat_team', seat_limit = 2
where id = (select workspace_id from workspace_billing_test_state where name = 'hq');

insert into stripe.customers (id)
values ('cus_seat_team')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_seat_team', 'cus_seat_team', 'active'::stripe.subscription_status)
on conflict (id) do nothing;

insert into stripe.active_entitlements (id, customer, lookup_key)
values
  ('ent_seat_pro', 'cus_seat_team', 'hyprnote_pro'),
  ('ent_seat_team', 'cus_seat_team', 'hyprnote_team')
on conflict (id) do nothing;

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('seat_owner');

select results_eq(
  $$
    select seat_limit, used_seats, is_billed
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_test_state where name = 'hq')
    )
  $$,
  $$values (2, 1, true)$$,
  'Managers see the purchased seat count and current usage'
);

select lives_ok(
  $$
    insert into workspace_billing_test_state (name, invitation_id, invite_token)
    select 'member_invite', invitation_id, invite_token
    from public.create_workspace_invitation(
      (select workspace_id from workspace_billing_test_state where name = 'hq'),
      'seat-member@example.com'
    )
  $$,
  'An invitation fits within the purchased seats'
);

select results_eq(
  $$
    select used_seats
    from public.get_workspace_seat_usage(
      (select workspace_id from workspace_billing_test_state where name = 'hq')
    )
  $$,
  array[2],
  'A pending invitation holds a seat so it cannot be oversubscribed'
);

select throws_ok(
  $$
    select * from public.create_workspace_invitation(
      (select workspace_id from workspace_billing_test_state where name = 'hq'),
      'seat-extra@example.com'
    )
  $$,
  '22023',
  'workspace seat limit reached',
  'Inviting past the purchased seats is refused'
);

select tests.clear_authentication();
select tests.authenticate_as('seat_member');

select lives_ok(
  $$
    select * from public.accept_workspace_invitation(
      (select invitation_id from workspace_billing_test_state where name = 'member_invite'),
      (select invite_token from workspace_billing_test_state where name = 'member_invite')
    )
  $$,
  'Accepting a held seat converts the invitation into a membership'
);

select results_eq(
  $$
    select count(*)
    from public.workspaces
    where id = (select workspace_id from workspace_billing_test_state where name = 'hq')
  $$,
  array[1::bigint],
  'The member sees the workspace they joined'
);

select tests.clear_authentication();
reset role;

-- A resumable personal trial must not hide an effective workspace plan.
update public.profiles
set stripe_customer_id = 'cus_seat_member_paused'
where id = tests.get_supabase_uid('seat_member');

insert into stripe.customers (id)
values ('cus_seat_member_paused')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_seat_member_paused', 'cus_seat_member_paused', 'paused'::stripe.subscription_status)
on conflict (id) do nothing;

select results_eq(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('seat_member')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  $$,
  array['["hyprnote_pro", "hyprnote_team"]'::jsonb],
  'A member inherits the workspace capabilities without buying their own plan'
);

select results_eq(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('seat_member')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' ->> 'subscription_status'
  $$,
  array['active'::text],
  'An otherwise unsubscribed member reads as covered by the workspace plan'
);

select results_eq(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('seat_extra')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  $$,
  array['[]'::jsonb],
  'Someone outside the workspace inherits nothing'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('seat_owner');

select lives_ok(
  $$
    select * from public.revoke_workspace_membership(
      (select workspace_id from workspace_billing_test_state where name = 'hq'),
      tests.get_supabase_uid('seat_member')
    )
  $$,
  'The owner frees a seat by revoking the member'
);

select tests.clear_authentication();
reset role;

select results_eq(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('seat_member')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  $$,
  array['[]'::jsonb],
  'A removed member immediately stops inheriting the workspace entitlement'
);

select * from finish();
rollback;
