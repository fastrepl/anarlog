begin;
select plan(119);

select tests.create_supabase_user('backup_owner', 'backup-owner@example.com');
select tests.create_supabase_user('backup_other', 'backup-other@example.com');
select tests.create_supabase_user('backup_no_e2ee', 'backup-no-e2ee@example.com');
select tests.create_supabase_user('backup_limit', 'backup-limit@example.com');
select tests.create_supabase_user('backup_quota', 'backup-quota@example.com');
select tests.create_supabase_user('backup_count', 'backup-count@example.com');
select tests.create_supabase_user('backup_fence', 'backup-fence@example.com');
select tests.create_supabase_user('backup_fence_account', 'backup-fence-account@example.com');

create temporary table attachment_backup_test_state (
  name text primary key,
  object_id uuid,
  object_key text,
  version_ref text,
  object_state text,
  ciphertext_sha256 text,
  ciphertext_size_bytes bigint,
  reservation_expires_at timestamptz,
  last_signed_at timestamptz,
  upload_expires_at timestamptz,
  cleanup_not_before timestamptz,
  was_created boolean,
  was_finalized boolean,
  displaced_object_id uuid,
  displaced_object_key text,
  was_promoted boolean,
  gc_lease_id uuid,
  gc_lease_expires_at timestamptz
);

grant all on attachment_backup_test_state to anon, authenticated, service_role;

create temporary table attachment_backup_delete_test_state (
  name text primary key,
  outcome text,
  object_id uuid,
  object_key text,
  delete_request_id uuid,
  delete_fence_id uuid,
  delete_generation bigint,
  delete_not_before timestamptz,
  was_created boolean
);

grant all on attachment_backup_delete_test_state
  to anon, authenticated, service_role;

create temporary table attachment_backup_fenced_object (
  name text primary key,
  id uuid not null,
  object_key text not null
);

grant all on attachment_backup_fenced_object
  to anon, authenticated, service_role;

insert into attachment_backup_test_state (name, object_id)
values ('quota_owner', tests.get_supabase_uid('backup_quota'));

select has_table(
  'public',
  'attachment_backup_objects',
  'Attachment backup authority has a durable ledger'
);

select is(
  (select bucket."public" from storage.buckets as bucket where bucket.id = 'attachment-backups'),
  false,
  'Attachment backup bucket is private'
);

select is(
  (select bucket.file_size_limit from storage.buckets as bucket where bucket.id = 'attachment-backups'),
  545259520::bigint,
  'Attachment backup bucket permits only the bounded ciphertext envelope'
);

select results_eq(
  $$
    select unnest(bucket.allowed_mime_types)
    from storage.buckets as bucket
    where bucket.id = 'attachment-backups'
  $$,
  array['application/octet-stream'::text],
  'Attachment backup bucket accepts only opaque ciphertext'
);

select ok(
  (
    select class.relrowsecurity
    from pg_class as class
    join pg_namespace as namespace on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname = 'attachment_backup_objects'
  ),
  'Attachment backup ledger has RLS enabled'
);

select ok(
  not has_table_privilege('anon', 'public.attachment_backup_objects', 'SELECT')
    and not has_table_privilege('authenticated', 'public.attachment_backup_objects', 'SELECT')
    and not has_table_privilege('authenticated', 'public.attachment_backup_objects', 'INSERT')
    and not has_table_privilege('authenticated', 'public.attachment_backup_objects', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.attachment_backup_objects', 'DELETE')
    and has_table_privilege('service_role', 'public.attachment_backup_objects', 'SELECT')
    and has_table_privilege('service_role', 'public.attachment_backup_objects', 'INSERT')
    and has_table_privilege('service_role', 'public.attachment_backup_objects', 'UPDATE')
    and has_table_privilege('service_role', 'public.attachment_backup_objects', 'DELETE'),
  'Only the service role has direct ledger privileges'
);

select ok(
  not has_function_privilege(
      'authenticated',
      'public.read_attachment_backup_by_key(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.read_current_attachment_backup(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.reserve_attachment_backup(uuid,text,text,bigint,smallint)',
      'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'public.mark_attachment_backup_signed(uuid,uuid,text,timestamptz)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.finalize_attachment_backup(uuid,uuid,text,bigint)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_attachment_backup_deleting(uuid,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.promote_attachment_backup(uuid,uuid,text,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.prepare_attachment_backup_download(uuid,text,timestamptz)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_attachment_backup_deleting_by_key(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.claim_attachment_backup_gc_leases(uuid,integer,integer)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.finish_attachment_backup_deletion(uuid,uuid,text,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.read_attachment_backup_by_key(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.read_current_attachment_backup(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.reserve_attachment_backup(uuid,text,text,bigint,smallint)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.mark_attachment_backup_signed(uuid,uuid,text,timestamptz)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.claim_attachment_backup_gc_leases(uuid,integer,integer)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.promote_attachment_backup(uuid,uuid,text,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.prepare_attachment_backup_download(uuid,text,timestamptz)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.mark_attachment_backup_deleting_by_key(uuid,text)',
      'EXECUTE'
    ),
  'Attachment backup RPC authority is service-role-only'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'read_attachment_backup_by_key',
        'read_current_attachment_backup',
        'reserve_attachment_backup',
        'mark_attachment_backup_signed',
        'finalize_attachment_backup',
        'promote_attachment_backup',
        'prepare_attachment_backup_download',
        'mark_attachment_backup_deleting',
        'mark_attachment_backup_deleting_by_key',
        'claim_attachment_backup_gc_leases',
        'finish_attachment_backup_deletion'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  ),
  'Public backup functions are security invokers with an empty search path'
);

select results_eq(
  $$
    select count(*)
    from information_schema.columns
    where table_schema = 'public'
      and table_name = 'attachment_backup_objects'
      and column_name in ('session_id', 'filename', 'sha256', 'content_type')
  $$,
  array[0::bigint],
  'The server ledger stores no plaintext note, filename, MIME, or plaintext digest metadata'
);

select results_eq(
  $$
    select count(*)
    from pg_policies
    where schemaname = 'storage'
      and tablename = 'objects'
      and policyname like 'attachment_backups_deny_client_%'
      and permissive = 'RESTRICTIVE'
      and 'anon' = any(roles)
      and 'authenticated' = any(roles)
  $$,
  array[4::bigint],
  'Restrictive policies deny every direct client operation on backup objects'
);

select results_eq(
  $$
    select count(*)
    from pg_policies
    where schemaname = 'public'
      and tablename = 'attachment_backup_objects'
      and policyname = 'attachment_backup_objects_service_all'
      and 'service_role' = any(roles)
  $$,
  array[1::bigint],
  'The service role is the only ledger policy principal'
);

select tests.authenticate_as('backup_owner');

select throws_ok(
  $$
    insert into storage.objects (bucket_id, name, owner_id)
    values (
      'attachment-backups',
      auth.uid()::text || '/00000000-0000-4000-8000-000000000001.anb1',
      auth.uid()::text
    )
  $$,
  '42501',
  null,
  'Authenticated clients cannot upload directly to the private backup bucket'
);

select results_eq(
  $$select count(*) from storage.objects where bucket_id = 'attachment-backups'$$,
  array[0::bigint],
  'Authenticated clients cannot list private backup objects'
);

select throws_ok(
  $$
    insert into public.attachment_backup_objects (
      owner_user_id,
      attachment_ref,
      version_ref,
      object_key,
      ciphertext_size_bytes
    ) values (
      auth.uid(),
      repeat('A', 43),
      repeat('B', 43),
      auth.uid()::text || '/00000000-0000-4000-8000-000000000001.anb1',
      1
    )
  $$,
  '42501',
  null,
  'Authenticated clients cannot write the backup ledger'
);

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      auth.uid(),
      repeat('A', 43),
      repeat('B', 43),
      1,
      1::smallint
    )
  $$,
  '42501',
  null,
  'Authenticated clients cannot bypass the trusted backup API'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_no_e2ee'),
      repeat('A', 43),
      repeat('B', 43),
      1,
      1::smallint
    )
  $$,
  '42501',
  'active personal E2EE workspace required',
  'Backup cannot start before the personal workspace claims an E2EE identity'
);

do $$
begin
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_owner'),
    'abcdefghijklmnopqrstuv'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_other'),
    'bcdefghijklmnopqrstuvw'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_limit'),
    'cdefghijklmnopqrstuvwx'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_quota'),
    'defghijklmnopqrstuvwxy'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_count'),
    'efghijklmnopqrstuvwxyz'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_fence'),
    'fghijklmnopqrstuvwxyza'
  );
  perform * from public.claim_personal_workspace_e2ee_key(
    tests.get_supabase_uid('backup_fence_account'),
    'ghijklmnopqrstuvwxyzab'
  );
end;
$$;

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      'short',
      repeat('B', 43),
      1,
      1::smallint
    )
  $$,
  '22023',
  'invalid attachment backup reservation',
  'Malformed blinded references are rejected'
);

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('B', 43),
      545259521,
      1::smallint
    )
  $$,
  '22023',
  'invalid attachment backup reservation',
  'Ciphertext larger than the bucket cap is rejected before reservation'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      ciphertext_sha256,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    )
    select
      'first',
      object_id,
      object_key,
      object_state,
      ciphertext_sha256,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('B', 43),
      1048576,
      1::smallint
    )
  $$,
  'A valid encrypted attachment reserves an opaque object atomically'
);

