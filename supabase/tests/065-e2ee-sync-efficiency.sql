begin;
select plan(25);

select tests.create_supabase_user('eff_owner', 'eff-owner@example.com');
select tests.create_supabase_user('eff_member', 'eff-member@example.com');
select tests.create_supabase_user('eff_other', 'eff-other@example.com');

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('eff_owner'),
  tests.get_supabase_uid('eff_member'),
  tests.get_supabase_uid('eff_other')
);

create temporary table eff_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text
);

grant all on eff_state to authenticated, service_role;

create temporary table eff_events (
  idx integer primary key,
  record_id text not null,
  payload text not null,
  payload_hash text not null
);

grant all on eff_events to authenticated, service_role;

create temporary table eff_seq (
  idx integer primary key,
  seq bigint not null
);

grant all on eff_seq to authenticated, service_role;

insert into eff_events (idx, record_id, payload, payload_hash)
select
  i,
  repeat(substr('abcdef', i, 1), 43),
  payload,
  rtrim(
    translate(encode(extensions.digest(payload, 'sha256'), 'base64'), '+/', '-_'),
    '='
  )
from (
  select
    g.i,
    jsonb_build_object(
      'version', case when g.i = 5 then 2 else 1 end,
      'key_id', 'PPPPPPPPPPPPPPPPPPPPPP',
      'nonce', repeat('A', 32),
      'ciphertext', 'opaque-' || g.i::text
    )::text as payload
  from generate_series(1, 6) as g(i)
) as src;

select has_table(
  'public',
  'e2ee_workspace_snapshots',
  'Encrypted workspace snapshots table exists'
);

select ok(
  exists(
    select 1
    from storage.buckets as bucket
    where bucket.id = 'e2ee-snapshots'
      and bucket."public" = false
      and bucket.file_size_limit = 268435456::bigint
      and bucket.allowed_mime_types = array['application/octet-stream'::text]
  ),
  'Snapshot bucket is private with strict size and MIME limits'
);

select ok(
  not has_table_privilege('authenticated', 'public.e2ee_workspace_snapshots', 'SELECT')
    and not has_table_privilege('authenticated', 'public.e2ee_workspace_snapshots', 'INSERT')
    and not has_table_privilege('authenticated', 'public.e2ee_workspace_snapshots', 'DELETE')
    and not has_table_privilege('authenticated', 'public.e2ee_snapshot_deletions', 'SELECT')
    and not has_table_privilege('authenticated', 'public.e2ee_snapshot_deletions', 'INSERT')
    and not has_table_privilege('authenticated', 'public.e2ee_snapshot_deletions', 'DELETE')
    and not has_function_privilege('authenticated', 'public.read_e2ee_witness_head(uuid,uuid)', 'EXECUTE')
    and not has_function_privilege('authenticated', 'public.read_latest_e2ee_snapshot(uuid,uuid)', 'EXECUTE')
    and not has_function_privilege(
      'authenticated',
      'public.commit_e2ee_snapshot(uuid,uuid,bigint,text,smallint,bigint,text,integer,text)',
      'EXECUTE'
    )
    and not has_function_privilege('authenticated', 'public.finish_e2ee_snapshot_deletions(uuid,text[])', 'EXECUTE')
    and not has_function_privilege('authenticated', 'public.prune_e2ee_compacted_events(uuid,bigint,integer)', 'EXECUTE')
    and not has_function_privilege('authenticated', 'public.prune_e2ee_replica_receipts(interval,integer)', 'EXECUTE'),
  'Authenticated clients have no direct access to snapshot tables or efficiency RPCs'
);

select ok(
  has_function_privilege('service_role', 'public.read_e2ee_witness_head(uuid,uuid)', 'EXECUTE')
    and has_function_privilege('service_role', 'public.read_latest_e2ee_snapshot(uuid,uuid)', 'EXECUTE')
    and has_function_privilege(
      'service_role',
      'public.commit_e2ee_snapshot(uuid,uuid,bigint,text,smallint,bigint,text,integer,text)',
      'EXECUTE'
    )
    and has_function_privilege('service_role', 'public.finish_e2ee_snapshot_deletions(uuid,text[])', 'EXECUTE')
    and has_function_privilege('service_role', 'public.prune_e2ee_compacted_events(uuid,bigint,integer)', 'EXECUTE')
    and has_function_privilege('service_role', 'public.prune_e2ee_replica_receipts(interval,integer)', 'EXECUTE')
    and not has_table_privilege('service_role', 'public.e2ee_freshness_events', 'DELETE'),
  'Service role gets efficiency RPCs while append-only table remains delete-restricted'
);

