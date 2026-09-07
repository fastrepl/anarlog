begin;
select plan(19);

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

select is(public.get_sync_device_limit(tests.get_supabase_uid('device_pro')), 3,
  'Personal accounts have three included slots');

select tests.authenticate_as_service_role();
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_pro')), 3,
  'The service role can read the authoritative allowance');
select public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('device_pro'), 'abcdefghijklmnopqrstuv');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-1');
select * from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-2');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-3')), true,
  'Pro can connect its third device');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-4')), false,
  'Pro cannot claim a fourth device');
select is((select allowed from public.register_e2ee_device_enrollment(tests.get_supabase_uid('device_pro'), 'pro-device-4', 'Phone', rpad('A', 43, 'A'))), false,
  'Enrollment enforces the same Pro limit as credential claims');
select public.remove_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-3');
select results_eq(
  $$select allowed, device_count from public.register_e2ee_device_enrollment(tests.get_supabase_uid('device_pro'), 'pro-device-4', 'Phone', rpad('A', 43, 'A'))$$,
  $$values (true, 3::bigint)$$,
  'A pending approval reserves the last Pro slot');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_pro'), 'pro-device-5')), false,
  'Credential claims cannot consume a slot already reserved by enrollment');

reset role;
insert into public.workspaces (id, owner_user_id, kind, name)
values ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_team'), 'shared', 'Device Team');
insert into public.workspace_memberships (workspace_id, user_id, role)
values
  ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_team'), 'owner'),
  ('05000000-0000-4000-8000-000000000001', tests.get_supabase_uid('device_member'), 'member');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 3,
  'Creating an unpaid Team workspace does not grant extra devices');
select tests.enable_workspace_plan('05000000-0000-4000-8000-000000000001');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 5,
  'Paid Team owners have five included slots');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_member')), 5,
  'Every paid Team member has their own five-slot allowance');

select tests.authenticate_as_service_role();
select public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('device_team'), 'abcdefghijklmnopqrstuv');
do $$begin
  for ordinal in 1..4 loop
    perform * from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-' || ordinal::text);
  end loop;
end$$;
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-5')), true,
  'Team can connect its fifth device');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-6')), false,
  'Team cannot claim a sixth device');

reset role;
update public.workspace_memberships set deleted_at = now()
where workspace_id = '05000000-0000-4000-8000-000000000001' and user_id = tests.get_supabase_uid('device_member');
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_member')), 3,
  'Removed members no longer receive Team device capacity');
update stripe.subscriptions set status = 'canceled' where customer = (
  select stripe_customer_id from public.workspaces where id = '05000000-0000-4000-8000-000000000001'
);
select is(public.get_sync_device_limit(tests.get_supabase_uid('device_team')), 3,
  'Canceled Team subscriptions no longer grant five slots');
select tests.authenticate_as_service_role();
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-5')), true,
  'A lower allowance does not disconnect existing devices');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('device_team'), 'team-device-6')), false,
  'An account above its new allowance cannot add a device');
select is((select allowed from public.register_e2ee_device_enrollment(tests.get_supabase_uid('device_team'), 'team-device-6', 'Phone', rpad('B', 43, 'B'), 'team-device-1')), false,
  'An over-limit account must free enough slots before replacement');
select is((select count(*) from public.sync_devices where user_id = tests.get_supabase_uid('device_team')), 5::bigint,
  'A rejected replacement leaves existing connections intact');

select * from finish();
rollback;
