begin;
select plan(81);

select tests.create_supabase_user('shared_attachment_owner', 'shared-attachment-owner@example.com');
select tests.create_supabase_user('shared_attachment_recipient', 'shared-attachment-recipient@example.com');
select tests.create_supabase_user('shared_attachment_other', 'shared-attachment-other@example.com');

create temporary table session_share_attachment_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  attachment_id uuid,
  object_key text,
  secret text,
  slug text,
  lease_id uuid
);

grant all on session_share_attachment_test_state to anon, authenticated, service_role;

insert into session_share_attachment_test_state (name, workspace_id)
values ('source_workspace', gen_random_uuid());

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('shared_attachment_owner'),
  tests.get_supabase_uid('shared_attachment_recipient'),
  tests.get_supabase_uid('shared_attachment_other')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('shared_attachment_owner'),
  'shared',
  'Shared attachment source workspace'
from session_share_attachment_test_state
where name = 'source_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('shared_attachment_owner'),
  'owner'
from session_share_attachment_test_state
where name = 'source_workspace';

select tests.clear_authentication();
reset role;

select has_table(
  'public',
  'session_share_attachment_objects',
  'Shared note attachments have a durable object ledger'
);

select has_table(
  'public',
  'session_share_snapshot_attachments',
  'Published snapshots have an explicit attachment manifest'
);

select is(
  (
    select bucket.public
    from storage.buckets as bucket
    where bucket.id = 'shared-note-attachments'
  ),
  false,
  'The shared note attachment bucket is private'
);

select is(
  (
    select bucket.file_size_limit
    from storage.buckets as bucket
    where bucket.id = 'shared-note-attachments'
  ),
  536870912::bigint,
  'The shared note attachment bucket has a bounded object size'
);

select ok(
  (
    select count(*) = 2 and bool_and(class.relrowsecurity)
    from pg_class as class
    join pg_namespace as namespace on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname in (
        'session_share_attachment_objects',
        'session_share_snapshot_attachments'
      )
  ),
  'Both shared attachment authority tables have RLS enabled'
);

select ok(
  not has_table_privilege(
    'anon',
    'public.session_share_attachment_objects',
    'SELECT'
  )
    and not has_table_privilege(
      'authenticated',
      'public.session_share_attachment_objects',
      'SELECT, INSERT, UPDATE, DELETE'
    )
    and not has_table_privilege(
      'authenticated',
      'public.session_share_snapshot_attachments',
      'SELECT, INSERT, UPDATE, DELETE'
    )
    and has_table_privilege(
      'service_role',
      'public.session_share_attachment_objects',
      'SELECT, INSERT, UPDATE, DELETE'
    )
    and has_table_privilege(
      'service_role',
      'public.session_share_snapshot_attachments',
      'SELECT, INSERT, UPDATE, DELETE'
    ),
  'Only trusted service code can access shared attachment authority rows'
);

select results_eq(
  $$
    select count(*)
    from pg_policies
    where schemaname = 'storage'
      and tablename = 'objects'
      and policyname like 'shared_note_attachments_deny_client_%'
      and permissive = 'RESTRICTIVE'
      and 'anon' = any(roles)
      and 'authenticated' = any(roles)
  $$,
  array[4::bigint],
  'Restrictive policies deny every direct client operation in the bucket'
);

select ok(
  not has_function_privilege(
    'authenticated',
    'public.reserve_session_share_attachment(uuid,uuid,text,text,text,text,bigint)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'public.publish_session_share_snapshot_with_attachments(uuid,uuid,text,jsonb,uuid[])',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.prepare_my_session_share_attachment_download(uuid,uuid,uuid,timestamptz)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.gateway_prepare_public_session_share_attachment_download(text,uuid,timestamptz)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.reserve_session_share_attachment(uuid,uuid,text,text,text,text,bigint)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.publish_session_share_snapshot_with_attachments(uuid,uuid,text,jsonb,uuid[])',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.prepare_my_session_share_attachment_download(uuid,uuid,uuid,timestamptz)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.gateway_prepare_public_session_share_attachment_download(text,uuid,timestamptz)',
      'EXECUTE'
    ),
  'Shared attachment RPC authority is service-role-only'
);

