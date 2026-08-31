begin;
select plan(8);

select tests.create_supabase_user(
  'preview_metadata_owner',
  'preview-metadata-owner@example.com'
);

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('preview_metadata_owner');

create temporary table preview_metadata_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  public_slug text
);

grant all on preview_metadata_test_state to anon, authenticated, service_role;

insert into preview_metadata_test_state (name, workspace_id)
values ('workspace', gen_random_uuid());

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('preview_metadata_owner'),
  'shared',
  'Preview metadata workspace'
from preview_metadata_test_state
where name = 'workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('preview_metadata_owner'),
  'owner'
from preview_metadata_test_state
where name = 'workspace';

select tests.enable_workspace_plan(workspace_id)
from preview_metadata_test_state
where name = 'workspace';

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('preview_metadata_owner');

insert into preview_metadata_test_state (name, share_id)
select 'share', share_id
from public.create_session_share(
  (select workspace_id from preview_metadata_test_state where name = 'workspace'),
  'preview-metadata-session'
);

update preview_metadata_test_state
set public_slug = scope.public_slug
from public.set_session_share_scope(
  (select share_id from preview_metadata_test_state where name = 'share'),
  'public',
  null
) as scope
where name = 'share';

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select throws_ok(
  $$
    select *
    from public.publish_session_share_snapshot_with_preview_cas(
      (select share_id from preview_metadata_test_state where name = 'share'),
      tests.get_supabase_uid('preview_metadata_owner'),
      0,
      '30000000-0000-4000-8000-000000000001',
      'Customer review',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"The team aligned on launch scope."}]}]}'::jsonb,
      ARRAY[]::uuid[],
      true,
      ARRAY[repeat('x', 101)],
      '2026-08-06T01:30:00Z'::timestamptz
    )
  $$,
  '22023',
  'invalid session share preview metadata',
  'Invalid preview metadata rejects the combined publication'
);

select is(
  (
    select count(*)
    from public.session_share_snapshots
    where share_id = (
      select share_id from preview_metadata_test_state where name = 'share'
    )
  ),
  0::bigint,
  'A preview metadata failure rolls back the snapshot mutation'
);

select lives_ok(
  $$
    select *
    from public.publish_session_share_snapshot_with_preview_cas(
      (select share_id from preview_metadata_test_state where name = 'share'),
      tests.get_supabase_uid('preview_metadata_owner'),
      0,
      '30000000-0000-4000-8000-000000000002',
      'Customer review',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"The team aligned on launch scope."}]}]}'::jsonb,
      ARRAY[]::uuid[],
      true,
      ARRAY['John Jeong', 'Sungbin Jo'],
      '2026-08-06T01:30:00Z'::timestamptz
    )
  $$,
  'The gateway publishes the snapshot and preview metadata together'
);

select lives_ok(
  $$
    select public.upsert_session_share_preview_metadata(
      (select share_id from preview_metadata_test_state where name = 'share'),
      tests.get_supabase_uid('preview_metadata_owner'),
      ARRAY['Updated Participant'],
      '2026-08-06T02:30:00Z'::timestamptz
    )
  $$,
  'Preview metadata can advance independently of note content'
);

select is(
  (
    select outcome
    from public.publish_session_share_snapshot_with_preview_cas(
      (select share_id from preview_metadata_test_state where name = 'share'),
      tests.get_supabase_uid('preview_metadata_owner'),
      0,
      '30000000-0000-4000-8000-000000000002',
      'Customer review',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"The team aligned on launch scope."}]}]}'::jsonb,
      ARRAY[]::uuid[],
      true,
      ARRAY['John Jeong', 'Sungbin Jo'],
      '2026-08-06T01:30:00Z'::timestamptz
    )
  ),
  'replayed',
  'An exact content replay does not reapply stale preview metadata'
);

select is(
  (
    select preview.body_json #>> '{content,0,content,0,text}'
    from public.gateway_read_public_session_share_preview(
      (select public_slug from preview_metadata_test_state where name = 'share')
    ) as preview
  ),
  'The team aligned on launch scope.',
  'Public previews provide the body for server-side summary extraction'
);

select is(
  (
    select preview.participants
    from public.gateway_read_public_session_share_preview(
      (select public_slug from preview_metadata_test_state where name = 'share')
    ) as preview
  ),
  ARRAY['Updated Participant']::text[],
  'Public previews retain the latest meeting participants after a replay'
);

select is(
  (
    select preview.meeting_at
    from public.gateway_read_public_session_share_preview(
      (select public_slug from preview_metadata_test_state where name = 'share')
    ) as preview
  ),
  '2026-08-06T02:30:00Z'::timestamptz,
  'Public previews retain the latest meeting date after a replay'
);

select * from finish();
rollback;