select ok(
  (
    select count(*) = 4
      and bool_and(p.provolatile = 's')
    from pg_proc as p
    join pg_namespace as n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.proname in (
        'read_e2ee_witness_head',
        'read_e2ee_freshness_page_unchecked',
        'read_e2ee_replica_page',
        'read_latest_e2ee_snapshot'
      )
  ),
  'All E2EE witness, replica, and snapshot read RPCs are STABLE for snapshot consistency'
);

select tests.authenticate_as('eff_owner');

select throws_ok(
  $$
    insert into storage.objects (bucket_id, name, owner_id)
    values (
      'e2ee-snapshots',
      auth.uid()::text || '/1-10000000-0000-4000-8000-000000000001.ans1',
      auth.uid()::text
    )
  $$,
  '42501',
  null,
  'Authenticated clients cannot write directly to e2ee-snapshots'
);

select results_eq(
  $$select count(*) from storage.objects where bucket_id = 'e2ee-snapshots'$$,
  array[0::bigint],
  'Authenticated clients cannot list objects in e2ee-snapshots'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select is(
  array[
    private.e2ee_freshness_payload_key_id('{"version":1,"key_id":"PPPPPPPPPPPPPPPPPPPPPP"}'),
    private.e2ee_freshness_payload_key_id('{"version":2,"key_id":"PPPPPPPPPPPPPPPPPPPPPP"}'),
    private.e2ee_freshness_payload_key_id('{"version":3,"key_id":"PPPPPPPPPPPPPPPPPPPPPP"}')
  ],
  array['PPPPPPPPPPPPPPPPPPPPPP', 'PPPPPPPPPPPPPPPPPPPPPP', null],
  'Payload key extractor accepts both v1 and v2 envelopes and rejects v3'
);

select key_id from public.claim_personal_workspace_e2ee_key(
  tests.get_supabase_uid('eff_owner'),
  'PPPPPPPPPPPPPPPPPPPPPP'
);

select throws_ok(
  $$
    select * from public.read_e2ee_witness_head(
      tests.get_supabase_uid('eff_other'),
      tests.get_supabase_uid('eff_owner')
    )
  $$,
  '42501',
  'E2EE freshness read is not permitted',
  'Head read denies unauthorized users'
);

with accepted as (
  select receipt.receipts
  from public.accept_e2ee_replica_batch(
    tests.get_supabase_uid('eff_owner'),
    tests.get_supabase_uid('eff_owner'),
    '10000000-0000-4000-8000-000000000001',
    0,
    false,
    (
      select jsonb_agg(jsonb_build_object(
        'record_id', record_id,
        'payload_hash', payload_hash,
        'payload', payload
      ) order by idx)
      from eff_events
      where idx <= 5
    )
  ) as receipt
)
insert into eff_seq (idx, seq)
select elem_rows.ord::integer, (elem_rows.elem ->> 'sequence')::bigint
from accepted,
  jsonb_array_elements(accepted.receipts) with ordinality as elem_rows(elem, ord);

select ok(
  (
    select receipt.head_sequence = (select seq from eff_seq where idx = 5)
      and head.head_sequence = (select seq from eff_seq where idx = 5)
      and head.initialized_at is not null
      and jsonb_array_length(receipt.receipts) = 5
    from public.accept_e2ee_replica_batch(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      '10000000-0000-4000-8000-000000000001',
      0,
      false,
      (
        select jsonb_agg(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ) order by idx)
        from eff_events
        where idx <= 5
      )
    ) as receipt,
    lateral public.read_e2ee_witness_head(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner')
    ) as head
  ),
  'Batch accept supports v1+v2 events and read_e2ee_witness_head reports head_sequence'
);

