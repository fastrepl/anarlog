begin;
select plan(27);

select tests.create_supabase_user('enrollment_owner', 'enrollment-owner@example.com');
select tests.create_supabase_user('enrollment_other', 'enrollment-other@example.com');

select ok(
  not has_table_privilege(
    'authenticated',
    'public.e2ee_device_enrollment_requests',
    'SELECT'
  )
    and has_table_privilege(
      'service_role',
      'public.e2ee_device_enrollment_requests',
      'SELECT'
    )
    and not has_function_privilege(
      'authenticated',
      'public.register_e2ee_device_enrollment(uuid, text, text, text, text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.register_e2ee_device_enrollment(uuid, text, text, text, text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.seal_e2ee_device_enrollment(uuid, uuid, text, text, text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.consume_e2ee_device_enrollment(uuid, uuid, text, text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.rename_sync_device(uuid, text, text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.rename_sync_device(uuid, text, text)',
      'EXECUTE'
    ),
  'Enrollment rows and RPCs are restricted to trusted service code'
);

select tests.authenticate_as_service_role();

select is(
  (
    select requires_existing_key
    from public.register_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      'device-0001',
      'First Mac',
      rpad('A', 43, 'A'),
      null
    )
  ),
  true,
  'A first device must establish the account recovery identity'
);

select is(
  (
    select key_id
    from public.claim_personal_workspace_e2ee_key(
      tests.get_supabase_uid('enrollment_owner'),
      'abcdefghijklmnopqrstuv'
    )
  ),
  'abcdefghijklmnopqrstuv',
  'The first device establishes the recovery identity'
);

create temporary table enrollment_test_state as
select request_id
from public.register_e2ee_device_enrollment(
  tests.get_supabase_uid('enrollment_owner'),
  'device-0001',
  'First Mac',
  rpad('A', 43, 'A'),
  null
);

select results_eq(
  format(
    $$
      select allowed, requires_existing_key, enrollment_status, device_count
      from public.register_e2ee_device_enrollment(%L, 'device-0001', 'Renamed Mac', %L, null)
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('A', 43, 'A')
  ),
  $$values (true, false, 'pending'::text, 1::bigint)$$,
  'Registration is idempotent and occupies one device slot'
);

select is(
  (
    select request_id
    from public.register_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      'device-0001',
      'Renamed Mac',
      rpad('A', 43, 'A'),
      null
    )
  ),
  (select request_id from enrollment_test_state),
  'Idempotent registration preserves the request identifier'
);

select is(
  (
    select result
    from public.seal_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_other'),
      (select request_id from enrollment_test_state),
      rpad('E', 43, 'E'),
      rpad('N', 32, 'N'),
      rpad('C', 100, 'C')
    )
  ),
  'unavailable',
  'Another account cannot seal an enrollment request'
);

select is(
  (
    select result
    from public.seal_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      (select request_id from enrollment_test_state),
      rpad('E', 43, 'E'),
      rpad('N', 32, 'N'),
      rpad('C', 100, 'C')
    )
  ),
  'sealed',
  'An approved device can publish an opaque enrollment package'
);

select is(
  (
    select result
    from public.seal_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      (select request_id from enrollment_test_state),
      rpad('F', 43, 'F'),
      rpad('O', 32, 'O'),
      rpad('D', 100, 'D')
    )
  ),
  'conflict',
  'A sealed package cannot be replaced'
);

select results_eq(
  format(
    $$
      select enrollment_status, ephemeral_public_key, nonce, ciphertext
      from public.register_e2ee_device_enrollment(%L, 'device-0001', 'Renamed Mac', %L, null)
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('A', 43, 'A')
  ),
  format(
    $$values ('sealed'::text, %L::text, %L::text, %L::text)$$,
    rpad('E', 43, 'E'),
    rpad('N', 32, 'N'),
    rpad('C', 100, 'C')
  ),
  'The requesting device can poll the same sealed package'
);

select is(
  (
    select consumed
    from public.consume_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      (select request_id from enrollment_test_state),
      'wrong-device',
      rpad('A', 43, 'A')
    )
  ),
  false,
  'A different device cannot acknowledge the package'
);

select is(
  (
    select consumed
    from public.consume_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      (select request_id from enrollment_test_state),
      'device-0001',
      rpad('A', 43, 'A')
    )
  ),
  true,
  'The requesting device can acknowledge an imported package'
);

select ok(
  (
    select consumed_at is not null
      and ephemeral_public_key is null
      and nonce is null
      and ciphertext is null
    from public.e2ee_device_enrollment_requests
    where id = (select request_id from enrollment_test_state)
  ),
  'Acknowledgement clears the relayed ciphertext'
);

select is(
  (
    select consumed
    from public.consume_e2ee_device_enrollment(
      tests.get_supabase_uid('enrollment_owner'),
      (select request_id from enrollment_test_state),
      'device-0001',
      rpad('A', 43, 'A')
    )
  ),
  true,
  'Acknowledgement is idempotent'
);

select results_eq(
  format(
    $$
      select
        request_id <> %L::uuid,
        enrollment_status,
        device_count
      from public.register_e2ee_device_enrollment(
        %L,
        'device-0001',
        'Renamed Mac',
        %L,
        null
      )
    $$,
    (select request_id from enrollment_test_state),
    tests.get_supabase_uid('enrollment_owner'),
    rpad('A', 43, 'A')
  ),
  $$values (true, 'pending'::text, 1::bigint)$$,
  'Registration replaces an acknowledged package with a fresh request'
);