select ok(
  has_function_privilege(
    'authenticated',
    'public.read_my_session_share_snapshot_with_attachments(uuid)',
    'EXECUTE'
  )
    and has_function_privilege(
      'authenticated',
      'public.list_my_session_share_snapshots_with_attachments()',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.list_my_session_share_snapshot_page_with_attachments(uuid,integer)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.read_my_session_share_snapshot_with_attachments(uuid)',
      'EXECUTE'
    ),
  'Attachment-aware snapshot reads expose manifests without anonymous access'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'reserve_session_share_attachment',
        'read_session_share_attachment_by_key',
        'mark_session_share_attachment_signed',
        'finalize_session_share_attachment',
        'prepare_my_session_share_attachment_download',
        'gateway_prepare_session_share_link_attachment_download',
        'gateway_prepare_public_session_share_attachment_download',
        'claim_session_share_attachment_gc_leases',
        'finish_session_share_attachment_deletion'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  ),
  'Public shared attachment RPCs are invokers with an empty search path'
);

select ok(
  lower(pg_get_functiondef(
    'private.prepare_session_share_attachment_download(uuid,uuid,timestamptz)'::regprocedure
  )) like '%for share of share%'
    and lower(pg_get_functiondef(
      'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
    )) like '%pg_advisory_xact_lock%'
    and lower(pg_get_functiondef(
      'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
    )) like '%for update%'
    and lower(pg_get_functiondef(
      'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
    )) like '%for share of share%'
    and strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'select handoff.share_id'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'pg_catalog.pg_advisory_xact_lock'
    )
    and strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'pg_catalog.pg_advisory_xact_lock'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'for update'
    )
    and strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'for update'
    ) < strpos(
      lower(pg_get_functiondef(
        'private.prepare_session_share_handoff_attachment_download(text,uuid,timestamptz)'::regprocedure
      )),
      'for share of share'
    )
    and lower(pg_get_functiondef(
      'private.prepare_my_session_share_attachment_download(uuid,uuid,uuid,timestamptz)'::regprocedure
    )) like '%for share of share%'
    and lower(pg_get_functiondef(
      'public.gateway_prepare_session_share_link_attachment_download(uuid,uuid,text,timestamptz)'::regprocedure
    )) like '%for share of share%'
    and lower(pg_get_functiondef(
      'public.gateway_prepare_public_session_share_attachment_download(text,uuid,timestamptz)'::regprocedure
    )) like '%for share of share%',
  'Every attachment grant holds the share lock through authorization and preparation'
);

select tests.authenticate_as_hyprnote_pro('shared_attachment_owner');

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (name, share_id, slug)
    select 'share', share_id, public_slug
    from public.create_session_share(
      (
        select workspace_id
        from session_share_attachment_test_state
        where name = 'source_workspace'
      ),
      'shared-attachment-session'
    )
  $query$,
  'A Pro source workspace owner can create the share fixture'
);

select throws_ok(
  $$select count(*) from public.session_share_attachment_objects$$,
  '42501',
  'permission denied for table session_share_attachment_objects',
  'Authenticated managers cannot directly inspect the attachment ledger'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (
      name,
      attachment_id,
      object_key
    )
    select 'primary', attachment_id, object_key
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'meeting.png',
      'image/png',
      2048
    )
  $query$,
  'A manager can reserve a shared attachment through trusted service code'
);

select ok(
  (
    select attachment.owner_user_id = tests.get_supabase_uid('shared_attachment_owner')
      and attachment.object_key = (
        tests.get_supabase_uid('shared_attachment_owner')::text
        || '/' || attachment.share_id::text
        || '/' || attachment.id::text || '.sna1'
      )
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Shared workspace objects remain owned by the workspace owner account prefix'
);

select results_eq(
  $query$
    select was_created
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'meeting.png',
      'image/png',
      2048
    )
  $query$,
  array[false],
  'The same immutable attachment version reserves idempotently'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'renamed.png',
      'image/png',
      2048
    )
  $query$,
  '40001',
  'shared attachment reservation conflicts',
  'An immutable version cannot be rebound to different metadata'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_other'),
      repeat('C', 43),
      repeat('D', 43),
      'forbidden.png',
      'image/png',
      10
    )
  $query$,
  '42501',
  'session attachment operation not permitted',
  'Trusted service calls still recheck the claimed source manager'
);

