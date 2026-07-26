begin;
select plan(55);

select tests.create_supabase_user('deletion_owner', 'deletion-owner@example.com');
select tests.create_supabase_user('deletion_reclaim', 'deletion-reclaim@example.com');

create temporary table account_deletion_test_state (
  name text primary key,
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  was_created boolean,
  lease_id uuid,
  lease_expires_at timestamptz
);

grant all on account_deletion_test_state to anon, authenticated, service_role;

select has_table(
  'private',
  'account_deletion_jobs',
  'Account deletion has a durable private queue'
);

select ok(
  (
    select class.relrowsecurity
    from pg_class as class
    join pg_namespace as namespace on namespace.oid = class.relnamespace
    where namespace.nspname = 'private'
      and class.relname = 'account_deletion_jobs'
  ),
  'Account deletion jobs have RLS enabled'
);

select ok(
  not exists (
    select 1
    from pg_constraint as relation_constraint
    where relation_constraint.conrelid = 'private.account_deletion_jobs'::regclass
      and relation_constraint.contype = 'f'
      and relation_constraint.confrelid = 'auth.users'::regclass
  ),
  'Account deletion jobs deliberately survive Auth user deletion'
);

select ok(
  not has_table_privilege('anon', 'private.account_deletion_jobs', 'SELECT')
    and not has_table_privilege('authenticated', 'private.account_deletion_jobs', 'SELECT')
    and not has_table_privilege('authenticated', 'private.account_deletion_jobs', 'INSERT')
    and not has_table_privilege('authenticated', 'private.account_deletion_jobs', 'UPDATE')
    and not has_table_privilege('authenticated', 'private.account_deletion_jobs', 'DELETE')
    and has_table_privilege('service_role', 'private.account_deletion_jobs', 'SELECT')
    and has_table_privilege('service_role', 'private.account_deletion_jobs', 'INSERT')
    and has_table_privilege('service_role', 'private.account_deletion_jobs', 'UPDATE')
    and has_table_privilege('service_role', 'private.account_deletion_jobs', 'DELETE'),
  'Only the service role can access account deletion jobs'
);

