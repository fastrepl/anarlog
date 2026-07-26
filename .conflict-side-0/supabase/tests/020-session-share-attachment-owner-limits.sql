begin;
select plan(18);

select tests.create_supabase_user(
  'shared_attachment_active_limit',
  'shared-attachment-active-limit@example.com'
);
select tests.create_supabase_user(
  'shared_attachment_byte_limit',
  'shared-attachment-byte-limit@example.com'
);
select tests.create_supabase_user(
  'shared_attachment_row_limit',
  'shared-attachment-row-limit@example.com'
);

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('shared_attachment_active_limit'),
  tests.get_supabase_uid('shared_attachment_byte_limit'),
  tests.get_supabase_uid('shared_attachment_row_limit')
);

select tests.authenticate_as_service_role();

insert into public.session_shares (
  id,
  workspace_id,
  session_id,
  created_by_user_id
)
values
  (
    '00000000-0000-4000-8000-000000002001'::uuid,
    tests.get_supabase_uid('shared_attachment_active_limit'),
    'active-limit-1',
    tests.get_supabase_uid('shared_attachment_active_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002002'::uuid,
    tests.get_supabase_uid('shared_attachment_active_limit'),
    'active-limit-2',
    tests.get_supabase_uid('shared_attachment_active_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002003'::uuid,
    tests.get_supabase_uid('shared_attachment_active_limit'),
    'active-limit-3',
    tests.get_supabase_uid('shared_attachment_active_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002101'::uuid,
    tests.get_supabase_uid('shared_attachment_byte_limit'),
    'byte-limit-1',
    tests.get_supabase_uid('shared_attachment_byte_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002102'::uuid,
    tests.get_supabase_uid('shared_attachment_byte_limit'),
    'byte-limit-2',
    tests.get_supabase_uid('shared_attachment_byte_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002103'::uuid,
    tests.get_supabase_uid('shared_attachment_byte_limit'),
    'byte-limit-3',
    tests.get_supabase_uid('shared_attachment_byte_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002201'::uuid,
    tests.get_supabase_uid('shared_attachment_row_limit'),
    'row-limit-1',
    tests.get_supabase_uid('shared_attachment_row_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002202'::uuid,
    tests.get_supabase_uid('shared_attachment_row_limit'),
    'row-limit-2',
    tests.get_supabase_uid('shared_attachment_row_limit')
  ),
  (
    '00000000-0000-4000-8000-000000002203'::uuid,
    tests.get_supabase_uid('shared_attachment_row_limit'),
    'row-limit-3',
    tests.get_supabase_uid('shared_attachment_row_limit')
  );

with definition as (
  select lower(pg_get_functiondef(
    'public.reserve_session_share_attachment(uuid,uuid,text,text,text,text,bigint)'::regprocedure
  )) as body
)
select ok(
  body like '%pg_catalog.pg_advisory_xact_lock%'
    and body like '%pg_catalog.hashtextextended(v_owner_user_id::text, 170003)%',
  'Reservations serialize on a dedicated owner-scoped advisory lock'
)
from definition;

with definition as (
  select lower(pg_get_functiondef(
    'public.reserve_session_share_attachment(uuid,uuid,text,text,text,text,bigint)'::regprocedure
  )) as body
)
select ok(
  strpos(body, 'pg_catalog.pg_advisory_xact_lock')
      < strpos(body, 'v_now := clock_timestamp()')
    and strpos(body, 'v_now := clock_timestamp()')
      < strpos(body, 'coalesce(sum(attachment.size_bytes), 0)')
    and strpos(body, 'return query select')
      < strpos(body, 'coalesce(sum(attachment.size_bytes), 0)')
    and strpos(body, 'coalesce(sum(attachment.size_bytes), 0)')
      < strpos(body, 'insert into public.session_share_attachment_objects'),
  'The lock and refreshed clock precede quotas while idempotence precedes quota enforcement'
)
from definition;

select lives_ok(
  $query$
    select count(*)
    from generate_series(1, 5) as input(sequence)
    cross join lateral public.reserve_session_share_attachment(
      case
        when input.sequence <= 3
          then '00000000-0000-4000-8000-000000002001'::uuid
        else '00000000-0000-4000-8000-000000002002'::uuid
      end,
      tests.get_supabase_uid('shared_attachment_active_limit'),
      lpad(input.sequence::text, 43, 'A'),
      lpad(input.sequence::text, 43, 'B'),
      'active-' || input.sequence::text || '.bin',
      'application/octet-stream',
      1
    ) as reservation
  $query$,
  'Five active reservations can be spread across an owner''s shares'
);

select ok(
  (
    select count(*) = 5
      and count(distinct attachment.share_id) = 2
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_active_limit'
    )
      and attachment.state = 'reserved'
      and attachment.cleanup_not_before > clock_timestamp()
  ),
  'The active reservation limit is owner-wide rather than share-local'
);

select results_eq(
  $query$
    select attachment_id, object_key, was_created
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002002'::uuid,
      tests.get_supabase_uid('shared_attachment_active_limit'),
      lpad('5', 43, 'A'),
      lpad('5', 43, 'B'),
      'active-5.bin',
      'application/octet-stream',
      1
    )
  $query$,
  $query$
    select attachment.id, attachment.object_key, false
    from public.session_share_attachment_objects as attachment
    where attachment.share_id = '00000000-0000-4000-8000-000000002002'::uuid
      and attachment.version_ref = lpad('5', 43, 'B')
  $query$,
  'An exact retry remains idempotent after the owner reaches the active limit'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002003'::uuid,
      tests.get_supabase_uid('shared_attachment_active_limit'),
      lpad('6', 43, 'A'),
      lpad('6', 43, 'B'),
      'active-6.bin',
      'application/octet-stream',
      1
    )
  $query$,
  '55000',
  'shared attachment reservation limit exceeded',
  'A sixth active reservation is rejected on a different share'
);

update public.session_share_attachment_objects
set
  state = 'ready',
  sha256 = repeat('a', 64),
  finalized_at = clock_timestamp(),
  updated_at = clock_timestamp()
where share_id = '00000000-0000-4000-8000-000000002001'::uuid
  and version_ref = lpad('1', 43, 'B');

select lives_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002003'::uuid,
      tests.get_supabase_uid('shared_attachment_active_limit'),
      lpad('6', 43, 'A'),
      lpad('6', 43, 'B'),
      'active-6.bin',
      'application/octet-stream',
      1
    )
  $query$,
  'Finalizing one reservation releases one active owner slot'
);

select is(
  (
    select count(*)
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_active_limit'
    )
      and attachment.state = 'reserved'
      and attachment.cleanup_not_before > clock_timestamp()
  ),
  5::bigint,
  'The replacement reservation fills the released owner slot'
);

with seed as materialized (
  select input.sequence, gen_random_uuid() as attachment_id
  from generate_series(1, 9) as input(sequence)
)
insert into public.session_share_attachment_objects (
  id,
  share_id,
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  filename,
  content_type,
  size_bytes,
  sha256,
  state,
  reservation_expires_at,
  cleanup_not_before,
  finalized_at,
  deletion_requested_at,
  created_at,
  updated_at
)
select
  seed.attachment_id,
  '00000000-0000-4000-8000-000000002101'::uuid,
  tests.get_supabase_uid('shared_attachment_byte_limit'),
  lpad(seed.sequence::text, 43, 'C'),
  lpad(seed.sequence::text, 43, 'D'),
  tests.get_supabase_uid('shared_attachment_byte_limit')::text
    || '/00000000-0000-4000-8000-000000002101/'
    || seed.attachment_id::text || '.sna1',
  'byte-' || seed.sequence::text || '.bin',
  'application/octet-stream',
  536870912,
  case when seed.sequence between 4 and 6 then repeat('b', 64) end,
  case
    when seed.sequence <= 3 then 'reserved'
    when seed.sequence <= 6 then 'ready'
    else 'deleting'
  end,
  clock_timestamp() - interval '3 hours',
  clock_timestamp() - interval '2 hours',
  case
    when seed.sequence between 4 and 6
      then clock_timestamp() - interval '1 hour'
  end,
  case
    when seed.sequence >= 7
      then clock_timestamp() - interval '1 hour'
  end,
  clock_timestamp() - interval '4 hours',
  clock_timestamp() - interval '1 hour'
from seed;

select ok(
  (
    select count(*) = 9
      and count(distinct attachment.state) = 3
      and sum(attachment.size_bytes) = 4831838208
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_byte_limit'
    )
  ),
  'Reserved, ready, and deleting rows all contribute to owner bytes'
);

