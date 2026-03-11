begin;
select plan(11);

select tests.create_supabase_user('pro', 'pro@example.com');
select tests.create_supabase_user('free', 'free@example.com');

update public.profiles
set stripe_customer_id = 'cus_pro'
where id = tests.get_supabase_uid('pro');

insert into stripe.customers (id)
values ('cus_pro')
on conflict (id) do nothing;

insert into stripe.active_entitlements (id, customer, lookup_key)
values ('ent_pro', 'cus_pro', 'hyprnote_pro')
on conflict (id) do nothing;

select results_eq(
  $$select has_table_privilege('supabase_auth_admin', 'public.profiles', 'SELECT')$$,
  array[true],
  'supabase_auth_admin has SELECT privilege on public.profiles'
);

select results_eq(
  $$select count(*) from pg_policies where schemaname = 'public' and tablename = 'profiles' and policyname = 'Allow auth admin to read profiles' and 'supabase_auth_admin' = any(roles)$$,
  array[1::bigint],
  'RLS policy for supabase_auth_admin on profiles is present'
);

select results_eq(
  $$select has_table_privilege('supabase_auth_admin', 'stripe.active_entitlements', 'SELECT')$$,
  array[true],
  'supabase_auth_admin has SELECT privilege on stripe.active_entitlements'
);

select results_eq(
  $$select has_table_privilege('supabase_auth_admin', 'stripe.subscriptions', 'SELECT')$$,
  array[true],
  'supabase_auth_admin has SELECT privilege on stripe.subscriptions'
);

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('pro')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  )::jsonb
  $$,
  array['["hyprnote_pro"]'::jsonb],
  'custom_access_token_hook sets entitlements=["hyprnote_pro"] when hyprnote_pro entitlement exists'
);

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('free')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  )::jsonb
  $$,
  array['[]'::jsonb],
  'custom_access_token_hook sets entitlements=[] when no customer id'
);

select tests.create_supabase_user('other_entitlement', 'other@example.com');

update public.profiles
set stripe_customer_id = 'cus_other'
where id = tests.get_supabase_uid('other_entitlement');

insert into stripe.customers (id)
values ('cus_other')
on conflict (id) do nothing;

insert into stripe.active_entitlements (id, customer, lookup_key)
values ('ent_other', 'cus_other', 'some_other_feature')
on conflict (id) do nothing;

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('other_entitlement')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'entitlements'
  )::jsonb
  $$,
  array['["some_other_feature"]'::jsonb],
  'custom_access_token_hook sets entitlements=["some_other_feature"] for user with other entitlement'
);

select tests.create_supabase_user('trialing', 'trialing@example.com');

update public.profiles
set stripe_customer_id = 'cus_trialing'
where id = tests.get_supabase_uid('trialing');

insert into stripe.customers (id)
values ('cus_trialing')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status, trial_end, created)
values ('sub_trialing', 'cus_trialing', 'trialing', '1738627200', 1000)
on conflict (id) do nothing;

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('trialing')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'subscription_status'
  )::text
  $$,
  array['"trialing"'],
  'custom_access_token_hook sets subscription_status for trialing user'
);

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('trialing')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'trial_end'
  )::text
  $$,
  array['1738627200'],
  'custom_access_token_hook sets trial_end for trialing user'
);

select tests.create_supabase_user('active', 'active@example.com');

update public.profiles
set stripe_customer_id = 'cus_active'
where id = tests.get_supabase_uid('active');

insert into stripe.customers (id)
values ('cus_active')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status, created)
values ('sub_active', 'cus_active', 'active', 2000)
on conflict (id) do nothing;

select results_eq(
  $$
  select (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('active')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'subscription_status'
  )::text
  $$,
  array['"active"'],
  'custom_access_token_hook sets subscription_status for active user'
);

select is(
  (
    public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('free')::text,
        'claims', '{}'::jsonb
      )
    ) -> 'claims' -> 'subscription_status'
  ),
  null,
  'custom_access_token_hook does not set subscription_status for user without subscription'
);

select * from finish();
rollback;