select results_eq(
  format(
    $$
      select allowed, device_count
      from public.claim_sync_device(%L, 'device-0001', 'Renamed Mac')
    $$,
    tests.get_supabase_uid('enrollment_owner')
  ),
  $$values (true, 1::bigint)$$,
  'Credential exchange converts the reserved slot into an active device'
);

select ok(
  exists (
    select 1
    from public.sync_devices
    where user_id = tests.get_supabase_uid('enrollment_owner')
      and device_fingerprint = 'device-0001'
  )
    and not exists (
      select 1
      from public.e2ee_device_enrollment_requests
      where id = (select request_id from enrollment_test_state)
    ),
  'Conversion removes the enrollment mailbox row'
);

select is(
  public.rename_sync_device(
    tests.get_supabase_uid('enrollment_owner'),
    'device-0001',
    'Desk Mac'
  ),
  true,
  'A device can be renamed through the trusted account-scoped RPC'
);

select is(
  (
    select device_name
    from public.sync_devices
    where user_id = tests.get_supabase_uid('enrollment_owner')
      and device_fingerprint = 'device-0001'
  ),
  'Desk Mac',
  'The renamed device name is stored with the account'
);

select is(
  (
    with claimed as materialized (
      select *
      from public.claim_sync_device(
        tests.get_supabase_uid('enrollment_owner'),
        'device-0001',
        'Automatic Hostname'
      )
    )
    select device.device_name
    from public.sync_devices as device
    cross join claimed
    where device.user_id = tests.get_supabase_uid('enrollment_owner')
      and device.device_fingerprint = 'device-0001'
  ),
  'Desk Mac',
  'Credential refreshes preserve a user-assigned device name'
);

do $$
declare
  ordinal integer;
begin
  for ordinal in 2..5 loop
    perform *
    from public.claim_sync_device(
      tests.get_supabase_uid('enrollment_owner'),
      'device-000' || ordinal::text,
      'Device ' || ordinal::text
    );
  end loop;
end;
$$;

select results_eq(
  format(
    $$
      select allowed, enrollment_status, device_count
      from public.register_e2ee_device_enrollment(
        %L,
        'device-0001',
        'Reinstalled Mac',
        %L,
        'device-0002'
      )
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('R', 43, 'R')
  ),
  $$values (true, 'pending'::text, 5::bigint)$$,
  'An existing device can re-enroll without consuming another slot'
);

select ok(
  exists (
    select 1
    from public.sync_devices
    where user_id = tests.get_supabase_uid('enrollment_owner')
      and device_fingerprint = 'device-0002'
  ),
  'Re-enrolling an existing device does not replace another device'
);

select results_eq(
  format(
    $$
      select allowed, device_count
      from public.register_e2ee_device_enrollment(%L, 'device-0006', 'Sixth Mac', %L, null)
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('B', 43, 'B')
  ),
  $$values (false, 5::bigint)$$,
  'Approved and pending devices share the five-device cap'
);

select results_eq(
  format(
    $$
      select allowed, enrollment_status, device_count
      from public.register_e2ee_device_enrollment(
        %L,
        'device-0006',
        'Sixth Mac',
        %L,
        'device-0002'
      )
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('B', 43, 'B')
  ),
  $$values (true, 'pending'::text, 5::bigint)$$,
  'A capped account can atomically replace a selected device'
);

select ok(
  not exists (
    select 1
    from public.sync_devices
    where user_id = tests.get_supabase_uid('enrollment_owner')
      and device_fingerprint = 'device-0002'
  )
    and exists (
      select 1
      from public.e2ee_device_enrollment_requests
      where user_id = tests.get_supabase_uid('enrollment_owner')
        and device_fingerprint = 'device-0006'
    ),
  'Replacement removes the old device and reserves the new slot'
);

update public.e2ee_device_enrollment_requests
set created_at = now() - interval '2 days',
    expires_at = now() - interval '1 day'
where user_id = tests.get_supabase_uid('enrollment_owner')
  and device_fingerprint = 'device-0006';

select results_eq(
  format(
    $$
      select allowed, device_count
      from public.register_e2ee_device_enrollment(%L, 'device-0007', 'Seventh Mac', %L, null)
    $$,
    tests.get_supabase_uid('enrollment_owner'),
    rpad('G', 43, 'G')
  ),
  $$values (true, 5::bigint)$$,
  'Expired requests free their device slot'
);

select lives_ok(
  format(
    $$
      select public.remove_sync_device(%L, 'device-0007')
    $$,
    tests.get_supabase_uid('enrollment_owner')
  ),
  'Removing a device also removes its pending enrollment'
);

select is(
  (
    select count(*)
    from (
      select device_fingerprint
      from public.sync_devices
      where user_id = tests.get_supabase_uid('enrollment_owner')
      union
      select device_fingerprint
      from public.e2ee_device_enrollment_requests
      where user_id = tests.get_supabase_uid('enrollment_owner')
        and expires_at > now()
    ) as occupied_slots
  ),
  4::bigint,
  'Removing a pending device immediately frees the slot'
);

select * from finish();
rollback;