select ok(
  (
    select object_state = 'reserved'
      and ciphertext_sha256 is null
      and ciphertext_size_bytes = 1048576
      and was_created
    from attachment_backup_test_state
    where name = 'first'
  ),
  'A new reservation returns its bounded state and size'
);

select ok(
  (
    select object_key ~ (
      '^'
      || tests.get_supabase_uid('backup_owner')::text
      || '/[0-9a-f]{8}-[0-9a-f]{4}-[47][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\.anb1$'
    )
      and object_key not like '%' || repeat('A', 43) || '%'
      and object_key not like '%' || repeat('B', 43) || '%'
    from attachment_backup_test_state
    where name = 'first'
  ),
  'Object keys expose only the owner prefix and a server-random identifier'
);

select throws_ok(
  format(
    $sql$
      update public.attachment_backup_objects
      set object_key = %L
      where id = %L::uuid
    $sql$,
    tests.get_supabase_uid('backup_owner')::text
      || '/00000000-0000-4000-8000-000000000099.anb1',
    (select object_id from attachment_backup_test_state where name = 'first')
  ),
  '22023',
  'attachment backup identity is immutable',
  'The object key used as the head compare token is immutable'
);

select results_eq(
  format(
    $sql$
      select
        object_id,
        attachment_ref,
        version_ref,
        object_key,
        object_state,
        ciphertext_sha256,
        ciphertext_size_bytes,
        format_version
      from public.read_attachment_backup_by_key(%L::uuid, %L)
    $sql$,
    tests.get_supabase_uid('backup_owner'),
    (select object_key from attachment_backup_test_state where name = 'first')
  ),
  $$
    select
      object_id,
      repeat('A', 43),
      repeat('B', 43),
      object_key,
      'reserved'::text,
      null::text,
      1048576::bigint,
      1::smallint
    from attachment_backup_test_state
    where name = 'first'
  $$,
  'The trusted finalize path can resolve exact owner-scoped reservation metadata'
);

select throws_ok(
  $$
    select *
    from public.read_attachment_backup_by_key(
      tests.get_supabase_uid('backup_owner'),
      'not-a-canonical-object-key'
    )
  $$,
  '22023',
  'invalid attachment backup object key',
  'Object-key reads reject noncanonical or cross-owner paths'
);

select ok(
  (
    select reservation_expires_at > now()
      and cleanup_not_before = reservation_expires_at
    from attachment_backup_test_state
    where name = 'first'
  ),
  'Unsigned reservations have a bounded cleanup horizon'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      ciphertext_sha256,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    )
    select
      'first_retry',
      object_id,
      object_key,
      object_state,
      ciphertext_sha256,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('B', 43),
      1048576,
      1::smallint
    )
  $$,
  'Retrying the same blinded version is idempotent'
);

select ok(
  (
    select retry.object_id = original.object_id
      and retry.object_key = original.object_key
      and retry.ciphertext_sha256 is null
      and not retry.was_created
    from attachment_backup_test_state as retry
    cross join attachment_backup_test_state as original
    where retry.name = 'first_retry'
      and original.name = 'first'
  ),
  'Idempotent reservation returns the existing object without quota duplication'
);

select results_eq(
  $$
    select count(*)
    from public.attachment_backup_objects
    where owner_user_id = tests.get_supabase_uid('backup_owner')
      and version_ref = repeat('B', 43)
  $$,
  array[1::bigint],
  'A blinded version has exactly one owner-scoped ledger row'
);

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('C', 43),
      repeat('B', 43),
      1048576,
      1::smallint
    )
  $$,
  '40001',
  'attachment backup version conflicts with existing reservation',
  'A version reference cannot be rebound to different attachment metadata'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    )
    select
      'unsigned',
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('C', 43),
      repeat('D', 43),
      2048,
      1::smallint
    )
  $$,
  'A second attachment can reserve independently'
);

