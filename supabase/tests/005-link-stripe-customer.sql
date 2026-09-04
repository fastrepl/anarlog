begin;
select plan(9);

insert into stripe.customers (id, email)
values ('cus_existing', 'existing@example.com')
on conflict (id) do nothing;

select tests.create_supabase_user('with_stripe', 'existing@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('with_stripe')$$,
  array['cus_existing'::text],
  'New user with matching stripe email gets auto-linked'
);

insert into stripe.customers (id, email)
values ('cus_mixed_case', 'Mixed.Case@Example.com')
on conflict (id) do nothing;

select tests.create_supabase_user('mixed_case', 'mixed.case@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('mixed_case')$$,
  array['cus_mixed_case'::text],
  'New user links a Stripe customer with differently cased email'
);

insert into stripe.customers (id, email, created)
values
  ('cus_duplicate_empty', 'duplicate@example.com', 100),
  ('cus_duplicate_paid', 'DUPLICATE@example.com', 200)
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_duplicate_paid', 'cus_duplicate_paid', 'active')
on conflict (id) do nothing;

select tests.create_supabase_user('duplicate_customer', 'duplicate@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('duplicate_customer')$$,
  array['cus_duplicate_paid'::text],
  'New user prefers the matching customer with an active subscription'
);

insert into stripe.customers (id, email, created)
values
  ('cus_old_canceled', 'paused@example.com', 100),
  ('cus_paused', 'PAUSED@example.com', 200)
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values
  ('sub_old_canceled', 'cus_old_canceled', 'canceled'),
  ('sub_paused', 'cus_paused', 'paused')
on conflict (id) do nothing;

select tests.create_supabase_user('paused_customer', 'paused@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('paused_customer')$$,
  array['cus_paused'::text],
  'New user prefers a resumable paused subscription over canceled history'
);

insert into stripe.customers (id, email, deleted)
values
  ('cus_deleted', 'deleted@example.com', true),
  ('cus_not_deleted', 'DELETED@example.com', false)
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values ('sub_deleted', 'cus_deleted', 'active')
on conflict (id) do nothing;

select tests.create_supabase_user('deleted_customer', 'deleted@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('deleted_customer')$$,
  array['cus_not_deleted'::text],
  'New user does not link a deleted Stripe customer'
);

select tests.create_supabase_user('workspace_owner', 'workspace-owner@example.com');

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('workspace_owner');

select tests.authenticate_as_hyprnote_pro('workspace_owner');

create temporary table workspace_linking_test_state as
select workspace_id
from public.create_workspace('Workspace billing owner');

select tests.clear_authentication();
reset role;

insert into stripe.customers (id, email)
values ('cus_workspace_billing', 'workspace-billing@example.com')
on conflict (id) do nothing;

update public.workspaces
set stripe_customer_id = 'cus_workspace_billing'
where id = (select workspace_id from workspace_linking_test_state);

select tests.create_supabase_user('workspace_collision', 'workspace-billing@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('workspace_collision')$$,
  array[null::text],
  'New user does not claim a customer already linked to workspace billing'
);

select tests.create_supabase_user('no_stripe', 'new@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('no_stripe')$$,
  array[null::text],
  'New user without matching stripe email has null stripe_customer_id'
);

insert into stripe.customers (id, email)
values ('cus_updated', 'updated@example.com')
on conflict (id) do nothing;

update auth.users
set email = 'updated@example.com'
where id = tests.get_supabase_uid('no_stripe');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('no_stripe')$$,
  array['cus_updated'::text],
  'Email update triggers stripe customer linking'
);

update auth.users
set email = 'changed@example.com'
where id = tests.get_supabase_uid('with_stripe');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('with_stripe')$$,
  array['cus_existing'::text],
  'Email update does not change existing stripe_customer_id'
);

select * from finish();
rollback;