select lives_ok(
  $query$
    select *
    from public.mark_session_share_attachment_signed(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      now() + interval '10 minutes',
      repeat('a', 64)
    )
  $query$,
  'A valid upload grant records its expected digest and expiry'
);

select ok(
  (
    select attachment.cleanup_not_before
        >= attachment.upload_expires_at + interval '24 hours 5 minutes'
      and attachment.sha256 = repeat('a', 64)
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Upload grants preserve objects beyond every signed upload URL'
);

select throws_ok(
  $query$
    select *
    from public.finalize_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      (
        select object_key
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      2049,
      'image/png'
    )
  $query$,
  '22023',
  'shared attachment object does not match reservation',
  'Finalization rejects observed object metadata mismatches'
);

select results_eq(
  $query$
    select object_state, was_finalized
    from public.finalize_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      (
        select object_key
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      2048,
      'image/png'
    )
  $query$,
  $$values ('ready'::text, true)$$,
  'Matching Storage metadata finalizes the reserved object exactly once'
);

select ok(
  (
    select attachment.state = 'ready'
      and attachment.finalized_at is not null
      and attachment.deletion_requested_at is null
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Finalized objects satisfy the ready-state lifecycle invariant'
);

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Unavailable attachment',
      '{"type":"doc","content":[]}'::jsonb,
      array[gen_random_uuid()]
    )
  $query$,
  '55000',
  'shared snapshot attachment is unavailable',
  'Snapshot publication rejects attachments outside the ready share inventory'
);

select throws_ok(
  $query$
    select *
    from public.publish_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Duplicate attachment',
      '{"type":"doc","content":[]}'::jsonb,
      array[
        (
          select attachment_id
          from session_share_attachment_test_state
          where name = 'primary'
        ),
        (
          select attachment_id
          from session_share_attachment_test_state
          where name = 'primary'
        )
      ]
    )
  $query$,
  '22023',
  'invalid session share snapshot',
  'Snapshot manifests reject duplicate attachment identifiers'
);

select lives_ok(
  $query$
    select *
    from public.publish_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Meeting with attachment',
      '{"type":"doc","content":[{"type":"image","attrs":{"sharedAttachmentId":"server-owned"}}]}'::jsonb,
      array[
        (
          select attachment_id
          from session_share_attachment_test_state
          where name = 'primary'
        )
      ]
    )
  $query$,
  'Ready attachment selection publishes atomically with the snapshot'
);

select is(
  (
    select private.session_share_attachment_manifest(share_id)
    from session_share_attachment_test_state
    where name = 'share'
  ),
  jsonb_build_array(
    jsonb_build_object(
      'id', (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      'filename', 'meeting.png',
      'contentType', 'image/png',
      'sizeBytes', 2048,
      'sha256', repeat('a', 64)
    )
  ),
  'Published snapshot reads return only the server-owned attachment manifest'
);

select results_eq(
  $query$
    select attachment_id, position
    from public.session_share_snapshot_attachments
    where share_id = (
      select share_id
      from session_share_attachment_test_state
      where name = 'share'
    )
  $query$,
  $query$
    values (
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      0::smallint
    )
  $query$,
  'The authoritative manifest preserves attachment order'
);

select lives_ok(
  $query$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Legacy snapshot update',
      '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
    )
  $query$,
  'Legacy publication can update content without replacing the attachment manifest'
);

select ok(
  (
    select attachment.state = 'ready'
      and exists (
        select 1
        from public.session_share_snapshot_attachments as binding
        where binding.share_id = attachment.share_id
          and binding.attachment_id = attachment.id
      )
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Omitted attachment IDs preserve ready objects and their published bindings'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('shared_attachment_owner');

select results_eq(
  $query$
    select share_id, jsonb_array_length(attachments_json)
    from public.read_my_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      )
    )
  $query$,
  $query$
    values (
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      1
    )
  $query$,
  'The source manager reads the authoritative attachment manifest'
);

select results_eq(
  $query$
    select share_id, jsonb_array_length(attachments_json)
    from public.list_my_session_share_snapshots_with_attachments()
    where share_id = (
      select share_id
      from session_share_attachment_test_state
      where name = 'share'
    )
  $query$,
  $query$
    values (
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      1
    )
  $query$,
  'The authenticated attachment-aware list includes manifests'
);

