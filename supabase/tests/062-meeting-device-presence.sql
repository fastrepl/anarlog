begin;
select plan(9);

select tests.create_supabase_user('meeting_owner', 'meeting-owner@example.com');
select tests.create_supabase_user('meeting_other', 'meeting-other@example.com');

select tests.authenticate_as_service_role();

insert into public.sync_devices (user_id, device_fingerprint, device_name)
values
  (tests.get_supabase_uid('meeting_owner'), 'device-aaaa', 'Work Mac'),
  (tests.get_supabase_uid('meeting_owner'), 'device-bbbb', 'Home Mac');

select is(
  (select count(*)::integer from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-aaaa', 'present'
  )),
  1,
  'A device announcing a meeting sees itself'
);

select results_eq(
  $$select device_fingerprint, device_name, is_primary from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-bbbb', 'present'
  )$$,
  $$values ('device-aaaa'::text, 'Work Mac'::text, false), ('device-bbbb'::text, 'Home Mac'::text, false)$$,
  'Both present devices are listed without a primary'
);

select results_eq(
  $$select device_fingerprint, is_primary from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-bbbb', 'claim'
  )$$,
  $$values ('device-aaaa'::text, false), ('device-bbbb'::text, true)$$,
  'Claiming makes the device primary'
);

select results_eq(
  $$select device_fingerprint, is_primary from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-aaaa', 'present'
  )$$,
  $$values ('device-aaaa'::text, false), ('device-bbbb'::text, true)$$,
  'Renewing presence keeps the existing primary'
);

select results_eq(
  $$select device_fingerprint, is_primary from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-aaaa', 'claim'
  )$$,
  $$values ('device-aaaa'::text, true), ('device-bbbb'::text, false)$$,
  'A later claim moves the primary to the claiming device'
);

select results_eq(
  $$select device_fingerprint from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-bbbb', 'release'
  )$$,
  $$values ('device-aaaa'::text)$$,
  'Releasing removes the device from the meeting'
);

update public.meeting_device_presence
set seen_at = now() - interval '1 minute'
where device_fingerprint = 'device-aaaa';

select results_eq(
  $$select device_fingerprint, is_primary from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-bbbb', 'present'
  )$$,
  $$values ('device-bbbb'::text, false)$$,
  'Devices that stopped renewing are dropped along with their primary claim'
);

select throws_ok(
  $$select * from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_other'), 'meeting-key-0123456789', 'device-aaaa', 'present'
  )$$,
  '42501',
  'sync device is not registered',
  'Another account cannot use a device it has not registered'
);

select tests.authenticate_as('meeting_owner');

select throws_ok(
  $$select * from public.heartbeat_meeting_device(
    tests.get_supabase_uid('meeting_owner'), 'meeting-key-0123456789', 'device-aaaa', 'present'
  )$$,
  '42501',
  null,
  'Signed-in users cannot call the function directly'
);

select * from finish();
rollback;
