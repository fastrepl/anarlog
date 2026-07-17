begin;
select plan(7);

select tests.create_supabase_user('owner', 'owner@example.com');
select tests.create_supabase_user('other', 'other@example.com');

select tests.authenticate_as('owner');

select results_eq(
  $$select count(*) from profiles where id = auth.uid()$$,
  array[1::bigint],
  'Owner can view own profile (auto-created by trigger)'
);

select ok(
  not has_table_privilege('authenticated', 'public.profiles', 'INSERT')
    and not has_table_privilege('authenticated', 'public.profiles', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.profiles', 'DELETE'),
  'Authenticated users cannot mutate server-owned billing profiles'
);

select throws_ok(
  $$update profiles set stripe_customer_id = 'cus_forged' where id = auth.uid()$$,
  '42501',
  'permission denied for table profiles',
  'An owner cannot forge a Stripe customer mapping'
);

select tests.clear_authentication();
select tests.authenticate_as('other');

select results_eq(
  $$select count(*) from profiles where id = tests.get_supabase_uid('owner')$$,
  array[0::bigint],
  'Other user cannot view owner profile'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $$update profiles set stripe_customer_id = 'cus_service_owned' where id = tests.get_supabase_uid('owner')$$,
  'Service code can update a billing profile'
);

select ok(
  exists (
    select 1
    from pg_index
    where indexrelid = 'public.profiles_stripe_customer_id_unique'::regclass
      and indisunique
      and indpred is not null
  ),
  'Stripe customer ownership is unique across non-null profile mappings'
);

select results_eq(
  $$select count(*) from profiles where id in (tests.get_supabase_uid('owner'), tests.get_supabase_uid('other'))$$,
  array[2::bigint],
  'Service role can view all test profiles'
);

select * from finish();
rollback;
