begin;
select plan(15);

select tests.create_supabase_user('witness_owner', 'witness-owner@example.com');
select tests.create_supabase_user('witness_other', 'witness-other@example.com');

select ok(
  not has_table_privilege('authenticated', 'public.e2ee_freshness_events', 'SELECT')
    and not has_table_privilege('authenticated', 'public.e2ee_freshness_events', 'INSERT')
    and not has_table_privilege('authenticated', 'public.e2ee_freshness_events', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.e2ee_freshness_events', 'DELETE'),
  'Authenticated clients cannot access the witness table directly'
);

select ok(
  has_table_privilege('service_role', 'public.e2ee_freshness_events', 'SELECT')
    and has_table_privilege('service_role', 'public.e2ee_freshness_events', 'INSERT')
    and not has_table_privilege('service_role', 'public.e2ee_freshness_events', 'UPDATE')
    and not has_table_privilege('service_role', 'public.e2ee_freshness_events', 'DELETE'),
  'Service code can append and read witness events but cannot rewrite history'
);

select ok(
  not has_function_privilege(
    'authenticated',
    'public.publish_e2ee_freshness_events(uuid, uuid, boolean, jsonb)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'public.read_e2ee_freshness_page(uuid, uuid, bigint, bigint, integer)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.publish_e2ee_freshness_events(uuid, uuid, boolean, jsonb)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.read_e2ee_freshness_page(uuid, uuid, bigint, bigint, integer)',
      'EXECUTE'
    ),
  'Only trusted service code can use witness functions'
);

select tests.authenticate_as_service_role();

select is(
  (
    select key_id
    from public.claim_personal_workspace_e2ee_key(
      tests.get_supabase_uid('witness_owner'),
      'abcdefghijklmnopqrstuv'
    )
  ),
  'abcdefghijklmnopqrstuv',
  'Claiming a new recovery-key identity succeeds'
);

select isnt(
  (
    select e2ee_freshness_initialized_at
    from public.workspaces
    where id = tests.get_supabase_uid('witness_owner')
  ),
  null,
  'A newly claimed identity starts with an authoritative empty witness'
);

select results_eq(
  format(
    $$
      select head_sequence, event_sequence
      from public.read_e2ee_freshness_page(%L, %L, 0, null, 64)
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_owner')
  ),
  $$values (0::bigint, null::bigint)$$,
  'An empty initialized witness returns a stable zero head'
);

create temporary table witness_event as
select
  repeat('r', 43)::text as record_id,
  '{"version":1,"ciphertext":"opaque"}'::text as payload;

alter table witness_event add column payload_hash text;
update witness_event
set payload_hash = rtrim(
  translate(encode(extensions.digest(payload, 'sha256'), 'base64'), '+/', '-_'),
  '='
);

select isnt(
  (
    select head_sequence
    from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('witness_owner'),
      tests.get_supabase_uid('witness_owner'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from witness_event
      )
    )
  ),
  0::bigint,
  'Publishing the first opaque ciphertext advances the append-only head'
);

create temporary table witness_head as
select max(sequence)::bigint as sequence
from public.e2ee_freshness_events
where workspace_id = tests.get_supabase_uid('witness_owner');

select is(
  (
    select head_sequence
    from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('witness_owner'),
      tests.get_supabase_uid('witness_owner'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from witness_event
      )
    )
  ),
  (select sequence from witness_head),
  'Publishing the same ciphertext is idempotent'
);

select results_eq(
  format(
    $$
      select head_sequence, through_sequence, event_sequence, record_id, payload_hash, payload
      from public.read_e2ee_freshness_page(%L, %L, 0, null, 64)
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_owner')
  ),
  $$
    select head.sequence, head.sequence, head.sequence, event.record_id, event.payload_hash,
      event.payload
    from witness_event as event
    cross join witness_head as head
  $$,
  'Witness pages return the exact immutable ciphertext event'
);

select throws_ok(
  format(
    $$
      select *
      from public.publish_e2ee_freshness_events(
        %L,
        %L,
        false,
        '[{"record_id":"rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr","payload_hash":"hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh","payload":"tampered"}]'::jsonb
      )
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_owner')
  ),
  '22023',
  'E2EE freshness event is invalid',
  'Payload hashes are verified before an event is appended'
);

select throws_ok(
  format(
    $$
      select *
      from public.publish_e2ee_freshness_events(%L, %L, null, '[]'::jsonb)
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_owner')
  ),
  '22023',
  'E2EE freshness request is invalid',
  'Witness initialization intent must be explicit'
);

update public.workspaces
set e2ee_key_id = 'zyxwvutsrqponmlkjihgfe',
    e2ee_freshness_initialized_at = null
where id = tests.get_supabase_uid('witness_other');

select throws_ok(
  format(
    $$
      select *
      from public.publish_e2ee_freshness_events(%L, %L, false, '[]'::jsonb)
    $$,
    tests.get_supabase_uid('witness_other'),
    tests.get_supabase_uid('witness_other')
  ),
  '55000',
  'E2EE freshness witness is not initialized',
  'Legacy identities cannot bootstrap from untrusted cloud state'
);

select lives_ok(
  format(
    $$
      select *
      from public.publish_e2ee_freshness_events(
        %L,
        %L,
        true,
        (
          select jsonb_build_array(jsonb_build_object(
            'record_id', record_id,
            'payload_hash', payload_hash,
            'payload', payload
          ))
          from witness_event
        )
      )
    $$,
    tests.get_supabase_uid('witness_other'),
    tests.get_supabase_uid('witness_other')
  ),
  'An established client can explicitly initialize a legacy witness'
);

select throws_ok(
  format(
    $$
      select *
      from public.read_e2ee_freshness_page(%L, %L, 0, null, 64)
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_other')
  ),
  '42501',
  'E2EE freshness read is not permitted',
  'A service request cannot cross the actor workspace boundary'
);

select tests.clear_authentication();
select tests.authenticate_as('witness_owner');

select throws_ok(
  format(
    $$
      select *
      from public.read_e2ee_freshness_page(%L, %L, 0, null, 64)
    $$,
    tests.get_supabase_uid('witness_owner'),
    tests.get_supabase_uid('witness_owner')
  ),
  '42501',
  null,
  'Authenticated clients cannot invoke witness reads directly'
);

select * from finish();
rollback;