select ok(
  (
    select array_agg(event_sequence order by event_sequence)
      = (select array_agg(seq order by idx) from eff_seq where idx in (1, 2))
    from public.read_e2ee_freshness_page_unchecked(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      0,
      null,
      2,
      50331648
    )
  ) and (
    select array_agg(event_sequence order by event_sequence)
      = array[(select seq from eff_seq where idx = 1)]
    from public.read_e2ee_freshness_page_unchecked(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      0,
      null,
      64,
      1
    )
  ),
  'Single-scan page reader honors p_limit and always includes first row over p_max_bytes'
);

select throws_ok(
  $$
    select * from public.commit_e2ee_snapshot(
      tests.get_supabase_uid('eff_other'),
      tests.get_supabase_uid('eff_owner'),
      (select seq from eff_seq where idx = 1),
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 1)::text || '-10000000-0000-4000-8000-000000000001.ans1',
      1::smallint,
      128::bigint,
      repeat('a', 64),
      1,
      'PPPPPPPPPPPPPPPPPPPPPP'
    )
  $$,
  '42501',
  'E2EE snapshot publication is not permitted',
  'Snapshot commit denies non-members'
);

select throws_ok(
  $$
    select * from public.commit_e2ee_snapshot(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      (select seq + 1 from eff_seq where idx = 5),
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq + 1 from eff_seq where idx = 5)::text || '-10000000-0000-4000-8000-000000000006.ans1',
      1::smallint,
      128::bigint,
      repeat('a', 64),
      5,
      'PPPPPPPPPPPPPPPPPPPPPP'
    )
  $$,
  '22023',
  'E2EE snapshot sequence exceeds witness head',
  'Snapshot commit rejects sequence beyond witness head'
);

select throws_ok(
  $$
    select * from public.commit_e2ee_snapshot(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      (select seq from eff_seq where idx = 1),
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 2)::text || '-10000000-0000-4000-8000-000000000001.ans1',
      1::smallint,
      100::bigint,
      repeat('a', 64),
      1,
      'PPPPPPPPPPPPPPPPPPPPPP'
    )
  $$,
  '22023',
  'Invalid E2EE snapshot commit request',
  'Snapshot commit rejects object_key whose sequence prefix does not match snapshot_sequence'
);

select ok(
  (select committed and cardinality(retired_object_keys) = 0
   from public.commit_e2ee_snapshot(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner'),
     (select seq from eff_seq where idx = 1),
     tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 1)::text || '-10000000-0000-4000-8000-000000000001.ans1',
     1::smallint, 100::bigint, repeat('a', 64), 1, 'PPPPPPPPPPPPPPPPPPPPPP'
   ))
  and (select committed and cardinality(retired_object_keys) = 0
   from public.commit_e2ee_snapshot(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner'),
     (select seq from eff_seq where idx = 2),
     tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 2)::text || '-10000000-0000-4000-8000-000000000002.ans1',
     1::smallint, 110::bigint, repeat('b', 64), 2, 'PPPPPPPPPPPPPPPPPPPPPP'
   ))
  and (select committed
     and retired_object_keys = array[tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 1)::text || '-10000000-0000-4000-8000-000000000001.ans1']
   from public.commit_e2ee_snapshot(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner'),
     (select seq from eff_seq where idx = 3),
     tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 3)::text || '-10000000-0000-4000-8000-000000000003.ans1',
     1::smallint, 120::bigint, repeat('c', 64), 3, 'PPPPPPPPPPPPPPPPPPPPPP'
   )),
  'Snapshot commit keeps the newest two snapshots and returns retired object keys'
);

select results_eq(
  $$
    select committed, retired_object_keys
    from public.commit_e2ee_snapshot(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      (select seq from eff_seq where idx = 2),
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 2)::text || '-20000000-0000-4000-8000-000000000099.ans1',
      1::smallint, 110::bigint, repeat('d', 64), 2, 'PPPPPPPPPPPPPPPPPPPPPP'
    )
  $$,
  $$
    values (
      false,
      array[
        tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 1)::text || '-10000000-0000-4000-8000-000000000001.ans1',
        tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 2)::text || '-20000000-0000-4000-8000-000000000099.ans1'
      ]
    )
  $$,
  'Older snapshot sequence commit is a monotonic no-op and keeps unacknowledged retired deletions queued'
);