select throws_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('A', 64),
      now() + interval '1 hour'
    )
  $$,
  '22023',
  'invalid attachment backup ciphertext hash',
  'Upload signing requires a strict lowercase ciphertext SHA-256'
);

select throws_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('a', 64),
      now() - interval '1 second'
    )
  $$,
  '22023',
  'invalid attachment backup upload expiry',
  'Expired signed-upload grants are rejected'
);

select throws_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('a', 64),
      now() + interval '3 hours'
    )
  $$,
  '22023',
  'invalid attachment backup upload expiry',
  'Overlong signed-upload grants are rejected'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      ciphertext_sha256,
      last_signed_at,
      upload_expires_at,
      cleanup_not_before
    )
    select
      'first_signed',
      object_id,
      object_key,
      ciphertext_sha256,
      last_signed_at,
      upload_expires_at,
      cleanup_not_before
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('a', 64),
      now() + interval '2 hours'
    )
  $$,
  'Trusted code records the signed-upload lifetime'
);

select ok(
  (
    select last_signed_at <= upload_expires_at
      and ciphertext_sha256 = repeat('a', 64)
      and cleanup_not_before >= upload_expires_at + interval '24 hours 5 minutes'
      and (
        select exact.ciphertext_sha256
        from public.read_attachment_backup_by_key(
          tests.get_supabase_uid('backup_owner'),
          signed.object_key
        ) as exact
      ) = repeat('a', 64)
    from attachment_backup_test_state as signed
    where signed.name = 'first_signed'
  ),
  'Signing writes the exact hash and protects the signed-token lifetime'
);

select ok(
  (
    select ciphertext_sha256 = repeat('a', 64)
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('a', 64),
      now() + interval '1 hour'
    )
  ),
  'Retrying upload signing with the same ciphertext hash is idempotent'
);

select throws_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      repeat('b', 64),
      now() + interval '1 hour'
    )
  $$,
  '40001',
  'attachment backup ciphertext hash conflicts with reservation',
  'A reservation ciphertext hash is write-once'
);

select throws_ok(
  $$
    select *
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      null,
      1048576
    )
  $$,
  '22023',
  'invalid attachment backup finalization',
  'Finalize rejects missing trusted observations as invalid input'
);

select throws_ok(
  $$
    select *
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'unsigned'),
      (select object_key from attachment_backup_test_state where name = 'unsigned'),
      2048
    )
  $$,
  '55000',
  'attachment backup ciphertext hash is unavailable',
  'A reservation cannot finalize before its ciphertext hash is established'
);

select throws_ok(
  $$
    select *
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      tests.get_supabase_uid('backup_owner')::text || '/00000000-0000-4000-8000-000000000099.anb1',
      1048576
    )
  $$,
  '40001',
  'attachment backup object does not match reservation',
  'Finalize rejects a confused object key'
);

select throws_ok(
  $$
    select *
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      1048575
    )
  $$,
  '40001',
  'attachment backup object does not match reservation',
  'Finalize requires the exact observed ciphertext size'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      was_finalized
    )
    select
      'first_finalized',
      object_id,
      object_key,
      object_state,
      was_finalized
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      1048576
    )
  $$,
  'Exact observed storage metadata finalizes the reservation'
);

select ok(
  (
    select backup.state = 'ready'
      and backup.finalized_at is not null
      and backup.cleanup_not_before >= backup.finalized_at + interval '24 hours'
      and result.was_finalized
    from public.attachment_backup_objects as backup
    join attachment_backup_test_state as result on result.object_id = backup.id
    where result.name = 'first_finalized'
  ),
  'The first exact upload becomes a bounded ready candidate'
);

select ok(
  not (
    select was_finalized
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      1048576
    )
  ),
  'Finalize is idempotent after the exact version is ready'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    )
    select
      'second',
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('E', 43),
      1048577,
      1::smallint
    )
  $$,
  'A new blinded version reserves a new random object key'
);

select lives_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'second'),
      repeat('b', 64),
      now() + interval '2 hours'
    )
  $$,
  'The replacement version receives its own signed-upload horizon'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      was_finalized
    )
    select
      'second_finalized',
      object_id,
      object_key,
      object_state,
      was_finalized
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'second'),
      (select object_key from attachment_backup_test_state where name = 'second'),
      1048577
    )
  $$,
  'Finalizing a replacement records only the newly uploaded version'
);

select ok(
  (
    select result.was_finalized
      and old_backup.state = 'ready'
      and old_backup.deletion_requested_at is null
      and new_backup.state = 'ready'
    from attachment_backup_test_state as result
    join attachment_backup_test_state as original on original.name = 'first'
    join public.attachment_backup_objects as old_backup on old_backup.id = original.object_id
    join public.attachment_backup_objects as new_backup on new_backup.id = result.object_id
    where result.name = 'second_finalized'
  ),
  'Finalize preserves the prior ready backup until desktop commits the new key'
);

select results_eq(
  $$
    select count(*)
    from public.attachment_backup_objects
    where owner_user_id = tests.get_supabase_uid('backup_owner')
      and attachment_ref = repeat('A', 43)
      and state = 'ready'
  $$,
  array[2::bigint],
  'Finalize alone does not choose or delete the canonical backup version'
);

select results_eq(
  $$
    select count(*)
    from public.read_current_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43)
    )
  $$,
  array[0::bigint],
  'A finalized candidate is not current before an explicit promotion'
);

select throws_ok(
  $$
    select *
    from public.read_current_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      'short'
    )
  $$,
  '22023',
  'invalid attachment backup reference',
  'Current-head reads require a canonical blinded attachment reference'
);

select throws_ok(
  $$
    select *
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      null,
      null
    )
  $$,
  '22023',
  'invalid attachment backup candidate key',
  'Promotion rejects a missing trusted candidate key as invalid input'
);

select throws_ok(
  $$
    select *
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      tests.get_supabase_uid('backup_owner')::text || '/00000000-0000-4000-8000-000000000099.anb1',
      null
    )
  $$,
  '40001',
  'attachment backup candidate key does not match',
  'Promotion classifies a stale candidate identity as a conflict'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      version_ref,
      ciphertext_sha256,
      displaced_object_id,
      displaced_object_key,
      was_promoted
    )
    select
      'initial_promote',
      current_object_id,
      current_object_key,
      current_version_ref,
      current_ciphertext_sha256,
      displaced_object_id,
      displaced_object_key,
      was_promoted
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      null
    )
  $$,
  'A null expected head atomically promotes the first finalized candidate'
);