select ok(
  not has_function_privilege(
      'authenticated',
      'public.begin_account_deletion(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.claim_account_deletion_leases(uuid,integer,integer)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.claim_account_deletion_leases_v2(uuid,integer,integer)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_account_deletion_stripe_deleted(uuid,uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_account_deletion_prefix_swept(uuid,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.finish_account_deletion(uuid,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.begin_account_deletion(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.claim_account_deletion_leases(uuid,integer,integer)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.claim_account_deletion_leases_v2(uuid,integer,integer)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.mark_account_deletion_stripe_deleted(uuid,uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.mark_account_deletion_prefix_swept(uuid,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.finish_account_deletion(uuid,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.account_deletion_auth_user_exists(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'private.account_deletion_auth_user_exists(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.prepare_account_deletion_extension(uuid,timestamp with time zone)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'private.prepare_account_deletion_extension(uuid,timestamp with time zone)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.account_deletion_extension_ledgers_empty(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'private.account_deletion_extension_ledgers_empty(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.account_deletion_extension_prefix_empty(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'private.account_deletion_extension_prefix_empty(uuid)',
      'EXECUTE'
    ),
  'Account deletion RPCs are service-role-only'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'begin_account_deletion',
        'claim_account_deletion_leases',
        'mark_account_deletion_stripe_deleted',
        'mark_account_deletion_prefix_swept',
        'finish_account_deletion'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  )
    and exists (
      select 1
      from pg_proc as proc
      join pg_namespace as namespace on namespace.oid = proc.pronamespace
      where namespace.nspname = 'private'
        and proc.proname = 'account_deletion_auth_user_exists'
        and proc.prosecdef
        and 'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
  ),
  'Account deletion RPCs are invokers and the Auth lookup is a hardened definer'
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
values (
  '00000000-0000-4000-8000-000000000350'::uuid,
  tests.get_supabase_uid('deletion_reclaim'),
  'shared',
  'Deletion test workspace'
);

insert into public.workspace_memberships (id, workspace_id, user_id, role)
values (
  '00000000-0000-4000-8000-000000000351'::uuid,
  '00000000-0000-4000-8000-000000000350'::uuid,
  tests.get_supabase_uid('deletion_reclaim'),
  'owner'
), (
  '00000000-0000-4000-8000-000000000352'::uuid,
  '00000000-0000-4000-8000-000000000350'::uuid,
  tests.get_supabase_uid('deletion_owner'),
  'member'
);

select throws_ok(
  $$
    select *
    from public.begin_account_deletion(
      '00000000-0000-4000-8000-000000000390'::uuid
    )
  $$,
  '55000',
  'account deletion owner is unavailable',
  'A missing personal workspace cannot create an account deletion job'
);

select results_eq(
  $$
    select count(*)
    from private.account_deletion_jobs
    where owner_user_id = '00000000-0000-4000-8000-000000000390'::uuid
  $$,
  array[0::bigint],
  'A rejected deletion request leaves no orphan cleanup job'
);

update public.workspaces
set e2ee_key_id = repeat('K', 22)
where id = tests.get_supabase_uid('deletion_owner');

select tests.clear_authentication();
select tests.authenticate_as('deletion_owner');

select lives_ok(
  $$
    insert into storage.objects (bucket_id, name, owner)
    values ('audio-files', auth.uid()::text || '/before.wav', auth.uid())
  $$,
  'An active account can upload audio before deletion'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

insert into public.attachment_backup_objects (
  id,
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  ciphertext_size_bytes,
  ciphertext_sha256,
  state,
  reservation_expires_at,
  cleanup_not_before,
  finalized_at,
  gc_lease_id,
  gc_lease_expires_at,
  created_at,
  updated_at
)
values (
  '00000000-0000-4000-8000-000000000301'::uuid,
  tests.get_supabase_uid('deletion_owner'),
  repeat('A', 43),
  repeat('B', 43),
  tests.get_supabase_uid('deletion_owner')::text
    || '/00000000-0000-4000-8000-000000000301.anb1',
  1024,
  null,
  'reserved',
  now() + interval '30 minutes',
  now() + interval '1 hour',
  null,
  '00000000-0000-4000-8000-000000000311'::uuid,
  now() + interval '5 minutes',
  now(),
  now()
), (
  '00000000-0000-4000-8000-000000000302'::uuid,
  tests.get_supabase_uid('deletion_owner'),
  repeat('C', 43),
  repeat('D', 43),
  tests.get_supabase_uid('deletion_owner')::text
    || '/00000000-0000-4000-8000-000000000302.anb1',
  2048,
  repeat('a', 64),
  'current',
  now() - interval '2 hours',
  now() + interval '2 hours',
  now() - interval '1 hour',
  null,
  null,
  now() - interval '3 hours',
  now()
);

select lives_ok(
  $$
    insert into account_deletion_test_state (
      name,
      owner_user_id,
      final_sweep_not_before,
      was_created
    )
    select
      'owner_begin',
      owner_user_id,
      final_sweep_not_before,
      was_created
    from public.begin_account_deletion(
      tests.get_supabase_uid('deletion_owner')
    )
  $$,
  'Account deletion is durably requested'
);

select ok(
  (
    select
      personal.deleted_at is not null
      and count(*) = 2
      and bool_and(membership.deleted_at is not null)
      and (
        select other_member.deleted_at is null
        from public.workspace_memberships as other_member
        where other_member.workspace_id = '00000000-0000-4000-8000-000000000350'::uuid
          and other_member.user_id = tests.get_supabase_uid('deletion_reclaim')
      )
    from public.workspaces as personal
    join public.workspace_memberships as membership
      on membership.user_id = tests.get_supabase_uid('deletion_owner')
    where personal.id = tests.get_supabase_uid('deletion_owner')
    group by personal.deleted_at
  ),
  'Deletion immediately revokes every membership without changing other members'
);

select ok(
  (
    select count(*) = 2
      and bool_and(backup.state = 'deleting')
      and bool_and(backup.deletion_requested_at is not null)
    from public.attachment_backup_objects as backup
    where backup.owner_user_id = tests.get_supabase_uid('deletion_owner')
  ),
  'Every backup state is marked for physical deletion'
);

select ok(
  (
    select backup.cleanup_not_before > now()
      and backup.gc_lease_id = '00000000-0000-4000-8000-000000000311'::uuid
      and backup.gc_lease_expires_at > now()
    from public.attachment_backup_objects as backup
    where backup.id = '00000000-0000-4000-8000-000000000301'::uuid
  ),
  'Deletion preserves cleanup horizons and active GC leases'
);

select ok(
  (
    select deletion.final_sweep_not_before
        >= deletion.requested_at + interval '24 hours 5 minutes'
      and deletion.final_sweep_not_before >= (
        select max(backup.cleanup_not_before)
        from public.attachment_backup_objects as backup
        where backup.owner_user_id = tests.get_supabase_uid('deletion_owner')
      )
    from private.account_deletion_jobs as deletion
    where deletion.owner_user_id = tests.get_supabase_uid('deletion_owner')
  ),
  'The final sweep waits for resumable audio and backup capabilities'
);

select is(
  (
    select was_created
    from public.begin_account_deletion(
      tests.get_supabase_uid('deletion_owner')
    )
  ),
  false,
  'Repeated deletion requests are idempotent'
);

select is(
  (
    select final_sweep_not_before
    from public.begin_account_deletion(
      tests.get_supabase_uid('deletion_owner')
    )
  ),
  (
    select final_sweep_not_before
    from account_deletion_test_state
    where name = 'owner_begin'
  ),
  'Repeated deletion requests never shorten the final sweep horizon'
);

select tests.clear_authentication();
select tests.authenticate_as('deletion_owner');

select results_eq(
  $$select count(*) from public.workspaces$$,
  array[0::bigint],
  'A stale token immediately loses shared-workspace visibility'
);

select throws_ok(
  $$
    select *
    from public.resolve_my_session_access(
      '00000000-0000-4000-8000-000000000350'::uuid
    )
  $$,
  '42501',
  'session access operation not permitted',
  'A stale token immediately loses individual shared-note access'
);

select throws_ok(
  $$
    insert into storage.objects (bucket_id, name, owner)
    values ('audio-files', auth.uid()::text || '/after.wav', auth.uid())
  $$,
  '42501',
  null,
  'A stale user token cannot upload audio after deletion begins'
);

select results_eq(
  $$select count(*) from storage.objects where bucket_id = 'audio-files'$$,
  array[0::bigint],
  'A tombstoned account can no longer read existing audio objects'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('deletion_owner'),
      repeat('E', 43),
      repeat('F', 43),
      4096::bigint,
      1::smallint
    )
  $$,
  '42501',
  'active personal E2EE workspace required',
  'New backup capabilities are denied after deletion begins'
);

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000321'::uuid,
      1,
      300
    )
  ),
  0::bigint,
  'Account deletion cannot be claimed before the capability horizon'
);