select results_eq(
  $query$
    select share_id, jsonb_array_length(attachments_json)
    from public.list_my_session_share_snapshot_page_with_attachments(null, 1)
    where share_id = (
      select share_id
      from session_share_attachment_test_state
      where name = 'share'
    )
  $query$,
  $query$
    values (
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      1
    )
  $query$,
  'The authenticated attachment-aware page preserves cursor ordering'
);

select lives_ok(
  $query$
    update session_share_attachment_test_state
    set slug = scope.public_slug
    from public.set_session_share_scope(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      'public'
    ) as scope
    where name = 'share'
  $query$,
  'A manager can expose the snapshot through public scope'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('shared_attachment_recipient');

select results_eq(
  $query$
    select share_id, jsonb_array_length(attachments_json)
    from public.read_my_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      )
    )
  $query$,
  $query$
    values (
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      1
    )
  $query$,
  'An authenticated public-scope viewer receives the v2 attachment manifest'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $query$
    select attachment_id
    from public.gateway_prepare_public_session_share_attachment_download(
      (
        select slug
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      now() + interval '60 seconds'
    )
  $query$,
  $query$
    values (
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      )
    )
  $query$,
  'Public download authorization returns the currently published object'
);

select results_eq(
  $query$
    select attachment_id
    from public.prepare_my_session_share_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      tests.get_supabase_uid('shared_attachment_recipient'),
      now() + interval '60 seconds'
    )
  $query$,
  $query$
    values (
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      )
    )
  $query$,
  'Authenticated viewers inherit public attachment access'
);

select results_eq(
  $query$
    select count(*)
    from public.gateway_prepare_public_session_share_attachment_download(
      (
        select slug
        from session_share_attachment_test_state
        where name = 'share'
      ),
      gen_random_uuid(),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'Unpublished attachment identifiers never authorize a download'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('shared_attachment_owner');

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (name, share_id, secret)
    select 'active_link', share_id, link_token
    from public.enable_session_share_link(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      )
    )
  $query$,
  'A manager can switch the published snapshot to bearer-link access'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $query$
    select attachment_id
    from public.gateway_prepare_session_share_link_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      (
        select secret
        from session_share_attachment_test_state
        where name = 'active_link'
      ),
      now() + interval '60 seconds'
    )
  $query$,
  $query$
    values (
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      )
    )
  $query$,
  'A valid bearer token authorizes only its published attachment'
);

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (name, share_id, secret, lease_id)
    select
      'handoff',
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      request_id,
      gen_random_uuid()
    from public.gateway_create_session_share_link_handoff(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select secret
        from session_share_attachment_test_state
        where name = 'active_link'
      ),
      repeat('a', 64)
    )
  $query$,
  'A valid bearer viewer can create a desktop handoff request'
);

select results_eq(
  $query$
    select
      jsonb_array_length(attachments_json),
      lease_expires_at > now(),
      to_jsonb(claimed) ? 'attachment_downloads_json'
    from public.gateway_lease_session_share_handoff(
      (
        select secret
        from session_share_attachment_test_state
        where name = 'handoff'
      ),
      (
        select lease_id::text
        from session_share_attachment_test_state
        where name = 'handoff'
      )
    ) as claimed
  $query$,
  $query$
    values (1, true, false)
  $query$,
  'A leased handoff returns the manifest without preparing download grants'
);

select results_eq(
  $query$
    select
      attachment_id,
      cleanup_not_before >= now() + interval '5 minutes'
    from public.gateway_prepare_session_share_handoff_attachment_download(
      (
        select lease_id::text
        from session_share_attachment_test_state
        where name = 'handoff'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      now() + interval '60 seconds'
    )
  $query$,
  $query$
    values ((
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    ), true)
  $query$,
  'A lease prepares only the requested manifest attachment just in time'
);

select results_eq(
  $query$
    select count(*)
    from public.gateway_prepare_session_share_link_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      repeat('Z', 43),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'An invalid bearer token cannot authorize attachment access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('shared_attachment_owner');

select lives_ok(
  $query$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      'restricted'
    )
  $query$,
  'A manager can revoke all general attachment access'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('shared_attachment_recipient');

select results_eq(
  $query$
    select count(*)
    from public.read_my_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      )
    )
  $query$,
  array[0::bigint],
  'Scope revocation removes the snapshot from attachment-aware reads'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $query$
    select count(*)
    from public.gateway_prepare_session_share_link_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      (
        select secret
        from session_share_attachment_test_state
        where name = 'active_link'
      ),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'Scope revocation immediately invalidates bearer attachment access'
);