select ok(
  (
    select result.was_promoted
      and result.version_ref = repeat('B', 43)
      and result.ciphertext_sha256 = repeat('a', 64)
      and result.displaced_object_id is null
      and result.displaced_object_key is null
      and promoted.state = 'current'
      and candidate.state = 'ready'
    from attachment_backup_test_state as result
    join public.attachment_backup_objects as promoted on promoted.id = result.object_id
    join attachment_backup_test_state as second on second.name = 'second'
    join public.attachment_backup_objects as candidate on candidate.id = second.object_id
    where result.name = 'initial_promote'
  ),
  'Initial promotion creates exactly one current head without displacing a candidate'
);

select ok(
  (
    select current_object_id = (
        select object_id from attachment_backup_test_state where name = 'first'
      )
      and current_object_key = (
        select object_key from attachment_backup_test_state where name = 'first'
      )
      and current_version_ref = repeat('B', 43)
      and current_ciphertext_sha256 = repeat('a', 64)
      and displaced_object_id is null
      and displaced_object_key is null
      and not was_promoted
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      null
    )
  ),
  'Retrying promotion of the same current object is idempotent'
);

select ok(
  not (
    select was_finalized
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'first'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      1048576
    )
  ),
  'Finalize retries recognize an already promoted current object'
);

select ok(
  (
    select object_state = 'current'
      and ciphertext_sha256 = repeat('a', 64)
      and not was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('B', 43),
      1048576,
      1::smallint
    )
  ),
  'Reservation retries return the stored hash for an already current version'
);

select throws_ok(
  $$
    select *
    from public.prepare_attachment_backup_download(
      tests.get_supabase_uid('backup_owner'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      now() + interval '3 hours'
    )
  $$,
  '22023',
  'invalid attachment backup download request',
  'Download preparation rejects overlong signed-link lifetimes'
);

update public.attachment_backup_objects
set
  created_at = now() - interval '30 hours',
  reservation_expires_at = now() - interval '29 hours',
  last_signed_at = now() - interval '28 hours',
  upload_expires_at = now() - interval '27 hours',
  cleanup_not_before = now() - interval '2 hours',
  finalized_at = now() - interval '26 hours',
  updated_at = now()
where id = (select object_id from attachment_backup_test_state where name = 'first');

select ok(
  (
    select ciphertext_sha256 = repeat('a', 64)
      and cleanup_not_before >= now() + interval '1 hour 4 minutes'
    from public.prepare_attachment_backup_download(
      tests.get_supabase_uid('backup_owner'),
      (select object_key from attachment_backup_test_state where name = 'first'),
      now() + interval '1 hour'
    )
  ),
  'Download preparation extends deletion safety beyond the signed-link lifetime'
);

select throws_ok(
  $$
    select *
    from public.prepare_attachment_backup_download(
      tests.get_supabase_uid('backup_owner'),
      (select object_key from attachment_backup_test_state where name = 'second'),
      now() + interval '1 hour'
    )
  $$,
  '55000',
  'attachment backup current object is unavailable',
  'Only the committed current object can receive a signed download'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      version_ref,
      ciphertext_sha256,
      displaced_object_id,
      displaced_object_key,
      was_promoted
    )
    select
      'replacement_promote',
      current_object_id,
      current_object_key,
      current_version_ref,
      current_ciphertext_sha256,
      displaced_object_id,
      displaced_object_key,
      was_promoted
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'second'),
      (select object_key from attachment_backup_test_state where name = 'second'),
      (select object_key from attachment_backup_test_state where name = 'first')
    )
  $$,
  'Replacement promotion compares the immutable expected current key'
);

select ok(
  (
    select result.was_promoted
      and result.version_ref = repeat('E', 43)
      and result.ciphertext_sha256 = repeat('b', 64)
      and result.displaced_object_id = original.object_id
      and result.displaced_object_key = original.object_key
      and old_backup.state = 'deleting'
      and old_backup.deletion_requested_at is not null
      and new_backup.state = 'current'
    from attachment_backup_test_state as result
    join attachment_backup_test_state as original on original.name = 'first'
    join public.attachment_backup_objects as old_backup on old_backup.id = original.object_id
    join public.attachment_backup_objects as new_backup on new_backup.id = result.object_id
    where result.name = 'replacement_promote'
  ),
  'Successful CAS returns and retires the displaced head only after promotion'
);

select results_eq(
  format(
    $sql$
      select
        object_id,
        version_ref,
        object_key,
        ciphertext_sha256,
        ciphertext_size_bytes,
        format_version
      from public.read_current_attachment_backup(%L::uuid, %L)
    $sql$,
    tests.get_supabase_uid('backup_owner'),
    repeat('A', 43)
  ),
  $$
    select
      object_id,
      repeat('E', 43),
      object_key,
      repeat('b', 64),
      1048577::bigint,
      1::smallint
    from attachment_backup_test_state
    where name = 'second'
  $$,
  'Current-head reads return only the CAS winner'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    )
    select
      'stale_candidate',
      object_id,
      object_key,
      object_state,
      ciphertext_size_bytes,
      reservation_expires_at,
      cleanup_not_before,
      was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      repeat('A', 43),
      repeat('F', 43),
      1048578,
      1::smallint
    )
  $$,
  'A concurrent replacement writer can reserve another candidate'
);

select lives_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'stale_candidate'),
      repeat('c', 64),
      now() + interval '2 hours'
    )
  $$,
  'The concurrent replacement candidate receives an upload horizon'
);

select lives_ok(
  $$
    select *
    from public.finalize_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'stale_candidate'),
      (select object_key from attachment_backup_test_state where name = 'stale_candidate'),
      1048578
    )
  $$,
  'The concurrent replacement upload finalizes independently'
);

select throws_ok(
  $$
    select *
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'stale_candidate'),
      (select object_key from attachment_backup_test_state where name = 'stale_candidate'),
      (select object_key from attachment_backup_test_state where name = 'first')
    )
  $$,
  '40001',
  'attachment backup head changed',
  'A stale concurrent CAS cannot replace the winning current head'
);