update private.account_deletion_jobs
set
  requested_at = now() - interval '3 hours',
  final_sweep_not_before = now() - interval '1 hour',
  updated_at = now()
where owner_user_id = tests.get_supabase_uid('deletion_owner');

select ok(
  (
    select not lease.stripe_deleted
      and not lease.cleanup_ready
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-4000-8000-000000000320'::uuid,
      1,
      300
    ) as lease
  ),
  'A due Stripe stage remains claimable while attachment ledgers block destructive cleanup'
);

select ok(
  public.mark_account_deletion_stripe_deleted(
    tests.get_supabase_uid('deletion_owner'),
    '00000000-0000-4000-8000-000000000320'::uuid,
    null
  ),
  'The Stripe stage checkpoints without touching retained attachment ledgers'
);

update private.account_deletion_jobs
set
  lease_expires_at = clock_timestamp() - interval '1 minute',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_owner');

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000321'::uuid,
      1,
      300
    )
  ),
  0::bigint,
  'Account deletion cannot be claimed while backup ledger rows remain'
);

delete from public.attachment_backup_objects
where owner_user_id = tests.get_supabase_uid('deletion_owner');

select lives_ok(
  $$
    insert into account_deletion_test_state (
      name,
      owner_user_id,
      final_sweep_not_before,
      lease_id,
      lease_expires_at
    )
    select
      'owner_claim',
      owner_user_id,
      final_sweep_not_before,
      lease_id,
      lease_expires_at
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000321'::uuid,
      1,
      300
    )
  $$,
  'A due account with an empty backup ledger can be leased'
);