select results_eq(
  $query$
    select count(*)
    from public.gateway_prepare_session_share_handoff_attachment_download(
      (
        select lease_id::text
        from session_share_attachment_test_state
        where name = 'handoff'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'Scope revocation immediately invalidates an active attachment lease'
);

select lives_ok(
  $query$
    select *
    from public.publish_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Meeting without attachments',
      '{"type":"doc","content":[]}'::jsonb,
      array[]::uuid[]
    )
  $query$,
  'Republishing can atomically remove every shared attachment'
);

select ok(
  (
    select attachment.state = 'deleting'
      and attachment.deletion_requested_at is not null
      and not exists (
        select 1
        from public.session_share_snapshot_attachments as binding
        where binding.share_id = attachment.share_id
          and binding.attachment_id = attachment.id
      )
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Removed snapshot objects are retired and unbound in the publication transaction'
);

select results_eq(
  $query$
    select count(*)
    from public.prepare_my_session_share_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'Even the owner cannot download an attachment removed from the current snapshot'
);

update public.session_share_attachment_objects
set
  gc_lease_id = '00000000-0000-4000-8000-000000000713'::uuid,
  gc_lease_expires_at = clock_timestamp() + interval '5 minutes',
  updated_at = clock_timestamp()
where id = (
  select attachment_id
  from session_share_attachment_test_state
  where name = 'primary'
);

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'renamed.png',
      'image/png',
      2048
    )
  $query$,
  '40001',
  'shared attachment reservation conflicts',
  'A deleting immutable version still rejects conflicting metadata'
);

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (
      name,
      attachment_id,
      object_key
    )
    select 'primary_reshare', attachment_id, object_key
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'meeting.png',
      'image/png',
      2048
    )
  $query$,
  'A deleting immutable version can be reserved again immediately'
);

select ok(
  (
    select replacement.attachment_id <> tombstone.attachment_id
      and replacement.object_key <> tombstone.object_key
    from session_share_attachment_test_state as replacement
    cross join session_share_attachment_test_state as tombstone
    where replacement.name = 'primary_reshare'
      and tombstone.name = 'primary'
  ),
  'Re-sharing a deleting version creates a fresh object ID and key'
);

select ok(
  (
    select attachment.state = 'deleting'
      and attachment.gc_lease_id = '00000000-0000-4000-8000-000000000713'::uuid
      and attachment.gc_lease_expires_at > now()
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary'
    )
  ),
  'Re-sharing preserves the deleting tombstone and its active GC lease'
);

select lives_ok(
  $query$
    select *
    from public.mark_session_share_attachment_signed(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'primary_reshare'
      ),
      now() + interval '10 minutes',
      repeat('a', 64)
    )
  $query$,
  'The replacement object can receive a bounded upload grant'
);

select results_eq(
  $query$
    select attachment_id, object_key, was_created
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('A', 43),
      repeat('B', 43),
      'meeting.png',
      'image/png',
      2048
    )
  $query$,
  $query$
    select attachment_id, object_key, false
    from session_share_attachment_test_state
    where name = 'primary_reshare'
  $query$,
  'Retrying the replacement reservation remains idempotent'
);

select throws_ok(
  $query$
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
      created_at,
      updated_at
    )
    select
      '00000000-0000-4000-8000-000000000703'::uuid,
      attachment.share_id,
      attachment.owner_user_id,
      attachment.attachment_ref,
      attachment.version_ref,
      attachment.owner_user_id::text
        || '/' || attachment.share_id::text
        || '/00000000-0000-4000-8000-000000000703.sna1',
      attachment.filename,
      attachment.content_type,
      attachment.size_bytes,
      'reserved',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp(),
      statement_timestamp()
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary_reshare'
    )
  $query$,
  '23505',
  'duplicate key value violates unique constraint "session_share_attachment_objects_active_version_key"',
  'The active-version index rejects a second reserved row'
);

