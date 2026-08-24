begin;
select plan(10);

select tests.create_supabase_user('subdomain_owner', 'subdomain-owner@example.com');
select tests.create_supabase_user('subdomain_other', 'subdomain-other@example.com');
select tests.create_supabase_user('subdomain_member', 'subdomain-member@example.com');

create temporary table workspace_subdomain_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid
);

grant all on workspace_subdomain_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('subdomain_owner'),
  tests.get_supabase_uid('subdomain_other'),
  tests.get_supabase_uid('subdomain_member')
);

select tests.authenticate_as_hyprnote_pro('subdomain_owner');

insert into workspace_subdomain_test_state (name, workspace_id)
select 'owner', workspace_id
from public.create_workspace('Fastrepl');

select lives_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
      '  Fastrepl-HQ  '
    )
  $$,
  'A workspace manager can claim a normalized share subdomain'
);

select results_eq(
  $$
    select share_slug
    from public.workspaces
    where id = (
      select workspace_id from workspace_subdomain_test_state where name = 'owner'
    )
  $$,
  $$values ('fastrepl-hq'::text)$$,
  'The normalized slug is stored on the workspace'
);

select results_eq(
  $$
    select workspace_share_slug, share_base_url
    from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
      'fastrepl'
    )
  $$,
  $$values ('fastrepl'::text, 'https://fastrepl.anarlog.so'::text)$$,
  'The setter returns the canonical enterprise sharing origin'
);

select throws_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
      'api'
    )
  $$,
  '22023',
  'invalid workspace subdomain',
  'Platform hostnames are reserved'
);

select throws_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
      '-invalid-'
    )
  $$,
  '22023',
  'invalid workspace subdomain',
  'Invalid DNS labels are rejected'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

insert into public.workspace_memberships (workspace_id, user_id, role)
values (
  (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
  tests.get_supabase_uid('subdomain_member'),
  'member'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('subdomain_member');

select results_eq(
  $$
    select share_slug
    from public.workspaces
    where id = (
      select workspace_id from workspace_subdomain_test_state where name = 'owner'
    )
  $$,
  $$values ('fastrepl'::text)$$,
  'Workspace members can read the sharing slug used by their clients'
);

select throws_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
      'member-choice'
    )
  $$,
  '42501',
  'workspace subdomain operation not permitted',
  'Plain members cannot change the workspace subdomain'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('subdomain_other');

insert into workspace_subdomain_test_state (name, workspace_id)
select 'other', workspace_id
from public.create_workspace('Other Company');

select throws_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_subdomain_test_state where name = 'other'),
      'FASTREPL'
    )
  $$,
  '23505',
  'workspace subdomain is already taken',
  'Workspace subdomains are globally unique after normalization'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('subdomain_owner');

insert into workspace_subdomain_test_state (name, share_id)
select 'share', share_id
from public.create_session_share(
  (select workspace_id from workspace_subdomain_test_state where name = 'owner'),
  'session-subdomain-1'
);

select results_eq(
  $$
    select workspace_share_slug
    from public.get_session_share_workspace_slug(
      (select share_id from workspace_subdomain_test_state where name = 'share')
    )
  $$,
  $$values ('fastrepl'::text)$$,
  'Share managers can resolve the workspace subdomain for link generation'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('subdomain_other');

select throws_ok(
  $$
    select * from public.get_session_share_workspace_slug(
      (select share_id from workspace_subdomain_test_state where name = 'share')
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Unrelated accounts cannot resolve a share workspace subdomain'
);

select * from finish();
rollback;
