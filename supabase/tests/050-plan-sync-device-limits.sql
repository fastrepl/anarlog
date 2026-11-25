begin;
select plan(18);

select tests.create_supabase_user('device_pro', 'device-pro@example.com');
select tests.create_supabase_user('device_team', 'device-team@example.com');
select tests.create_supabase_user('device_member', 'device-member@example.com');

select ok(
  not has_function_privilege('authenticated', 'public.get_sync_device_limit(uuid)', 'EXECUTE')
    and not has_function_privilege('anon', 'public.get_sync_device_limit(uuid)', 'EXECUTE')
    and not has_function_privilege('authenticated', 'private.sync_device_limit(uuid)', 'EXECUTE')
    and has_function_privilege('service_role', 'public.get_sync_device_limit(uuid)', 'EXECUTE'),
  'Device allowances can only be read through trusted service code'
);

select is(public.get_sync_device_limit(tests.get_supabase_uid('device_pro')), 255,
  'Personal Pro accounts report the unlimited compatibility ceiling');

select tests.authenticate_as_service_role();
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_pro')), 255,
  'The service role can read the compatibility ceiling');
select public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('device_pro'), 'abcdefghijklmnopqrstuv');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-1');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-2');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-3');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-4');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-5')), true,
  'Pro can connect its fifth device');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-6')), true,
  'Pro can claim a sixth device without hitting a limit');
select results_eq(
  format(
    $$select allowed, device_count from public.register_e2ee_device_enrollment(%L, 'pro-device-7', 'Phone', %L)$$,
    tests.get_supabase_uid('device_pro'),
    rpad('A', 43, 'A')
  ),
  $$values (true, 7::bigint)$$,
  'Enrollment allows additional Pro devices above five'
);
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-8')), true,
  'Pending enrollment does not block additional device claims');

reset role;
insert into public.workspaces (id, owner_user_id, kind, name)
values ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_team'), 'shared', 'Device Team');
insert into public.workspace_memberships (workspace_id, user_id, role)
values
  ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_team'), 'owner'),
  ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_member'), 'member');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 255,
  'Creating an unpaid Team workspace keeps the compatibility ceiling');
select tests.enable_workspace_plan('05000000-0000-4000-8000-000000000001');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 255,
  'Paid Team owners keep the compatibility ceiling');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_member')), 255,
  'Every paid Team member keeps the compatibility ceiling');

select tests.authenticate_as_service_role();
select public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('device_team'), 'abcdefghijklmnopqrstuv');
do $$begin
  for ordinal in 1..4 loop
    perform * from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-' || ordinal::text);
  end loop;
end$$;
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-5')), true,
  'Team can connect its fifth device');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-6')), true,
  'Team can claim a sixth device without hitting a limit');

reset role;
update public.workspace_memberships set deleted_at = now()
where workspace_id = '05000000-0000-4000-8000-000000000001' and user_id = tests.get_supabase_uid('device_member');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_member')), 255,
  'Removed Team members keep the compatibility ceiling');
update stripe.subscriptions set status = 'canceled' where customer = (
  select stripe_customer_id from public.workspaces where id = '05000000-0000-4000-8000-000000000001'
);
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 255,
  'Canceling a Team subscription keeps the compatibility ceiling');
select tests.authenticate_as_service_role();
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-5')), true,
  'A Team cancellation does not disconnect existing devices');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-7')), true,
  'An account can still add another device without a slot cap');
select is((select allowed from public.register_e2ee_device_enrollment(tests.get_supabase_uid('device_team'), 'team-device-8', 'Phone', rpad('B', 43, 'B'), 'team-device-1')), true,
  'Explicit device replacement during enrollment remains supported');
reset role;
select is(private.sync_device_slot_count(tests.get_supabase_uid('device_team')), 7::bigint,
  'Explicit replacement deletes the replaced device');

select * from finish();
rollback;