select ok(
  (
    select owner_user_id = tests.get_supabase_uid('deletion_owner')
      and lease_id = '00000000-0000-4000-8000-000000000321'::uuid
      and lease_expires_at > now()
    from account_deletion_test_state
    where name = 'owner_claim'
  ),
  'The deletion lease is fenced to the claiming worker'
);

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000322'::uuid,
      1,
      300
    )
  ),
  0::bigint,
  'An active deletion lease prevents duplicate work'
);

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000399'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'A stale worker cannot confirm the Storage sweep'
);

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'The Storage sweep cannot be checkpointed while owned audio remains'
);

update storage.objects
set
  owner = null,
  owner_id = null
where bucket_id = 'audio-files'
  and name = tests.get_supabase_uid('deletion_owner')::text || '/before.wav';

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'The Storage sweep detects audio independently of ownership metadata'
);

insert into storage.objects (bucket_id, name, owner, owner_id)
values (
  'attachment-backups',
  tests.get_supabase_uid('deletion_owner')::text
    || '/00000000-0000-4000-8000-000000000398.anb1',
  null,
  null
);

update storage.objects
set name = 'removed-audio/' || name
where bucket_id = 'audio-files'
  and name = tests.get_supabase_uid('deletion_owner')::text || '/before.wav';

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'The Storage sweep detects orphaned attachment backups'
);

update storage.objects
set name = 'removed-backup/' || name
where bucket_id = 'attachment-backups'
  and name = tests.get_supabase_uid('deletion_owner')::text
    || '/00000000-0000-4000-8000-000000000398.anb1';

select ok(
  public.mark_account_deletion_prefix_swept(
    tests.get_supabase_uid('deletion_owner'),
    '00000000-0000-4000-8000-000000000321'::uuid
  ),
  'The current worker can confirm an empty Storage prefix'
);

select ok(
  (
    select prefix_swept_at is not null
    from private.account_deletion_jobs
    where owner_user_id = tests.get_supabase_uid('deletion_owner')
  ),
  'The Storage sweep checkpoint is durable'
);

update private.account_deletion_jobs
set
  lease_expires_at = clock_timestamp() - interval '1 minute',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_owner');

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'An expired worker cannot refresh the Storage sweep checkpoint'
);

select throws_ok(
  $$
    select public.finish_account_deletion(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion completion is unavailable',
  'An expired worker cannot finish account deletion'
);

update private.account_deletion_jobs
set
  lease_expires_at = clock_timestamp() + interval '5 minutes',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_owner');

select ok(
  (
    with retried as (
      select final_sweep_not_before
      from public.begin_account_deletion(
        tests.get_supabase_uid('deletion_owner')
      )
    )
    select retried.final_sweep_not_before = deletion.final_sweep_not_before
      and deletion.prefix_swept_at is not null
    from retried
    join private.account_deletion_jobs as deletion
      on deletion.owner_user_id = tests.get_supabase_uid('deletion_owner')
  ),
  'A retry after the Storage checkpoint remains idempotent'
);

select ok(
  public.mark_account_deletion_stripe_deleted(
    tests.get_supabase_uid('deletion_owner'),
    '00000000-0000-4000-8000-000000000321'::uuid,
    null
  ),
  'The worker checkpoints an account without a Stripe customer'
);

select ok(
  public.mark_account_deletion_e2ee_purged(
    tests.get_supabase_uid('deletion_owner'),
    '00000000-0000-4000-8000-000000000321'::uuid,
    (
      select e2ee_workspace_ids
      from private.account_deletion_jobs
      where owner_user_id = tests.get_supabase_uid('deletion_owner')
    )
  ),
  'The worker checkpoints E2EE cleanup before Auth deletion'
);

select throws_ok(
  $$
    select public.finish_account_deletion(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000399'::uuid
    )
  $$,
  '55000',
  'account deletion completion is unavailable',
  'A stale worker cannot finish account deletion'
);

select throws_ok(
  $$
    select public.finish_account_deletion(
      tests.get_supabase_uid('deletion_owner'),
      '00000000-0000-4000-8000-000000000321'::uuid
    )
  $$,
  '55000',
  'account deletion completion is unavailable',
  'The durable deletion job cannot finish before Auth deletion'
);

select results_eq(
  $$
    select count(*)
    from private.account_deletion_jobs
    where owner_user_id = tests.get_supabase_uid('deletion_owner')
  $$,
  array[1::bigint],
  'A premature finish attempt preserves the durable deletion job'
);

select tests.clear_authentication();
reset role;

delete from auth.users
where id = (
  select owner_user_id
  from account_deletion_test_state
  where name = 'owner_begin'
);

select tests.authenticate_as_service_role();

select is(
  (
    select count(*)
    from private.account_deletion_jobs
    where owner_user_id = (
      select owner_user_id
      from account_deletion_test_state
      where name = 'owner_begin'
    )
  ),
  1::bigint,
  'The deletion checkpoint survives Auth and workspace cascades'
);

select ok(
  (
    select retried.final_sweep_not_before = deletion.final_sweep_not_before
      and not retried.was_created
    from public.begin_account_deletion(
      (
        select owner_user_id
        from account_deletion_test_state
        where name = 'owner_begin'
      )
    ) as retried
    join private.account_deletion_jobs as deletion
      on deletion.owner_user_id = (
        select owner_user_id
        from account_deletion_test_state
        where name = 'owner_begin'
      )
  ),
  'Retries return the durable deletion checkpoint after the workspace is gone'
);

select ok(
  public.finish_account_deletion(
    (
      select owner_user_id
      from account_deletion_test_state
      where name = 'owner_begin'
    ),
    '00000000-0000-4000-8000-000000000321'::uuid
  ),
  'The worker can finish after the Auth user is removed'
);

select ok(
  not public.finish_account_deletion(
    (
      select owner_user_id
      from account_deletion_test_state
      where name = 'owner_begin'
    ),
    '00000000-0000-4000-8000-000000000321'::uuid
  ),
  'Finishing an already removed deletion job is idempotent'
);

insert into account_deletion_test_state (
  name,
  owner_user_id,
  final_sweep_not_before,
  was_created
)
select
  'reclaim_begin',
  owner_user_id,
  final_sweep_not_before,
  was_created
from public.begin_account_deletion(
  tests.get_supabase_uid('deletion_reclaim')
);

select ok(
  (select was_created from account_deletion_test_state where name = 'reclaim_begin'),
  'A second account can enter the deletion queue'
);

update private.account_deletion_jobs
set
  requested_at = clock_timestamp() - interval '2 hours',
  final_sweep_not_before = clock_timestamp() - interval '1 hour',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_reclaim');

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000331'::uuid,
      1,
      30
    )
  ),
  1::bigint,
  'The second account receives an initial deletion lease'
);

