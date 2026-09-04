begin;
select plan(21);

select tests.create_supabase_user(
  'resource_owner',
  'resource-owner@example.com'
);
select tests.create_supabase_user(
  'resource_guest',
  'resource-guest@example.com'
);
select tests.create_supabase_user(
  'resource_member',
  'resource-member@example.com'
);
select tests.create_supabase_user(
  'resource_outsider',
  'resource-outsider@example.com'
);

create temporary table shared_resource_test_state (
  name text primary key,
  value uuid not null
);

grant all on shared_resource_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('resource_owner'),
  tests.get_supabase_uid('resource_guest'),
  tests.get_supabase_uid('resource_member'),
  tests.get_supabase_uid('resource_outsider')
);

select tests.authenticate_as('resource_owner');

select ok(
  has_function_privilege(
    'authenticated',
    'public.upsert_shared_resource(text,text,text,jsonb,uuid)',
    'EXECUTE'
  )
    and has_function_privilege(
      'authenticated',
      'public.move_shared_resource(uuid,text,text,jsonb)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.upsert_shared_resource(text,text,text,jsonb,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.move_shared_resource(uuid,text,text,jsonb)',
      'EXECUTE'
    ),
  'Only authenticated callers can publish shared resources'
);

select ok(
  not has_table_privilege('authenticated', 'public.shared_resources', 'SELECT')
    and not has_table_privilege(
      'authenticated',
      'public.shared_resource_guests',
      'SELECT'
    ),
  'Shared resource tables are reachable only through authorized RPCs'
);