select ok(
  (
    select stale.state = 'ready'
      and stale.deletion_requested_at is null
      and current.state = 'current'
    from attachment_backup_test_state as stale_result
    join public.attachment_backup_objects as stale on stale.id = stale_result.object_id
    join attachment_backup_test_state as current_result on current_result.name = 'second'
    join public.attachment_backup_objects as current on current.id = current_result.object_id
    where stale_result.name = 'stale_candidate'
  ),
  'A CAS loser remains an independently GC-bounded ready candidate'
);

update public.attachment_backup_objects
set
  created_at = now() - interval '30 hours',
  reservation_expires_at = now() - interval '29 hours',
  last_signed_at = now() - interval '28 hours',
  upload_expires_at = now() - interval '27 hours',
  cleanup_not_before = now() - interval '2 hours',
  finalized_at = now() - interval '26 hours',
  updated_at = now()
where id = (select object_id from attachment_backup_test_state where name = 'second');

select is(
  (
    select count(*)
    from public.claim_attachment_backup_gc_leases(
      '00000000-0000-4000-8000-000000000200'::uuid,
      10,
      300
    )
  ),
  0::bigint,
  'GC excludes the current head even after its cleanup horizon passes'
);

select throws_ok(
  $$
    select *
    from public.mark_attachment_backup_signed(
      tests.get_supabase_uid('backup_other'),
      (select object_id from attachment_backup_test_state where name = 'second'),
      repeat('b', 64),
      now() + interval '2 hours'
    )
  $$,
  '55000',
  'attachment backup reservation is unavailable',
  'Trusted calls still enforce ledger ownership'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      ciphertext_size_bytes,
      cleanup_not_before,
      was_created
    )
    select
      'unsigned_deleting',
      object_id,
      object_key,
      ciphertext_size_bytes,
      cleanup_not_before,
      was_marked
    from public.mark_attachment_backup_deleting_by_key(
      tests.get_supabase_uid('backup_owner'),
      (select object_key from attachment_backup_test_state where name = 'unsigned')
    )
  $$,
  'Explicit deletion first marks the logical object for cleanup'
);

select ok(
  not (
    select was_marked
    from public.mark_attachment_backup_deleting_by_key(
      tests.get_supabase_uid('backup_owner'),
      (select object_key from attachment_backup_test_state where name = 'unsigned')
    )
  ),
  'Marking an already deleting object is idempotent'
);

select throws_ok(
  $$
    select public.finish_attachment_backup_deletion(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'unsigned'),
      (select object_key from attachment_backup_test_state where name = 'unsigned'),
      null
    )
  $$,
  '55000',
  'attachment backup deletion is unavailable',
  'Ledger quota cannot be released before late upload URLs are harmless'
);

update public.attachment_backup_objects
set
  created_at = now() - interval '30 hours',
  reservation_expires_at = now() - interval '29 hours',
  last_signed_at = now() - interval '28 hours',
  upload_expires_at = now() - interval '27 hours',
  cleanup_not_before = now() - interval '2 hours',
  finalized_at = now() - interval '26 hours',
  updated_at = now()
where id = (select object_id from attachment_backup_test_state where name = 'stale_candidate');

select throws_ok(
  $$select * from public.claim_attachment_backup_gc_leases(null, 1, 300)$$,
  '22023',
  'invalid attachment backup GC lease',
  'GC requires a bounded explicit lease'
);

select lives_ok(
  $$
    insert into attachment_backup_test_state (
      name,
      object_id,
      object_key,
      ciphertext_size_bytes,
      gc_lease_id,
      gc_lease_expires_at
    )
    select
      'gc_claim',
      object_id,
      object_key,
      ciphertext_size_bytes,
      gc_lease_id,
      gc_lease_expires_at
    from public.claim_attachment_backup_gc_leases(
      '00000000-0000-4000-8000-000000000101'::uuid,
      1,
      300
    )
  $$,
  'GC claims an abandoned ready CAS loser with a bounded lease'
);

select ok(
  (
    select backup.state = 'deleting'
      and backup.deletion_requested_at is not null
      and backup.gc_lease_id = '00000000-0000-4000-8000-000000000101'::uuid
      and backup.gc_lease_expires_at > now()
    from public.attachment_backup_objects as backup
    join attachment_backup_test_state as claim on claim.object_id = backup.id
    where claim.name = 'gc_claim'
  ),
  'GC atomically transitions an expired ready candidate to leased deletion'
);

select is(
  (
    select count(*)
    from public.claim_attachment_backup_gc_leases(
      '00000000-0000-4000-8000-000000000102'::uuid,
      1,
      300
    )
  ),
  0::bigint,
  'An active GC lease prevents duplicate workers from claiming the same object'
);

select throws_ok(
  $$
    select public.finish_attachment_backup_deletion(
      tests.get_supabase_uid('backup_owner'),
      (select object_id from attachment_backup_test_state where name = 'gc_claim'),
      (select object_key from attachment_backup_test_state where name = 'gc_claim'),
      '00000000-0000-4000-8000-000000000199'::uuid
    )
  $$,
  '55000',
  'attachment backup deletion is unavailable',
  'Only the current GC lease can finish a claimed deletion'
);

select ok(
  public.finish_attachment_backup_deletion(
    tests.get_supabase_uid('backup_owner'),
    (select object_id from attachment_backup_test_state where name = 'gc_claim'),
    (select object_key from attachment_backup_test_state where name = 'gc_claim'),
    '00000000-0000-4000-8000-000000000101'::uuid
  ),
  'The current GC lease can release the ledger row after physical deletion'
);

select ok(
  not public.finish_attachment_backup_deletion(
    tests.get_supabase_uid('backup_owner'),
    (select object_id from attachment_backup_test_state where name = 'gc_claim'),
    (select object_key from attachment_backup_test_state where name = 'gc_claim'),
    '00000000-0000-4000-8000-000000000101'::uuid
  ),
  'Finishing an already removed ledger row is idempotent'
);

select results_eq(
  $$
    select count(*)
    from public.attachment_backup_objects
    where id = (select object_id from attachment_backup_test_state where name = 'gc_claim')
  $$,
  array[0::bigint],
  'Finished deletion frees the object and its reserved quota'
);

select lives_ok(
  $sql$
    do $body$
    declare
      index integer;
    begin
      for index in 1..5 loop
        perform *
        from public.reserve_attachment_backup(
          tests.get_supabase_uid('backup_limit'),
          lpad('limit-attachment-' || index::text, 43, 'A'),
          lpad('limit-version-' || index::text, 43, 'B'),
          1,
          1::smallint
        );
      end loop;
    end;
    $body$;
  $sql$,
  'An account can hold five active upload reservations'
);

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_limit'),
      lpad('limit-attachment-6', 43, 'A'),
      lpad('limit-version-6', 43, 'B'),
      1,
      1::smallint
    )
  $$,
  '55000',
  'attachment backup reservation limit exceeded',
  'A sixth active upload reservation is rejected'
);

