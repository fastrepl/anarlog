begin;
select plan(4);

insert into stripe.customers (id, email)
values ('cus_existing', 'existing@example.com')
on conflict (id) do nothing;

select tests.create_supabase_user('with_stripe', 'existing@example.com');

select results_eq(
  $$select stripe_customer_id from public.profiles where id = tests.get_supabase_uid('with_stripe')$$,
  array['cus_existing'::text],
  'New user with matching stripe email gets auto-linked'
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
