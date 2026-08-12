begin;
select plan(18);

select tests.create_supabase_user('referrer_paid', 'referrer-paid@example.com');
select tests.create_supabase_user('referrer_free', 'referrer-free@example.com');
select tests.create_supabase_user('referred_new', 'referred-new@example.com');
select tests.create_supabase_user('referred_other', 'referred-other@example.com');

update public.profiles
set stripe_customer_id = 'cus_referrer_paid'
where id = tests.get_supabase_uid('referrer_paid');

insert into stripe.customers (id) values ('cus_referrer_paid');
insert into stripe.subscriptions (id, customer, status, created)
values (
  'sub_referrer_paid',
  'cus_referrer_paid',
  'active',
  extract(epoch from now())::integer
);

select ok(
  has_function_privilege(
    'authenticated',
    'public.get_or_create_referral_invites()',
    'EXECUTE'
  )
    and has_function_privilege(
      'authenticated',
      'public.claim_referral(text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.get_referral_trial_days()',
      'EXECUTE'
    ),
  'Authenticated users can call the referral-facing RPCs'
);

select ok(
  not has_function_privilege(
    'authenticated',
    'public.prepare_referral_reward(uuid,text)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'public.complete_referral_reward(uuid,text,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.prepare_referral_reward(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.complete_referral_reward(uuid,text,text)',
      'EXECUTE'
    ),
  'Only the billing service can prepare and complete rewards'
);

select tests.authenticate_as('referrer_free');
select results_eq(
  $$select count(*) from public.get_or_create_referral_invites()$$,
  array[0::bigint],
  'Free users do not receive referral slots'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('referrer_paid');

select results_eq(
  $$
    select slot, status, reward_amount_cents, reward_currency
    from public.get_or_create_referral_invites()
  $$,
  $$
    values
      (1::smallint, 'available'::text, 1500, 'usd'::text),
      (2::smallint, 'available'::text, 1500, 'usd'::text),
      (3::smallint, 'available'::text, 1500, 'usd'::text)
  $$,
  'An active paid subscriber receives three available referral slots'
);

select is(
  (select count(*) from public.get_or_create_referral_invites()),
  3::bigint,
  'Fetching referral slots is idempotent'
);

create temporary table referral_test_state as
select slot, code
from public.get_or_create_referral_invites();

select is(
  public.claim_referral((select code from referral_test_state where slot = 1)),
  false,
  'A subscriber cannot claim their own referral'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('referred_new');

select is(
  public.claim_referral((select code from referral_test_state where slot = 1)),
  true,
  'A new account can claim an available referral'
);

select is(
  public.claim_referral((select code from referral_test_state where slot = 1)),
  true,
  'Claiming the same referral again is idempotent'
);

select is(
  public.get_referral_trial_days(),
  30,
  'A referred account receives a 30-day Pro trial'
);

select is(
  public.claim_referral((select code from referral_test_state where slot = 2)),
  false,
  'One account cannot consume multiple referral slots'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('referred_other');

select is(
  public.claim_referral((select code from referral_test_state where slot = 1)),
  false,
  'A claimed referral cannot be consumed by another account'
);

select is(
  public.get_referral_trial_days(),
  null,
  'A non-referred account keeps the standard trial duration'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('referrer_paid');

select is(
  (
    select status
    from public.get_or_create_referral_invites()
    where slot = 1
  ),
  'trial_started',
  'The referrer sees when an invite starts a trial'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select referrer_user_id, referrer_customer_id,
      reward_amount_cents, reward_currency
    from public.prepare_referral_reward(
      tests.get_supabase_uid('referred_new'),
      'in_referralfirst'
    )
  $$,
  $$
    values (
      tests.get_supabase_uid('referrer_paid'),
      'cus_referrer_paid'::text,
      1500,
      'usd'::text
    )
  $$,
  'The first paid invoice prepares a $15 referrer credit'
);

select is(
  (
    select count(*)
    from public.prepare_referral_reward(
      tests.get_supabase_uid('referred_new'),
      'in_referralfirst'
    )
  ),
  1::bigint,
  'Reward preparation retries return the same pending reward'
);

select is(
  public.complete_referral_reward(
    (
      select referral_id
      from public.prepare_referral_reward(
        tests.get_supabase_uid('referred_new'),
        'in_referralfirst'
      )
    ),
    'in_referralfirst',
    'cbtxn_referralreward'
  ),
  true,
  'The billing service can complete the prepared reward'
);

select is(
  (
    select count(*)
    from public.prepare_referral_reward(
      tests.get_supabase_uid('referred_new'),
      'in_referralfirst'
    )
  ),
  0::bigint,
  'A completed reward cannot be prepared again'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('referrer_paid');

select is(
  (
    select status
    from public.get_or_create_referral_invites()
    where slot = 1
  ),
  'reward_earned',
  'The referrer sees when the reward is earned'
);

select * from finish();
rollback;