select throws_ok(
  $query$
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
      created_at,
      updated_at
    )
    select
      '00000000-0000-4000-8000-000000000704'::uuid,
      attachment.share_id,
      attachment.owner_user_id,
      attachment.attachment_ref,
      attachment.version_ref,
      attachment.owner_user_id::text
        || '/' || attachment.share_id::text
        || '/00000000-0000-4000-8000-000000000704.sna1',
      attachment.filename,
      attachment.content_type,
      attachment.size_bytes,
      repeat('a', 64),
      'ready',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp(),
      statement_timestamp(),
      statement_timestamp()
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary_reshare'
    )
  $query$,
  '23505',
  'duplicate key value violates unique constraint "session_share_attachment_objects_active_version_key"',
  'The active-version index rejects a second ready row'
);

select lives_ok(
  $query$
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
      created_at,
      updated_at
    )
    select
      '00000000-0000-4000-8000-000000000705'::uuid,
      attachment.share_id,
      attachment.owner_user_id,
      attachment.attachment_ref,
      attachment.version_ref,
      attachment.owner_user_id::text
        || '/' || attachment.share_id::text
        || '/00000000-0000-4000-8000-000000000705.sna1',
      attachment.filename,
      attachment.content_type,
      attachment.size_bytes,
      'deleting',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp() + interval '15 minutes',
      statement_timestamp(),
      statement_timestamp(),
      statement_timestamp()
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'primary_reshare'
    )
  $query$,
  'The active-version index permits another deleting tombstone'
);

select results_eq(
  $query$
    select count(*)
    from public.session_share_attachment_objects as attachment
    where attachment.share_id = (
      select share_id
      from session_share_attachment_test_state
      where name = 'share'
    )
      and attachment.version_ref = repeat('B', 43)
      and attachment.state = 'deleting'
  $query$,
  array[2::bigint],
  'Multiple deleting tombstones can retain the same immutable version'
);

select lives_ok(
  $query$
    insert into session_share_attachment_test_state (
      name,
      attachment_id,
      object_key
    )
    select 'secondary', attachment_id, object_key
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('C', 43),
      repeat('D', 43),
      'audio.m4a',
      'audio/mp4',
      4096
    )
  $query$,
  'A second immutable object can be reserved after the first is retired'
);

select lives_ok(
  $query$
    select *
    from public.mark_session_share_attachment_signed(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'secondary'
      ),
      now() + interval '10 minutes',
      repeat('b', 64)
    )
  $query$,
  'The second object receives a bounded upload grant'
);

select lives_ok(
  $query$
    select *
    from public.finalize_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'secondary'
      ),
      (
        select object_key
        from session_share_attachment_test_state
        where name = 'secondary'
      ),
      4096,
      'audio/mp4'
    )
  $query$,
  'The second object finalizes after its metadata is verified'
);

select lives_ok(
  $query$
    select *
    from public.publish_session_share_snapshot_with_attachments(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      'Meeting audio',
      '{"type":"doc","content":[]}'::jsonb,
      array[
        (
          select attachment_id
          from session_share_attachment_test_state
          where name = 'secondary'
        )
      ]
    )
  $query$,
  'The second object can become the current published manifest'
);

update public.session_shares
set deleted_at = now()
where id = (
  select share_id
  from session_share_attachment_test_state
  where name = 'share'
);

select ok(
  (
    select attachment.state = 'deleting'
      and attachment.deletion_requested_at is not null
      and not exists (
        select 1
        from public.session_share_snapshot_attachments as binding
        where binding.share_id = attachment.share_id
      )
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'secondary'
    )
  ),
  'Soft-deleting a share retires and unbinds every published object'
);

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
  created_at,
  updated_at
)
select
  '00000000-0000-4000-8000-000000000701'::uuid,
  share_id,
  tests.get_supabase_uid('shared_attachment_owner'),
  repeat('E', 43),
  repeat('F', 43),
  tests.get_supabase_uid('shared_attachment_owner')::text
    || '/' || share_id::text
    || '/00000000-0000-4000-8000-000000000701.sna1',
  'expired.bin',
  'application/octet-stream',
  512,
  'reserved',
  now() - interval '1 hour',
  now() - interval '1 hour',
  now() - interval '2 hours',
  now() - interval '1 hour'
from session_share_attachment_test_state
where name = 'share';