insert into public.attachment_backup_objects (
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  ciphertext_size_bytes,
  ciphertext_sha256,
  state,
  finalized_at
)
select
  tests.get_supabase_uid('backup_quota'),
  lpad('quota-attachment-' || item::text, 43, 'A'),
  lpad('quota-version-' || item::text, 43, 'B'),
  tests.get_supabase_uid('backup_quota')::text
    || '/'
    || extensions.gen_random_uuid()::text
    || '.anb1',
  545259520,
  md5(item::text) || md5(item::text),
  'ready',
  now()
from generate_series(1, 9) as item;

select lives_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_quota'),
      lpad('quota-boundary-attachment', 43, 'A'),
      lpad('quota-boundary-version', 43, 'B'),
      461373440,
      1::smallint
    )
  $$,
  'An account may reserve ciphertext up to exactly 5 GiB'
);

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_quota'),
      lpad('quota-over-attachment', 43, 'A'),
      lpad('quota-over-version', 43, 'B'),
      1,
      1::smallint
    )
  $$,
  '54000',
  'attachment backup storage quota exceeded',
  'The 5 GiB account quota counts reserved and ready ciphertext'
);

insert into public.attachment_backup_objects (
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  ciphertext_size_bytes,
  ciphertext_sha256,
  state,
  finalized_at
)
select
  tests.get_supabase_uid('backup_count'),
  lpad('count-attachment-' || item::text, 43, 'A'),
  lpad('count-version-' || item::text, 43, 'B'),
  tests.get_supabase_uid('backup_count')::text
    || '/'
    || extensions.gen_random_uuid()::text
    || '.anb1',
  1,
  md5(item::text) || md5(item::text),
  'ready',
  now()
from generate_series(1, 10000) as item;

select throws_ok(
  $$
    select *
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_count'),
      lpad('count-over-attachment', 43, 'A'),
      lpad('count-over-version', 43, 'B'),
      1,
      1::smallint
    )
  $$,
  '54000',
  'attachment backup object limit exceeded',
  'An account cannot exceed 10,000 tracked backup objects'
);

select has_table(
  'private',
  'attachment_backup_delete_requests',
  'Attachment deletion requests have a durable private replay ledger'
);

select ok(
  not has_table_privilege(
      'authenticated',
      'private.attachment_backup_delete_requests',
      'SELECT'
    )
    and has_table_privilege(
      'service_role',
      'private.attachment_backup_delete_requests',
      'SELECT'
    )
    and not has_function_privilege(
      'authenticated',
      'public.schedule_attachment_backup_deletion(uuid,text,text,text,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.cancel_attachment_backup_deletion(uuid,text,text,text,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.schedule_attachment_backup_deletion(uuid,text,text,text,uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.cancel_attachment_backup_deletion(uuid,text,text,text,uuid)',
      'EXECUTE'
    ),
  'Delete fencing remains private and service-role-only'
);

with created as (
  insert into public.attachment_backup_objects (
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
    created_at,
    updated_at
  ) values (
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('H', 43),
    tests.get_supabase_uid('backup_fence')::text
      || '/00000000-0000-4000-8000-000000000501.anb1',
    4096,
    repeat('d', 64),
    'current',
    now() - interval '29 hours',
    now() - interval '2 hours',
    now() - interval '28 hours',
    now() - interval '30 hours',
    now() - interval '1 hour'
  )
  returning id, object_key
)
insert into attachment_backup_fenced_object (name, id, object_key)
select 'fenced_object', created.id, created.object_key
from created;

select throws_ok(
  format(
    $sql$
      update public.attachment_backup_objects
      set delete_request_id = %L::uuid
      where id = %L::uuid
    $sql$,
    '00000000-0000-4000-8000-000000000301',
    (select id from attachment_backup_fenced_object)
  ),
  '23514',
  null,
  'Object fences cannot be partially populated'
);

select throws_ok(
  format(
    $sql$
      insert into private.attachment_backup_delete_requests (
        owner_user_id,
        delete_request_id,
        attachment_ref,
        version_ref,
        object_key,
        outcome
      ) values (%L::uuid, %L::uuid, %L, %L, %L, 'dependency_appeared')
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    '00000000-0000-4000-8000-000000000399',
    repeat('G', 43),
    repeat('H', 43),
    (select object_key from attachment_backup_fenced_object)
  ),
  '23514',
  null,
  'Only explicit cancellation tombstones may omit a fence snapshot'
);

