begin;
select plan(3);

select tests.create_supabase_user(
  'short_link_owner',
  'short-link-owner@example.com'
);

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('short_link_owner');

create temporary table short_link_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  link_id uuid
);

grant all on short_link_test_state to anon, authenticated, service_role;

insert into short_link_test_state (name, workspace_id)
values ('workspace', gen_random_uuid());

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('short_link_owner'),
  'shared',
  'Short link workspace'
from short_link_test_state
where name = 'workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('short_link_owner'),
  'owner'
from short_link_test_state
where name = 'workspace';

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('short_link_owner');

insert into short_link_test_state (name, share_id)
select 'share', share_id
from public.create_session_share(
  (select workspace_id from short_link_test_state where name = 'workspace'),
  'short-link-session'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select *
from public.publish_session_share_snapshot_with_preview_cas(
  (select share_id from short_link_test_state where name = 'share'),
  tests.get_supabase_uid('short_link_owner'),
  0,
  '30000000-0000-4000-8000-000000000001',
  'Short link preview',
  '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Private meeting summary."}]}]}'::jsonb,
  ARRAY[]::uuid[],
  true,
  ARRAY['John Jeong'],
  '2026-08-12T01:30:00Z'::timestamptz
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('short_link_owner');

insert into short_link_test_state (name, share_id, link_id)
select 'link', share_id, link_id
from public.enable_session_share_link(
  (select share_id from short_link_test_state where name = 'share')
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select is(
  (
    select preview.share_id
    from public.gateway_read_session_share_link_preview_by_id(
      (select link_id from short_link_test_state where name = 'link')
    ) as preview
  ),
  (select share_id from short_link_test_state where name = 'share'),
  'An active link ID resolves its share without exposing the bearer token'
);

select is(
  (
    select preview.title
    from public.gateway_read_session_share_link_preview_by_id(
      (select link_id from short_link_test_state where name = 'link')
    ) as preview
  ),
  'Short link preview',
  'An active link ID exposes only preview metadata'
);

update public.session_share_links
set revoked_at = now()
where id = (select link_id from short_link_test_state where name = 'link');

select is_empty(
  $$
    select *
    from public.gateway_read_session_share_link_preview_by_id(
      (select link_id from short_link_test_state where name = 'link')
    )
  $$,
  'Revoked link IDs no longer resolve preview metadata'
);

select * from finish();
rollback;