insert into session_share_attachment_test_state (
  name,
  attachment_id,
  object_key,
  lease_id
)
values (
  'expired',
  '00000000-0000-4000-8000-000000000701'::uuid,
  (
    select object_key
    from public.session_share_attachment_objects
    where id = '00000000-0000-4000-8000-000000000701'::uuid
  ),
  '00000000-0000-4000-8000-000000000711'::uuid
);

select results_eq(
  $query$
    select attachment_id, gc_lease_id
    from public.claim_session_share_attachment_gc_leases(
      (
        select lease_id
        from session_share_attachment_test_state
        where name = 'expired'
      ),
      1,
      300
    )
  $query$,
  $query$
    values (
      '00000000-0000-4000-8000-000000000701'::uuid,
      '00000000-0000-4000-8000-000000000711'::uuid
    )
  $query$,
  'Expired unbound reservations receive fenced GC leases'
);

select is(
  (
    select attachment.state
    from public.session_share_attachment_objects as attachment
    where attachment.id = '00000000-0000-4000-8000-000000000701'::uuid
  ),
  'deleting',
  'A GC lease removes an object from the publishable ready state before storage deletion'
);

select throws_ok(
  $query$
    select public.finish_session_share_attachment_deletion(
      '00000000-0000-4000-8000-000000000701'::uuid,
      (
        select object_key
        from session_share_attachment_test_state
        where name = 'expired'
      ),
      gen_random_uuid()
    )
  $query$,
  '55000',
  'shared attachment deletion is unavailable',
  'A stale GC worker cannot acknowledge another worker lease'
);

select is(
  public.finish_session_share_attachment_deletion(
    '00000000-0000-4000-8000-000000000701'::uuid,
    (
      select object_key
      from session_share_attachment_test_state
      where name = 'expired'
    ),
    '00000000-0000-4000-8000-000000000711'::uuid
  ),
  true,
  'The current GC worker can delete an unbound expired ledger row'
);

select results_eq(
  $$
    select count(*)
    from public.session_share_attachment_objects
    where id = '00000000-0000-4000-8000-000000000701'::uuid
  $$,
  array[0::bigint],
  'GC completion removes the durable object row after physical deletion'
);

update public.session_shares
set deleted_at = null
where id = (
  select share_id
  from session_share_attachment_test_state
  where name = 'share'
);

update public.session_share_attachment_objects
set
  state = 'ready',
  deletion_requested_at = null,
  gc_lease_id = '00000000-0000-4000-8000-000000000719'::uuid,
  gc_lease_expires_at = now() + interval '5 minutes'
where id = (
  select attachment_id
  from session_share_attachment_test_state
  where name = 'secondary'
);

insert into public.session_share_snapshot_attachments (
  share_id,
  attachment_id,
  position
)
select
  share.share_id,
  attachment.attachment_id,
  0
from session_share_attachment_test_state as share
cross join session_share_attachment_test_state as attachment
where share.name = 'share'
  and attachment.name = 'secondary';

select lives_ok(
  $$
    select *
    from public.begin_account_deletion(
      tests.get_supabase_uid('shared_attachment_owner')
    )
  $$,
  'Account deletion enrolls shared attachment objects in durable cleanup'
);

select ok(
  (
    select attachment.state = 'deleting'
      and attachment.deletion_requested_at is not null
      and attachment.gc_lease_id is null
      and attachment.gc_lease_expires_at is null
      and not exists (
        select 1
        from public.session_share_snapshot_attachments as binding
        where binding.share_id = attachment.share_id
          and binding.attachment_id = attachment.id
      )
    from public.session_share_attachment_objects as attachment
    where attachment.id = (
      select attachment_id
      from session_share_attachment_test_state
      where name = 'secondary'
    )
  ),
  'Account deletion unbinds shared files and clears their cleanup leases'
);

update public.workspace_memberships
set deleted_at = null
where workspace_id = (
  select workspace_id
  from session_share_attachment_test_state
  where name = 'source_workspace'
)
  and user_id = tests.get_supabase_uid('shared_attachment_owner');

select throws_ok(
  $query$
    select *
    from public.reserve_session_share_attachment(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      repeat('G', 43),
      repeat('H', 43),
      'blocked.bin',
      'application/octet-stream',
      16
    )
  $query$,
  '42501',
  'session attachment operation not permitted',
  'A deletion job blocks trusted attachment writes despite a stale active membership'
);