select is(
  public.finish_e2ee_snapshot_deletions(
    tests.get_supabase_uid('eff_owner'),
    array[
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 1)::text || '-10000000-0000-4000-8000-000000000001.ans1',
      tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 2)::text || '-20000000-0000-4000-8000-000000000099.ans1'
    ]::text[]
  ),
  2,
  'finish_e2ee_snapshot_deletions clears acknowledged retired and orphan object keys'
);

select results_eq(
  $$
    select snapshot_sequence, head_sequence, cloud_authority_after
    from public.read_latest_e2ee_snapshot(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner')
    )
  $$,
  $$values ((select seq from eff_seq where idx = 3), (select seq from eff_seq where idx = 5), 0::bigint)$$,
  'read_latest_e2ee_snapshot returns the latest snapshot and current head'
);

select results_eq(
  $$
    select compacted_through, deleted_rows
    from public.prune_e2ee_compacted_events(tests.get_supabase_uid('eff_owner'), 2, 5000)
  $$,
  $$values ((select seq from eff_seq where idx = 3), 3)$$,
  'prune_e2ee_compacted_events deletes only events through the compacted watermark'
);

select throws_ok(
  $$
    select * from public.read_e2ee_replica_page(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      (select seq from eff_seq where idx = 2),
      null,
      64,
      50331648
    )
  $$,
  'PGRST',
  '{"code":"A0003","message":"E2EE witness history compacted","details":null,"hint":null}',
  'Reading before the compacted watermark raises PGRST A0003 (410)'
);

select results_eq(
  $$
    select array_agg(event_sequence order by event_sequence)
    from public.read_e2ee_replica_page(
      tests.get_supabase_uid('eff_owner'),
      tests.get_supabase_uid('eff_owner'),
      (select seq from eff_seq where idx = 3),
      null,
      64,
      50331648
    )
  $$,
  $$select array_agg(seq order by idx) from eff_seq where idx in (4, 5)$$,
  'Reading at the compacted watermark streams only retained tail events'
);

select ok(
  (select committed
   from public.commit_e2ee_snapshot(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner'),
     (select seq from eff_seq where idx = 5),
     tests.get_supabase_uid('eff_owner')::text || '/' || (select seq from eff_seq where idx = 5)::text || '-10000000-0000-4000-8000-000000000005.ans1',
     1::smallint, 150::bigint, repeat('e', 64), 5, 'PPPPPPPPPPPPPPPPPPPPPP'
   ))
  and (select compacted_through = (select seq from eff_seq where idx = 5) and deleted_rows = 2
   from public.prune_e2ee_compacted_events(tests.get_supabase_uid('eff_owner'), 0, 5000))
  and (select head_sequence = (select seq from eff_seq where idx = 5)
   from public.read_e2ee_witness_head(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner')
   ))
  and (select receipt.head_sequence > (select seq from eff_seq where idx = 5)
   from public.accept_e2ee_replica_batch(
     tests.get_supabase_uid('eff_owner'),
     tests.get_supabase_uid('eff_owner'),
     '10000000-0000-4000-8000-000000000006',
     (select seq from eff_seq where idx = 5),
     false,
     (select jsonb_agg(jsonb_build_object(
       'record_id', record_id,
       'payload_hash', payload_hash,
       'payload', payload
     )) from eff_events where idx = 6)
   ) as receipt),
  'Compacted watermark keeps head_sequence intact even when zero tail events remain'
);

update public.e2ee_replica_receipts
set created_at = now() - interval '1 hour'
where workspace_id = tests.get_supabase_uid('eff_owner')
  and mutation_id = '10000000-0000-4000-8000-000000000001';

select is(
  public.prune_e2ee_replica_receipts(interval '1 minute', 5000),
  1,
  'prune_e2ee_replica_receipts removes aged receipts while keeping fresh ones'
);

-- Shared workspace key check
select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('eff_owner');
select public.publish_e2ee_member_identity(rpad('effo', 43, 'A'));
insert into eff_state (name, workspace_id)
select 'shared', workspace_id from public.create_workspace('Eff Shared');
select tests.enable_workspace_plan((select workspace_id from eff_state where name = 'shared'));
insert into eff_state (name, invitation_id, invite_token)
select 'invite', invitation_id, invite_token
from public.create_workspace_invitation(
  (select workspace_id from eff_state where name = 'shared'),
  'eff-member@example.com'
);