select lives_ok(
  format(
    $sql$
      insert into attachment_backup_delete_test_state (
        name,
        outcome,
        object_id,
        object_key,
        delete_request_id,
        delete_fence_id,
        delete_generation,
        delete_not_before,
        was_created
      )
      select
        'request_one',
        outcome,
        object_id,
        object_key,
        delete_request_id,
        delete_fence_id,
        delete_generation,
        delete_not_before,
        was_created
      from public.schedule_attachment_backup_deletion(
        %L::uuid,
        %L,
        %L,
        %L,
        %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('H', 43),
    (select object_key from attachment_backup_fenced_object),
    '00000000-0000-4000-8000-000000000301'
  ),
  'An exact current backup receives a durable conditional deletion fence'
);

select ok(
  (
    select request.outcome = 'scheduled'
      and request.object_id = object.id
      and request.delete_fence_id is not null
      and request.delete_generation = 1
      and request.delete_not_before >= now() + interval '23 hours 59 minutes'
      and request.was_created
    from attachment_backup_delete_test_state as request
    cross join attachment_backup_fenced_object as object
    where request.name = 'request_one'
  ),
  'A new fence records a bounded server generation and grace deadline'
);

select results_eq(
  format(
    $sql$
      select
        outcome,
        object_id,
        object_key,
        delete_request_id,
        delete_fence_id,
        delete_generation,
        delete_not_before,
        was_created
      from public.schedule_attachment_backup_deletion(
        %L::uuid, %L, %L, %L, %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('H', 43),
    (select object_key from attachment_backup_fenced_object),
    '00000000-0000-4000-8000-000000000301'
  ),
  $$
    select
      outcome,
      object_id,
      object_key,
      delete_request_id,
      delete_fence_id,
      delete_generation,
      delete_not_before,
      false
    from attachment_backup_delete_test_state
    where name = 'request_one'
  $$,
  'An ambiguous schedule replay returns the exact fence without extending it'
);

select throws_ok(
  format(
    $sql$
      select * from public.schedule_attachment_backup_deletion(
        %L::uuid, %L, %L, %L, %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('H', 43),
    (select object_key from attachment_backup_fenced_object),
    '00000000-0000-4000-8000-000000000302'
  ),
  '40001',
  'attachment backup deletion is already pending',
  'A competing request cannot replace an active fence'
);

select throws_ok(
  format(
    $sql$
      select * from public.cancel_attachment_backup_deletion(
        %L::uuid, %L, %L, %L, %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('I', 43),
    (select object_key from attachment_backup_fenced_object),
    '00000000-0000-4000-8000-000000000301'
  ),
  '40001',
  'attachment backup delete request conflicts with existing identity',
  'A request identifier cannot be rebound to different blinded metadata'
);

select ok(
  (
    select outcome = 'cancelled' and was_cancelled
    from public.cancel_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000301'::uuid
    )
  ),
  'Exact cancellation terminalizes the active fence'
);

select ok(
  (
    select backup.state = 'current'
      and backup.delete_request_id is null
      and backup.delete_fence_id is null
      and backup.delete_not_before is null
      and backup.delete_generation = 2
      and deletion.outcome = 'cancelled'
    from public.attachment_backup_objects as backup
    join private.attachment_backup_delete_requests as deletion
      on deletion.owner_user_id = backup.owner_user_id
      and deletion.delete_request_id = '00000000-0000-4000-8000-000000000301'::uuid
    where backup.id = (select id from attachment_backup_fenced_object)
  ),
  'Cancellation preserves the current object and advances its server generation'
);

select ok(
  (
    select outcome = 'cancelled'
      and not was_cancelled
    from public.cancel_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000301'::uuid
    )
  ),
  'Exact cancellation replay remains terminal and idempotent'
);

select ok(
  (
    select outcome = 'cancelled'
      and delete_fence_id is null
      and not was_created
    from public.schedule_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000301'::uuid
    )
  ),
  'A canceled request replay cannot re-arm deletion'
);

select ok(
  (
    select outcome = 'cancelled'
      and not was_cancelled
    from public.cancel_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000300'::uuid
    )
  ),
  'Cancellation can durably arrive before scheduling as a distinct tombstone'
);

select ok(
  (
    select outcome = 'cancelled'
      and object_id is null
      and delete_fence_id is null
      and delete_generation is null
      and delete_not_before is null
      and not was_created
    from public.schedule_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000300'::uuid
    )
  ),
  'A cancel-first tombstone deterministically defeats a later schedule'
);

insert into attachment_backup_delete_test_state (
  name,
  outcome,
  object_id,
  object_key,
  delete_request_id,
  delete_fence_id,
  delete_generation,
  delete_not_before,
  was_created
)
select
  'request_two',
  outcome,
  object_id,
  object_key,
  delete_request_id,
  delete_fence_id,
  delete_generation,
  delete_not_before,
  was_created
from public.schedule_attachment_backup_deletion(
  tests.get_supabase_uid('backup_fence'),
  repeat('G', 43),
  repeat('H', 43),
  (select object_key from attachment_backup_fenced_object),
  '00000000-0000-4000-8000-000000000302'::uuid
);

select ok(
  (
    select old.outcome = 'cancelled'
      and current.delete_request_id = newer.delete_request_id
      and current.delete_fence_id = newer.delete_fence_id
    from public.schedule_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000301'::uuid
    ) as old
    cross join attachment_backup_delete_test_state as newer
    join public.attachment_backup_objects as current
      on current.id = newer.object_id
    where newer.name = 'request_two'
  ),
  'An old replay cannot replace a newer request fence'
);

select ok(
  (
    select old.outcome = 'cancelled'
      and not old.was_cancelled
      and current.delete_request_id = newer.delete_request_id
      and current.delete_fence_id = newer.delete_fence_id
    from public.cancel_attachment_backup_deletion(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      (select object_key from attachment_backup_fenced_object),
      '00000000-0000-4000-8000-000000000301'::uuid
    ) as old
    cross join attachment_backup_delete_test_state as newer
    join public.attachment_backup_objects as current
      on current.id = newer.object_id
    where newer.name = 'request_two'
  ),
  'A stale cancellation cannot clear a newer request fence'
);

select ok(
  (
    select object_state = 'current' and not was_created
    from public.reserve_attachment_backup(
      tests.get_supabase_uid('backup_fence'),
      repeat('G', 43),
      repeat('H', 43),
      4096,
      1::smallint
    )
  ),
  'Reusing a current version is an explicit relink'
);

select ok(
  (
    select deletion.outcome = 'dependency_appeared'
      and backup.delete_request_id is null
    from private.attachment_backup_delete_requests as deletion
    join public.attachment_backup_objects as backup
      on backup.id = deletion.object_id
    where deletion.owner_user_id = tests.get_supabase_uid('backup_fence')
      and deletion.delete_request_id = '00000000-0000-4000-8000-000000000302'::uuid
  ),
  'Relinking terminalizes the active delete request before returning current'
);

insert into attachment_backup_delete_test_state
select
  'request_three',
  scheduled.outcome,
  scheduled.object_id,
  scheduled.object_key,
  scheduled.delete_request_id,
  scheduled.delete_fence_id,
  scheduled.delete_generation,
  scheduled.delete_not_before,
  scheduled.was_created
from public.schedule_attachment_backup_deletion(
  tests.get_supabase_uid('backup_fence'),
  repeat('G', 43),
  repeat('H', 43),
  (select object_key from attachment_backup_fenced_object),
  '00000000-0000-4000-8000-000000000303'::uuid
) as scheduled;

select ok(
  (
    select cleanup_not_before >= now() + interval '59 minutes'
    from public.prepare_attachment_backup_download(
      tests.get_supabase_uid('backup_fence'),
      (select object_key from attachment_backup_fenced_object),
      now() + interval '1 hour'
    )
  ),
  'Download preparation remains available during the delete grace window'
);

select ok(
  (
    select deletion.outcome = 'dependency_appeared'
      and backup.delete_request_id is null
    from private.attachment_backup_delete_requests as deletion
    join public.attachment_backup_objects as backup
      on backup.id = deletion.object_id
    where deletion.owner_user_id = tests.get_supabase_uid('backup_fence')
      and deletion.delete_request_id = '00000000-0000-4000-8000-000000000303'::uuid
  ),
  'Download authorization cancels the fence before URL signing'
);

