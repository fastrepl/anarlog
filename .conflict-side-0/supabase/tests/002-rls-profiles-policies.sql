begin;
select plan(4);

select tests.create_supabase_user('owner', 'owner@example.com');
select tests.create_supabase_user('other', 'other@example.com');

select tests.authenticate_as('owner');

select results_eq(
  $$select count(*) from profiles where id = auth.uid()$$,
  array[1::bigint],
  'Owner can view own profile (auto-created by trigger)'
);

select lives_ok(
  $$update profiles set stripe_customer_id = 'cus_test' where id = auth.uid()$$,
  'Owner can update own profile'
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

select results_eq(
  $$select count(*) from profiles where id in (tests.get_supabase_uid('owner'), tests.get_supabase_uid('other'))$$,
  array[2::bigint],
  'Service role can view all test profiles'
);

select * from finish();
rollback;