update public.session_share_attachment_objects
set
  state = 'ready',
  deletion_requested_at = null
where id = (
  select attachment_id
  from session_share_attachment_test_state
  where name = 'secondary'
);

insert into public.session_share_snapshot_attachments (
  share_id,
  attachment_id,
  position
)
select
  share.share_id,
  attachment.attachment_id,
  0
from session_share_attachment_test_state as share
cross join session_share_attachment_test_state as attachment
where share.name = 'share'
  and attachment.name = 'secondary';

select results_eq(
  $query$
    select count(*)
    from public.prepare_my_session_share_attachment_download(
      (
        select share_id
        from session_share_attachment_test_state
        where name = 'share'
      ),
      (
        select attachment_id
        from session_share_attachment_test_state
        where name = 'secondary'
      ),
      tests.get_supabase_uid('shared_attachment_owner'),
      now() + interval '60 seconds'
    )
  $query$,
  array[0::bigint],
  'A deletion job blocks attachment reads despite restored stale access rows'
);

select ok(
  (
    select deletion.final_sweep_not_before >= max(attachment.cleanup_not_before)
    from private.account_deletion_jobs as deletion
    join public.session_share_attachment_objects as attachment
      on attachment.owner_user_id = deletion.owner_user_id
    where deletion.owner_user_id = tests.get_supabase_uid('shared_attachment_owner')
    group by deletion.final_sweep_not_before
  ),
  'Account deletion waits beyond every shared attachment cleanup horizon'
);

update private.account_deletion_jobs
set
  requested_at = now() - interval '2 minutes',
  final_sweep_not_before = now() - interval '1 minute',
  updated_at = now()
where owner_user_id = tests.get_supabase_uid('shared_attachment_owner');

select ok(
  (
    select not deletion.stripe_deleted and not deletion.cleanup_ready
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-4000-8000-000000000721'::uuid,
      10,
      300
    ) as deletion
    where deletion.owner_user_id = tests.get_supabase_uid('shared_attachment_owner')
  ),
  'Stripe cleanup can start while shared attachment rows keep durable cleanup pending'
);

select ok(
  public.mark_account_deletion_stripe_deleted(
    tests.get_supabase_uid('shared_attachment_owner'),
    '00000000-0000-4000-8000-000000000721'::uuid,
    null
  ),
  'Shared attachment cleanup checkpoints Stripe before the Storage prefix'
);

delete from public.session_share_snapshot_attachments
where share_id = (
  select share_id
  from session_share_attachment_test_state
  where name = 'share'
);

delete from public.session_share_attachment_objects
where owner_user_id = tests.get_supabase_uid('shared_attachment_owner');

update private.account_deletion_jobs
set
  lease_expires_at = now() - interval '1 minute',
  updated_at = now()
where owner_user_id = tests.get_supabase_uid('shared_attachment_owner');

select ok(
  (
    select deletion.stripe_deleted and deletion.cleanup_ready
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-4000-8000-000000000722'::uuid,
      10,
      300
    ) as deletion
    where deletion.owner_user_id = tests.get_supabase_uid('shared_attachment_owner')
  ),
  'Durable cleanup becomes ready after every shared ledger row is removed'
);

insert into storage.objects (bucket_id, name, owner, owner_id)
values (
  'shared-note-attachments',
  tests.get_supabase_uid('shared_attachment_owner')::text
    || '/orphaned-shared-file.sna1',
  null,
  null
);

select throws_ok(
  $$
    select public.mark_account_deletion_prefix_swept(
      tests.get_supabase_uid('shared_attachment_owner'),
      '00000000-0000-4000-8000-000000000722'::uuid
    )
  $$,
  '55000',
  'account deletion sweep is unavailable',
  'The Storage sweep detects ownerless shared files by their physical prefix'
);

update storage.objects
set name = 'removed-shared-file/' || name
where bucket_id = 'shared-note-attachments'
  and name = tests.get_supabase_uid('shared_attachment_owner')::text
    || '/orphaned-shared-file.sna1';

select ok(
  public.mark_account_deletion_prefix_swept(
    tests.get_supabase_uid('shared_attachment_owner'),
    '00000000-0000-4000-8000-000000000722'::uuid
  ),
  'The account deletion worker can checkpoint an empty shared-file prefix'
);

select * from finish();
rollback;