select tests.clear_authentication();
select tests.authenticate_as('eff_member');
select * from public.accept_workspace_invitation(
  (select invitation_id from eff_state where name = 'invite'),
  (select invite_token from eff_state where name = 'invite')
);
select public.publish_e2ee_member_identity(rpad('effm', 43, 'A'));

select tests.clear_authentication();
select tests.authenticate_as('eff_owner');
select * from public.set_workspace_e2ee_key(
  (select workspace_id from eff_state where name = 'shared'),
  'AAAAAAAAAAAAAAAAAAAAAA',
  jsonb_build_array(
    jsonb_build_object('userId', tests.get_supabase_uid('eff_owner'), 'ephemeralPublicKey', rpad('eo', 43, 'A'), 'nonce', rpad('no', 32, 'B'), 'ciphertext', rpad('co', 64, 'C')),
    jsonb_build_object('userId', tests.get_supabase_uid('eff_member'), 'ephemeralPublicKey', rpad('em', 43, 'A'), 'nonce', rpad('nm', 32, 'B'), 'ciphertext', rpad('cm', 64, 'C'))
  )
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

with shared_event as (
  select
    repeat('s', 43) as record_id,
    payload,
    rtrim(translate(encode(extensions.digest(payload, 'sha256'), 'base64'), '+/', '-_'), '=') as payload_hash
  from (select '{"version":2,"key_id":"AAAAAAAAAAAAAAAAAAAAAA","nonce":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ciphertext":"shared-1"}'::text as payload) p
)
insert into eff_seq (idx, seq)
select 7, receipt.head_sequence
from public.accept_e2ee_replica_batch(
  tests.get_supabase_uid('eff_owner'),
  (select workspace_id from eff_state where name = 'shared'),
  '20000000-0000-4000-8000-000000000001',
  0,
  false,
  (select jsonb_agg(jsonb_build_object('record_id', record_id, 'payload_hash', payload_hash, 'payload', payload)) from shared_event)
) as receipt;

select throws_ok(
  $$
    select * from public.commit_e2ee_snapshot(
      tests.get_supabase_uid('eff_owner'),
      (select workspace_id from eff_state where name = 'shared'),
      (select seq from eff_seq where idx = 7),
      (select workspace_id::text from eff_state where name = 'shared') || '/' || (select seq from eff_seq where idx = 7)::text || '-10000000-0000-4000-8000-000000000001.ans1',
      1::smallint, 100::bigint, repeat('f', 64), 1, 'BBBBBBBBBBBBBBBBBBBBBB'
    )
  $$,
  '22023',
  'Invalid E2EE snapshot key identity',
  'Shared workspace snapshot commit rejects stale or foreign key_id'
);

-- Split commit and read across separate statements because STABLE read RPCs
-- intentionally read a single statement snapshot.
insert into eff_state (name, invite_token)
select
  'shared_commit',
  case when committed then 'committed' else 'not-committed' end
from public.commit_e2ee_snapshot(
  tests.get_supabase_uid('eff_member'),
  (select workspace_id from eff_state where name = 'shared'),
  (select seq from eff_seq where idx = 7),
  (select workspace_id::text from eff_state where name = 'shared') || '/' || (select seq from eff_seq where idx = 7)::text || '-10000000-0000-4000-8000-000000000001.ans1',
  1::smallint, 100::bigint, repeat('f', 64), 1, 'AAAAAAAAAAAAAAAAAAAAAA'
);

select results_eq(
  $$
    select
      (select invite_token = 'committed' from eff_state where name = 'shared_commit'),
      snap.snapshot_sequence,
      head.head_sequence
    from public.read_latest_e2ee_snapshot(
      tests.get_supabase_uid('eff_member'),
      (select workspace_id from eff_state where name = 'shared')
    ) as snap,
    lateral public.read_e2ee_witness_head(
      tests.get_supabase_uid('eff_member'),
      (select workspace_id from eff_state where name = 'shared')
    ) as head
  $$,
  $$values (true, (select seq from eff_seq where idx = 7), (select seq from eff_seq where idx = 7))$$,
  'Shared workspace member can commit, read snapshot descriptor, and read witness head'
);

select * from finish();
rollback;
