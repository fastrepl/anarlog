begin;
select plan(9);

select tests.create_supabase_user(
  'slug_availability_owner',
  'slug-availability-owner@example.com'
);
select tests.create_supabase_user(
  'slug_availability_outsider',
  'slug-availability-outsider@example.com'
);

create temporary table workspace_slug_availability_test_state (
  name text primary key,
  workspace_id uuid not null
);

grant all on workspace_slug_availability_test_state
to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('slug_availability_owner'),
  tests.get_supabase_uid('slug_availability_outsider')
);

select tests.authenticate_as('slug_availability_owner');

insert into workspace_slug_availability_test_state (name, workspace_id)
select 'candidate', workspace_id
from public.create_workspace('Candidate workspace');

insert into workspace_slug_availability_test_state (name, workspace_id)
select 'claimed', workspace_id
from public.create_workspace('Claimed workspace');

insert into workspace_slug_availability_test_state (name, workspace_id)
select 'team', workspace_id
from public.create_workspace('Team workspace');

select tests.clear_authentication();
reset role;

select tests.enable_workspace_plan(
  (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
  'enterprise'
);
select tests.enable_workspace_plan(
  (select workspace_id from workspace_slug_availability_test_state where name = 'claimed'),
  'enterprise'
);
select tests.enable_workspace_plan(
  (select workspace_id from workspace_slug_availability_test_state where name = 'team'),
  'team'
);

select tests.authenticate_as('slug_availability_owner');

select ok(
  has_function_privilege(
    'authenticated',
    'public.check_workspace_share_slug_availability(uuid,text)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'anon',
      'public.check_workspace_share_slug_availability(uuid,text)',
      'EXECUTE'
    ),
  'Only authenticated callers can check workspace subdomain availability'
);

select is(
  public.check_workspace_share_slug_availability(
    (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
    'fresh-domain'
  ),
  'available',
  'An unclaimed workspace subdomain is available'
);

select is(
  public.check_workspace_share_slug_availability(
    (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
    'models'
  ),
  'invalid',
  'A reserved platform subdomain is invalid'
);

select is(
  public.check_workspace_share_slug_availability(
    (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
    'no'
  ),
  'invalid',
  'A malformed workspace subdomain is invalid'
);

select lives_ok(
  $$
    select * from public.set_workspace_share_slug(
      (select workspace_id from workspace_slug_availability_test_state where name = 'claimed'),
      'claimed-domain'
    )
  $$,
  'An Enterprise manager can claim the comparison subdomain'
);

select is(
  public.check_workspace_share_slug_availability(
    (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
    'claimed-domain'
  ),
  'taken',
  'A subdomain claimed by another workspace is taken'
);

select is(
  public.check_workspace_share_slug_availability(
    (select workspace_id from workspace_slug_availability_test_state where name = 'claimed'),
    'claimed-domain'
  ),
  'available',
  'A workspace can keep its current subdomain'
);

select throws_ok(
  $$
    select public.check_workspace_share_slug_availability(
      (select workspace_id from workspace_slug_availability_test_state where name = 'team'),
      'team-domain'
    )
  $$,
  '42501',
  'workspace capability required: team.custom_subdomain',
  'Team cannot check Enterprise workspace subdomains'
);

select tests.clear_authentication();
select tests.authenticate_as('slug_availability_outsider');

select throws_ok(
  $$
    select public.check_workspace_share_slug_availability(
      (select workspace_id from workspace_slug_availability_test_state where name = 'candidate'),
      'fresh-domain'
    )
  $$,
  '42501',
  'workspace subdomain operation not permitted',
  'An unrelated account cannot check an Enterprise workspace subdomain'
);

select * from finish();
rollback;