update private.account_deletion_jobs
set
  lease_expires_at = clock_timestamp() - interval '1 minute',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_reclaim');

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases(
      '00000000-0000-4000-8000-000000000332'::uuid,
      1,
      300
    )
  ),
  1::bigint,
  'An expired deletion lease can be reclaimed'
);

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_reclaim'),
      '00000000-0000-4000-8000-000000000331'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'The previous worker is fenced out after lease reassignment'
);

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('deletion_reclaim'),
      '00000000-0000-4000-8000-000000000332'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'An old worker cannot checkpoint a prefix before Stripe cleanup'
);

select ok(
  public.mark_account_deletion_stripe_deleted(
    tests.get_supabase_uid('deletion_reclaim'),
    '00000000-0000-4000-8000-000000000332'::uuid,
    null
  ),
  'The replacement worker checkpoints an account without a Stripe customer'
);

select ok(
  public.mark_account_deletion_prefix_swept(
    tests.get_supabase_uid('deletion_reclaim'),
    '00000000-0000-4000-8000-000000000332'::uuid
  ),
  'The replacement worker can confirm the Storage sweep'
);

select ok(
  public.mark_account_deletion_e2ee_purged(
    tests.get_supabase_uid('deletion_reclaim'),
    '00000000-0000-4000-8000-000000000332'::uuid,
    (
      select e2ee_workspace_ids
      from private.account_deletion_jobs
      where owner_user_id = tests.get_supabase_uid('deletion_reclaim')
    )
  ),
  'The replacement worker checkpoints E2EE cleanup'
);

select tests.clear_authentication();
reset role;

delete from auth.users
where id = (
  select owner_user_id
  from account_deletion_test_state
  where name = 'reclaim_begin'
);

select tests.authenticate_as_service_role();

select ok(
  public.finish_account_deletion(
    (
      select owner_user_id
      from account_deletion_test_state
      where name = 'reclaim_begin'
    ),
    '00000000-0000-4000-8000-000000000332'::uuid
  ),
  'The replacement worker can finish account deletion'
);

select * from finish();
rollback;
