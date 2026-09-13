begin;
select plan(8);

select tests.create_supabase_user('gallery_owner', 'gallery-owner@example.com');
select tests.create_supabase_user('gallery_member', 'gallery-member@example.com');

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('gallery_owner'),
  tests.get_supabase_uid('gallery_member')
);

create temporary table managed_share_gallery_test_state (
  workspace_id uuid primary key
);

grant all on managed_share_gallery_test_state
  to anon, authenticated, service_role;

insert into managed_share_gallery_test_state (workspace_id)
values ('31000000-0000-4000-8000-000000000001');

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('gallery_owner'),
  'shared',
  'Managed share gallery workspace'
from managed_share_gallery_test_state;

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('gallery_owner'),
  'owner'
from managed_share_gallery_test_state
union all
select
  workspace_id,
  tests.get_supabase_uid('gallery_member'),
  'member'
from managed_share_gallery_test_state;

insert into public.session_shares (
  id,
  workspace_id,
  session_id,
  created_by_user_id,
  general_scope,
  created_at,
  updated_at
)
select
  share.id,
  state.workspace_id,
  share.session_id,
  tests.get_supabase_uid('gallery_owner'),
  share.general_scope,
  share.share_created_at,
  share.share_updated_at
from managed_share_gallery_test_state as state
cross join (
  values
    (
      '31000000-0000-4000-8000-000000000011'::uuid,
      'gallery-session-1',
      'restricted',
      '2026-09-14T01:00:00Z'::timestamptz,
      '2026-09-14T09:00:00Z'::timestamptz
    ),
    (
      '31000000-0000-4000-8000-000000000012'::uuid,
      'gallery-session-2',
      'link',
      '2026-09-14T02:00:00Z'::timestamptz,
      '2026-09-14T08:00:00Z'::timestamptz
    ),
    (
      '31000000-0000-4000-8000-000000000013'::uuid,
      'gallery-session-3',
      'public',
      '2026-09-14T03:00:00Z'::timestamptz,
      '2026-09-14T07:00:00Z'::timestamptz
    ),
    (
      '31000000-0000-4000-8000-000000000014'::uuid,
      'gallery-session-4',
      'restricted',
      '2026-09-14T04:00:00Z'::timestamptz,
      '2026-09-14T06:00:00Z'::timestamptz
    )
) as share(
  id,
  session_id,
  general_scope,
  share_created_at,
  share_updated_at
);

insert into public.session_share_snapshots (
  share_id,
  title,
  body_json,
  published_by_user_id,
  published_at,
  updated_at
)
values
  (
    '31000000-0000-4000-8000-000000000011',
    'Alpha kickoff',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"First preview"}]}]}'::jsonb,
    tests.get_supabase_uid('gallery_owner'),
    '2026-09-14T01:00:00Z',
    '2026-09-14T01:00:00Z'
  ),
  (
    '31000000-0000-4000-8000-000000000012',
    'Beta review',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Second preview"}]}]}'::jsonb,
    tests.get_supabase_uid('gallery_owner'),
    '2026-09-14T02:00:00Z',
    '2026-09-14T02:00:00Z'
  ),
  (
    '31000000-0000-4000-8000-000000000013',
    'Alpha launch',
    '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Third preview"}]}]}'::jsonb,
    tests.get_supabase_uid('gallery_owner'),
    '2026-09-14T03:00:00Z',
    '2026-09-14T03:00:00Z'
  );

select ok(
  has_function_privilege(
    'authenticated',
    'public.list_my_managed_share_gallery_page(text,timestamptz,uuid,integer)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'anon',
      'public.list_my_managed_share_gallery_page(text,timestamptz,uuid,integer)',
      'EXECUTE'
    ),
  'Only authenticated users can call the gallery page function'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('gallery_owner');

select is(
  (
    select array_agg(page.share_id order by page.published_at desc)
    from public.list_my_managed_share_gallery_page(null, null, null, 2) as page
  ),
  array[
    '31000000-0000-4000-8000-000000000014'::uuid,
    '31000000-0000-4000-8000-000000000013'::uuid
  ],
  'The first page preserves snapshotless shares and orders by snapshot or management time'
);

select results_eq(
  $$
    select title, body_json, has_snapshot, published_at
    from public.list_my_managed_share_gallery_page(null, null, null, 1)
  $$,
  $$values (null::text, null::jsonb, false, '2026-09-14T06:00:00Z'::timestamptz)$$,
  'A snapshotless share retains a management card with its management time'
);

select is(
  (
    select page.has_snapshot
    from public.list_my_managed_share_gallery_page('Beta', null, null, 13) as page
  ),
  true,
  'A published share is marked as previewable'
);

select is(
  (
    select array_agg(page.share_id order by page.published_at desc)
    from public.list_my_managed_share_gallery_page(
      null,
      '2026-09-14T02:00:00Z',
      '31000000-0000-4000-8000-000000000012',
      2
    ) as page
  ),
  array['31000000-0000-4000-8000-000000000011'::uuid],
  'The cursor returns only older snapshot content'
);

select is(
  (
    select array_agg(page.share_id order by page.published_at desc)
    from public.list_my_managed_share_gallery_page('ALPHA', null, null, 13) as page
  ),
  array[
    '31000000-0000-4000-8000-000000000013'::uuid,
    '31000000-0000-4000-8000-000000000011'::uuid
  ],
  'Title search is case insensitive and runs before pagination'
);

select is(
  (
    select page.general_scope
    from public.list_my_managed_share_gallery_page('Beta', null, null, 13) as page
  ),
  'link',
  'The page returns the current share scope with each snapshot'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('gallery_member');

select is(
  (
    select count(*)
    from public.list_my_managed_share_gallery_page(null, null, null, 13)
  ),
  0::bigint,
  'Non-managers cannot list gallery snapshots'
);

select * from finish();
rollback;