select lives_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002102'::uuid,
      tests.get_supabase_uid('shared_attachment_byte_limit'),
      repeat('I', 43),
      repeat('J', 43),
      'byte-boundary.bin',
      'application/octet-stream',
      536870912
    )
  $query$,
  'An owner can reserve exactly up to the five GiB byte boundary'
);

select is(
  (
    select sum(attachment.size_bytes)
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_byte_limit'
    )
  ),
  5368709120::numeric,
  'Owner byte accounting reaches exactly five GiB across shares'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002103'::uuid,
      tests.get_supabase_uid('shared_attachment_byte_limit'),
      repeat('K', 43),
      repeat('L', 43),
      'byte-overflow.bin',
      'application/octet-stream',
      1
    )
  $query$,
  '54000',
  'shared attachment storage quota exceeded',
  'One more byte is rejected on another share'
);

with seed as materialized (
  select input.sequence, gen_random_uuid() as attachment_id
  from generate_series(1, 9998) as input(sequence)
)
insert into public.session_share_attachment_objects (
  id,
  share_id,
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  filename,
  content_type,
  size_bytes,
  sha256,
  state,
  reservation_expires_at,
  cleanup_not_before,
  finalized_at,
  deletion_requested_at,
  created_at,
  updated_at
)
select
  seed.attachment_id,
  '00000000-0000-4000-8000-000000002201'::uuid,
  tests.get_supabase_uid('shared_attachment_row_limit'),
  lpad(seed.sequence::text, 43, 'M'),
  lpad(seed.sequence::text, 43, 'N'),
  tests.get_supabase_uid('shared_attachment_row_limit')::text
    || '/00000000-0000-4000-8000-000000002201/'
    || seed.attachment_id::text || '.sna1',
  'row-' || seed.sequence::text || '.bin',
  'application/octet-stream',
  1,
  case when seed.sequence % 3 = 1 then repeat('c', 64) end,
  case
    when seed.sequence % 3 = 0 then 'reserved'
    when seed.sequence % 3 = 1 then 'ready'
    else 'deleting'
  end,
  clock_timestamp() - interval '3 hours',
  clock_timestamp() - interval '2 hours',
  case
    when seed.sequence % 3 = 1
      then clock_timestamp() - interval '1 hour'
  end,
  case
    when seed.sequence % 3 = 2
      then clock_timestamp() - interval '1 hour'
  end,
  clock_timestamp() - interval '4 hours',
  clock_timestamp() - interval '1 hour'