select throws_ok(
  $$
    select * from public.upsert_shared_resource(
      'folder',
      'Customers',
      'Customers',
      '{"version":1,"path":"Customers","notes":[]}'::jsonb,
      null
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'Free accounts cannot publish a personal shared folder'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('resource_owner');

insert into shared_resource_test_state (name, value)
select 'folder', share_id
from public.upsert_shared_resource(
  'folder',
  'Customers',
  'Customers',
  '{"version":1,"path":"Customers","notes":[]}'::jsonb,
  null
);

select results_eq(
  $$
    select resource_title, access_kind
    from public.list_shared_resources('folder')
  $$,
  $$ values ('Customers'::text, 'owner'::text) $$,
  'A Pro owner can publish and manage a folder share'
);

select results_eq(
  $$
    select invitee_email, was_created
    from public.grant_shared_resource_access(
      (select value from shared_resource_test_state where name = 'folder'),
      'RESOURCE-GUEST@example.com'
    )
  $$,
  $$ values ('resource-guest@example.com'::text, true) $$,
  'A Pro owner can grant one resource to a guest'
);

select results_eq(
  $$
    select share_id, source_id, resource_title
    from public.move_shared_resource(
      (select value from shared_resource_test_state where name = 'folder'),
      'Renamed Customers',
      'Renamed Customers',
      '{"version":1,"path":"Renamed Customers","notes":[]}'::jsonb
    )
  $$,
  $$
    values (
      (select value from shared_resource_test_state where name = 'folder'),
      'Renamed Customers'::text,
      'Renamed Customers'::text
    )
  $$,
  'Renaming a shared folder preserves its share identity and access list'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_guest');

select results_eq(
  $$
    select resource_title, access_kind
    from public.list_shared_resources('folder')
  $$,
  $$ values ('Renamed Customers'::text, 'guest'::text) $$,
  'The matching confirmed account can open its shared folder'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_outsider');

select is_empty(
  $$ select * from public.list_shared_resources('folder') $$,
  'An unrelated account cannot discover the folder'
);

select throws_ok(
  $$
    select * from public.move_shared_resource(
      (select value from shared_resource_test_state where name = 'folder'),
      'Stolen',
      'Stolen',
      '{"version":1,"path":"Stolen","notes":[]}'::jsonb
    )
  $$,
  '42501',
  'shared resource operation not permitted',
  'An unrelated account cannot rename another owner shared resource'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('resource_owner');

insert into shared_resource_test_state (name, value)
select 'automation', share_id
from public.upsert_shared_resource(
  'automation',
  'weekly-recap',
  'Weekly recap',
  '{"version":1,"workflow":{"id":"weekly-recap"}}'::jsonb,
  null
);

select throws_ok(
  $$
    select * from public.grant_shared_resource_access(
      (select value from shared_resource_test_state where name = 'automation'),
      'resource-guest@example.com'
    )
  $$,
  '42501',
  'multi-resource guest requires Team membership',
  'A second resource for one guest is routed to Team membership'
);

select results_eq(
  $$
    select invitee_email
    from public.list_shared_resource_guests(
      (select value from shared_resource_test_state where name = 'folder')
    )
  $$,
  array['resource-guest@example.com'::text],
  'The owner can audit resource-scoped guest access'
);

select lives_ok(
  $$
    select * from public.revoke_shared_resource_access(
      (
        select guest_id
        from public.list_shared_resource_guests(
          (select value from shared_resource_test_state where name = 'folder')
        )
      )
    )
  $$,
  'The owner can revoke folder access'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_guest');

select is_empty(
  $$ select * from public.list_shared_resources('folder') $$,
  'Revoked guest access disappears immediately'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_owner');

insert into shared_resource_test_state (name, value)
select 'team_workspace', workspace_id
from public.create_workspace('Resource Team');

select tests.clear_authentication();
reset role;

select tests.enable_workspace_plan(
  (select value from shared_resource_test_state where name = 'team_workspace'),
  'team'
);

select tests.authenticate_as_service_role();

insert into public.workspace_memberships (workspace_id, user_id, role)
values (
  (select value from shared_resource_test_state where name = 'team_workspace'),
  tests.get_supabase_uid('resource_member'),
  'member'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_owner');

select ok(
  exists (
    select 1
    from public.get_workspace_access(
      (select value from shared_resource_test_state where name = 'team_workspace')
    )
    where 'team.shared_resources' = any (capabilities)
  ),
  'The Team plan exposes the shared resource capability'
);

insert into shared_resource_test_state (name, value)
select 'team_template', share_id
from public.upsert_shared_resource(
  'template',
  'team-standup',
  'Team standup',
  '{"version":1,"template":{"title":"Team standup","sections":[]}}'::jsonb,
  (select value from shared_resource_test_state where name = 'team_workspace')
);

select results_eq(
  $$
    select workspace_name, access_kind
    from public.list_shared_resources('template')
  $$,
  $$ values ('Resource Team'::text, 'owner'::text) $$,
  'The owner can publish a template to the Team library'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_member');

select results_eq(
  $$
    select resource_title, workspace_name, access_kind
    from public.list_shared_resources('template')
  $$,
  $$ values ('Team standup'::text, 'Resource Team'::text, 'team'::text) $$,
  'Every Team member can discover a workspace template'
);

select throws_ok(
  $$
    select * from public.delete_shared_resource(
      (select value from shared_resource_test_state where name = 'team_template')
    )
  $$,
  '42501',
  'shared resource operation not permitted',
  'A Team member cannot delete another member shared resource'
);

select tests.clear_authentication();
reset role;

delete from stripe.active_entitlements
where customer = (
  select stripe_customer_id
  from public.workspaces
  where id = (
    select value
    from shared_resource_test_state
    where name = 'team_workspace'
  )
);

select tests.clear_authentication();
select tests.authenticate_as('resource_member');

select is_empty(
  $$ select * from public.list_shared_resources('template') $$,
  'Team library access ends when the workspace plan ends'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_outsider');

select is_empty(
  $$ select * from public.list_shared_resources('template') $$,
  'Accounts outside the Team cannot discover its resources'
);

select tests.clear_authentication();
select tests.authenticate_as('resource_owner');

select lives_ok(
  $$
    select * from public.delete_shared_resource(
      (select value from shared_resource_test_state where name = 'team_template')
    )
  $$,
  'The resource owner can stop sharing with the Team'
);

select is_empty(
  $$ select * from public.list_shared_resources('template') $$,
  'Deleted resources disappear from the owner library'
);

select * from finish();
rollback;
