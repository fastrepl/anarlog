begin;
select plan(10);

select tests.create_supabase_user('logo_owner', 'logo-owner@example.com');
select tests.create_supabase_user('logo_member', 'logo-member@example.com');
select tests.create_supabase_user('logo_other', 'logo-other@example.com');

create temporary table workspace_logo_test_state (
  name text primary key,
  workspace_id uuid
);

grant all on workspace_logo_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('logo_owner'),
  tests.get_supabase_uid('logo_member'),
  tests.get_supabase_uid('logo_other')
);

select ok(
  has_function_privilege(
    'authenticated',
    'public.set_workspace_logo(uuid,text)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'anon',
      'public.set_workspace_logo(uuid,text)',
      'EXECUTE'
    ),
  'Only authenticated clients can execute the workspace logo RPC wrapper'
);

select tests.authenticate_as_hyprnote_pro('logo_owner');

insert into workspace_logo_test_state (name, workspace_id)
select 'owner', workspace_id
from public.create_workspace('Fastrepl');

select tests.enable_workspace_plan(
  (select workspace_id from workspace_logo_test_state where name = 'owner')
);

select lives_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      'data:image/jpeg;base64,/9j/4AAQ'
    )
  $$,
  'A workspace manager can set a JPEG logo'
);

select results_eq(
  $$
    select logo_data
    from public.workspaces
    where id = (
      select workspace_id from workspace_logo_test_state where name = 'owner'
    )
  $$,
  $$values ('data:image/jpeg;base64,/9j/4AAQ'::text)$$,
  'The logo is stored on the workspace'
);

select throws_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      'data:image/png;base64,AAAA'
    )
  $$,
  '22023',
  'invalid workspace logo',
  'Non-JPEG data URLs are rejected'
);

select throws_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      'javascript:alert(1)'
    )
  $$,
  '22023',
  'invalid workspace logo',
  'Non-image payloads are rejected'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

insert into public.workspace_memberships (workspace_id, user_id, role)
values (
  (select workspace_id from workspace_logo_test_state where name = 'owner'),
  tests.get_supabase_uid('logo_member'),
  'member'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('logo_member');

select results_eq(
  $$
    select logo_data
    from public.workspaces
    where id = (
      select workspace_id from workspace_logo_test_state where name = 'owner'
    )
  $$,
  $$values ('data:image/jpeg;base64,/9j/4AAQ'::text)$$,
  'Workspace members can read the logo used by their clients'
);

select throws_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      'data:image/jpeg;base64,/9j/4AAQ'
    )
  $$,
  '42501',
  'workspace logo operation not permitted',
  'Plain members cannot change the workspace logo'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('logo_other');

insert into workspace_logo_test_state (name, workspace_id)
select 'other', workspace_id
from public.create_workspace('Other Company');

select tests.enable_workspace_plan(
  (select workspace_id from workspace_logo_test_state where name = 'other')
);

select throws_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      'data:image/jpeg;base64,/9j/4AAQ'
    )
  $$,
  '42501',
  'workspace logo operation not permitted',
  'Unrelated accounts cannot change another workspace logo'
);

select tests.clear_authentication();
select tests.authenticate_as('logo_owner');

select lives_ok(
  $$
    select * from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      NULL
    )
  $$,
  'A free owner can manage a workspace paid by Team'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('logo_owner');

select results_eq(
  $$
    select workspace_logo_data
    from public.set_workspace_logo(
      (select workspace_id from workspace_logo_test_state where name = 'owner'),
      NULL
    )
  $$,
  $$values (NULL::text)$$,
  'A workspace manager can clear the logo'
);

select * from finish();
rollback;