from seed;

insert into public.session_share_attachment_objects (
  id,
  share_id,
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  filename,
  content_type,
  size_bytes,
  state,
  reservation_expires_at,
  cleanup_not_before,
  deletion_requested_at,
  gc_lease_id,
  gc_lease_expires_at,
  created_at,
  updated_at
)
values (
  '00000000-0000-4000-8000-000000002299'::uuid,
  '00000000-0000-4000-8000-000000002201'::uuid,
  tests.get_supabase_uid('shared_attachment_row_limit'),
  repeat('O', 43),
  repeat('P', 43),
  tests.get_supabase_uid('shared_attachment_row_limit')::text
    || '/00000000-0000-4000-8000-000000002201/'
    || '00000000-0000-4000-8000-000000002299.sna1',
  'row-gc.bin',
  'application/octet-stream',
  1,
  'deleting',
  clock_timestamp() - interval '3 hours',
  clock_timestamp() - interval '2 hours',
  clock_timestamp() - interval '1 hour',
  '00000000-0000-4000-8000-000000002298'::uuid,
  clock_timestamp() + interval '5 minutes',
  clock_timestamp() - interval '4 hours',
  clock_timestamp() - interval '1 hour'
);

select ok(
  (
    select count(*) = 9999
      and count(distinct attachment.state) = 3
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_row_limit'
    )
  ),
  'All attachment states contribute to the owner row count'
);

select lives_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002202'::uuid,
      tests.get_supabase_uid('shared_attachment_row_limit'),
      repeat('Q', 43),
      repeat('R', 43),
      'row-boundary.bin',
      'application/octet-stream',
      1
    )
  $query$,
  'An owner can reserve the ten-thousandth ledger row on another share'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002203'::uuid,
      tests.get_supabase_uid('shared_attachment_row_limit'),
      repeat('S', 43),
      repeat('T', 43),
      'row-overflow.bin',
      'application/octet-stream',
      1
    )
  $query$,
  '54000',
  'shared attachment object limit exceeded',
  'A deleting row still blocks the ten-thousand-and-first reservation'
);

select is(
  public.finish_session_share_attachment_deletion(
    '00000000-0000-4000-8000-000000002299'::uuid,
    tests.get_supabase_uid('shared_attachment_row_limit')::text
      || '/00000000-0000-4000-8000-000000002201/'
      || '00000000-0000-4000-8000-000000002299.sna1',
    '00000000-0000-4000-8000-000000002298'::uuid
  ),
  true,
  'Physical GC completion removes the deleting ledger row'
);

select lives_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      '00000000-0000-4000-8000-000000002203'::uuid,
      tests.get_supabase_uid('shared_attachment_row_limit'),
      repeat('S', 43),
      repeat('T', 43),
      'row-overflow.bin',
      'application/octet-stream',
      1
    )
  $query$,
  'A new reservation succeeds only after physical GC frees the owner row'
);

select is(
  (
    select count(*)
    from public.session_share_attachment_objects as attachment
    where attachment.owner_user_id = tests.get_supabase_uid(
      'shared_attachment_row_limit'
    )
  ),
  10000::bigint,
  'The owner returns to the exact ten-thousand-row boundary after GC'
);

select * from finish();
rollback;