insert into attachment_backup_delete_test_state
select
  'request_four',
  scheduled.outcome,
  scheduled.object_id,
  scheduled.object_key,
  scheduled.delete_request_id,
  scheduled.delete_fence_id,
  scheduled.delete_generation,
  scheduled.delete_not_before,
  scheduled.was_created
from public.schedule_attachment_backup_deletion(
  tests.get_supabase_uid('backup_fence'),
  repeat('G', 43),
  repeat('H', 43),
  (select object_key from attachment_backup_fenced_object),
  '00000000-0000-4000-8000-000000000304'::uuid
) as scheduled;

select ok(
  not (
    select was_promoted
    from public.promote_attachment_backup(
      tests.get_supabase_uid('backup_fence'),
      (select id from attachment_backup_fenced_object),
      (select object_key from attachment_backup_fenced_object),
      null
    )
  ),
  'Reaffirming an already-current candidate is an idempotent relink'
);

select ok(
  (
    select deletion.outcome = 'dependency_appeared'
      and backup.delete_request_id is null
    from private.attachment_backup_delete_requests as deletion
    join public.attachment_backup_objects as backup
      on backup.id = deletion.object_id
    where deletion.owner_user_id = tests.get_supabase_uid('backup_fence')
      and deletion.delete_request_id = '00000000-0000-4000-8000-000000000304'::uuid
  ),
  'Current-candidate promotion terminalizes an active fence'
);

select lives_ok(
  $$
    select *
    from public.claim_attachment_backup_gc_leases(
      '00000000-0000-4000-8000-000000000401'::uuid,
      100,
      300
    )
  $$,
  'GC can scan safely after a fenced request was canceled'
);

select ok(
  (
    select state = 'current'
      and gc_lease_id is null
      and delete_request_id is null
    from public.attachment_backup_objects
    where id = (select id from attachment_backup_fenced_object)
  ),
  'Canceled current objects are excluded from GC'
);

insert into attachment_backup_delete_test_state
select
  'request_five',
  scheduled.outcome,
  scheduled.object_id,
  scheduled.object_key,
  scheduled.delete_request_id,
  scheduled.delete_fence_id,
  scheduled.delete_generation,
  scheduled.delete_not_before,
  scheduled.was_created
from public.schedule_attachment_backup_deletion(
  tests.get_supabase_uid('backup_fence'),
  repeat('G', 43),
  repeat('H', 43),
  (select object_key from attachment_backup_fenced_object),
  '00000000-0000-4000-8000-000000000305'::uuid
) as scheduled;

update public.attachment_backup_objects
set
  cleanup_not_before = now() - interval '2 hours',
  delete_not_before = now() - interval '1 hour',
  updated_at = now()
where id = (select id from attachment_backup_fenced_object);

update private.attachment_backup_delete_requests
set
  created_at = now() - interval '2 hours',
  delete_not_before = now() - interval '1 hour',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('backup_fence')
  and delete_request_id = '00000000-0000-4000-8000-000000000305'::uuid;

select is(
  (
    select count(*)
    from public.claim_attachment_backup_gc_leases(
      '00000000-0000-4000-8000-000000000402'::uuid,
      100,
      300
    ) as claimed
    where claimed.object_id = (select id from attachment_backup_fenced_object)
  ),
  1::bigint,
  'GC claims a due current object only through its exact durable fence'
);

select ok(
  (
    select state = 'deleting'
      and deletion_requested_at is not null
      and delete_request_id is null
      and delete_fence_id is null
      and delete_not_before is null
      and gc_lease_id = '00000000-0000-4000-8000-000000000402'::uuid
    from public.attachment_backup_objects
    where id = (select id from attachment_backup_fenced_object)
  ),
  'A claimed current fence becomes an irreversible leased deletion'
);

select throws_ok(
  format(
    $sql$
      select * from public.cancel_attachment_backup_deletion(
        %L::uuid, %L, %L, %L, %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence'),
    repeat('G', 43),
    repeat('H', 43),
    (select object_key from attachment_backup_fenced_object),
    '00000000-0000-4000-8000-000000000305'
  ),
  '55006',
  'attachment backup deletion is too late to cancel',
  'Cancellation reports a distinct condition after GC acquires deletion'
);

insert into public.attachment_backup_objects (
  owner_user_id,
  attachment_ref,
  version_ref,
  object_key,
  ciphertext_size_bytes,
  ciphertext_sha256,
  state,
  finalized_at
) values (
  tests.get_supabase_uid('backup_fence_account'),
  repeat('I', 43),
  repeat('J', 43),
  tests.get_supabase_uid('backup_fence_account')::text
    || '/00000000-0000-4000-8000-000000000502.anb1',
  8192,
  repeat('e', 64),
  'current',
  now()
);

select lives_ok(
  format(
    $sql$
      select * from public.schedule_attachment_backup_deletion(
        %L::uuid, %L, %L, %L, %L::uuid
      )
    $sql$,
    tests.get_supabase_uid('backup_fence_account'),
    repeat('I', 43),
    repeat('J', 43),
    tests.get_supabase_uid('backup_fence_account')::text
      || '/00000000-0000-4000-8000-000000000502.anb1',
    '00000000-0000-4000-8000-000000000306'
  ),
  'Account cleanup can begin with an active conditional fence'
);

select lives_ok(
  format(
    'select * from public.begin_account_deletion(%L::uuid)',
    tests.get_supabase_uid('backup_fence_account')
  ),
  'Account deletion preserves its existing irreversible cleanup path'
);

select ok(
  (
    select backup.state = 'deleting'
      and backup.deletion_requested_at is not null
      and backup.delete_request_id is null
      and backup.delete_fence_id is null
      and backup.delete_not_before is null
      and deletion.outcome = 'dependency_appeared'
    from public.attachment_backup_objects as backup
    join private.attachment_backup_delete_requests as deletion
      on deletion.owner_user_id = backup.owner_user_id
      and deletion.object_id = backup.id
    where backup.owner_user_id = tests.get_supabase_uid('backup_fence_account')
  ),
  'Account deletion terminalizes active fences before irreversible cleanup'
);

select tests.clear_authentication();
reset role;

select throws_ok(
  $$
    delete from auth.users
    where id = (
      select object_id
      from attachment_backup_test_state
      where name = 'quota_owner'
    )
  $$,
  '55000',
  'account durable cleanup is incomplete',
  'Account deletion remains blocked until durable cleanup completes'
);

select * from finish();
rollback;
