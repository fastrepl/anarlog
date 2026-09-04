begin;
select plan(6);

select ok(
  to_regclass('private.account_analytics_outbox') is not null,
  'Account analytics outbox remains private during migration cleanup'
);

select ok(
  not has_table_privilege(
    'anon',
    'private.account_analytics_outbox',
    'SELECT'
  )
    and not has_table_privilege(
      'authenticated',
      'private.account_analytics_outbox',
      'SELECT'
    ),
  'Client roles cannot read the retired account analytics outbox'
);

select is(
  (
    select count(*)
    from pg_trigger
    where tgrelid = 'auth.users'::regclass
      and tgname in (
        'on_auth_user_account_analytics_created',
        'on_auth_user_account_analytics_confirmed'
      )
      and not tgisinternal
  ),
  0::bigint,
  'Auth-user analytics triggers are removed'
);

select lives_ok(
  $$
    select private.enqueue_account_analytics_event(
      'account_created',
      gen_random_uuid(),
      clock_timestamp(),
      'private@example.com',
      'email',
      false
    )
  $$,
  'Retired enqueue calls remain backward compatible'
);

select results_eq(
  $$ select count(*) from private.account_analytics_outbox $$,
  array[0::bigint],
  'Retired enqueue calls do not retain identity data'
);

select results_eq(
  $$
    select count(*)
    from public.claim_account_analytics_events(gen_random_uuid(), 500, 300)
  $$,
  array[0::bigint],
  'No account identity events can be claimed for delivery'
);

select * from finish();
rollback;
