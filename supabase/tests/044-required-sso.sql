begin;
select plan(11);

select tests.create_supabase_user('sso_owner', 'sso-owner@gmail.test');
select tests.create_supabase_user('sso_member', 'sso-member@acme.test');

create temporary table required_sso_test_state (
  name text primary key,
  workspace_id uuid
);

grant all on required_sso_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('sso_owner'),
  tests.get_supabase_uid('sso_member')
);

select tests.authenticate_as_hyprnote_pro('sso_owner');

select lives_ok(
  $$
    insert into required_sso_test_state (name, workspace_id)
    select 'hq', workspace_id from public.create_workspace('SSO HQ')
  $$,
  'The owner creates a shared workspace'
);

select tests.enable_workspace_plan(
  (select workspace_id from required_sso_test_state where name = 'hq'),
  'enterprise'
);

select throws_ok(
  $$
    select * from public.set_workspace_policy(
      (select workspace_id from required_sso_test_state where name = 'hq'),
      array['restricted', 'workspace', 'link', 'public']::text[],
      'restricted',
      null,
      true,
      true,
      true
    )
  $$,
  '22023',
  'claim an email domain before requiring SSO',
  'Require SSO cannot be enabled before a domain is claimed'
);

select lives_ok(
  $$
    select * from public.claim_workspace_domain(
      (select workspace_id from required_sso_test_state where name = 'hq'),
      'acme.test'
    )
  $$,
  'The owner claims the company email domain'
);

select lives_ok(
  $$
    select * from public.set_workspace_policy(
      (select workspace_id from required_sso_test_state where name = 'hq'),
      array['restricted', 'workspace', 'link', 'public']::text[],
      'restricted',
      null,
      true,
      true,
      true
    )
  $$,
  'Require SSO can be enabled after a domain is claimed'
);

select tests.clear_authentication();
reset role;

select lives_ok(
  $$
    insert into public.workspace_memberships (workspace_id, user_id, role)
    values (
      (select workspace_id from required_sso_test_state where name = 'hq'),
      tests.get_supabase_uid('sso_member'),
      'member'
    )
  $$,
  'A company-domain user can be added as a member'
);

select is(
  public.email_requires_sso('sso-member@acme.test'),
  true,
  'Claimed domains with Require SSO report SSO is required'
);

select is(
  public.email_requires_sso('sso-owner@gmail.test'),
  false,
  'Personal emails outside the claimed domain do not require SSO'
);

select throws_ok(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('sso_member')::text,
        'claims', jsonb_build_object(
          'email', 'sso-member@acme.test',
          'app_metadata', jsonb_build_object(
            'provider', 'google',
            'providers', jsonb_build_array('google')
          )
        )
      )
    )
  $$,
  '42501',
  'this organization requires SSO',
  'Google sign-in is rejected for an SSO-required company domain'
);

select throws_ok(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('sso_member')::text,
        'claims', jsonb_build_object(
          'email', 'sso-member@acme.test',
          'app_metadata', jsonb_build_object(
            'provider', 'google',
            'providers', jsonb_build_array(
              'sso:11111111-1111-1111-1111-111111111111',
              'google'
            )
          )
        )
      )
    )
  $$,
  '42501',
  'this organization requires SSO',
  'A linked SSO identity does not allow Google sign-in on a required domain'
);

select lives_ok(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('sso_member')::text,
        'claims', jsonb_build_object(
          'email', 'sso-member@acme.test',
          'app_metadata', jsonb_build_object(
            'provider', 'sso:11111111-1111-1111-1111-111111111111',
            'providers', jsonb_build_array(
              'sso:11111111-1111-1111-1111-111111111111'
            )
          ),
          'amr', jsonb_build_array(
            jsonb_build_object('method', 'sso/saml', 'timestamp', 1)
          )
        )
      )
    )
  $$,
  'SSO sign-in is allowed for an SSO-required company domain'
);

select lives_ok(
  $$
    select public.custom_access_token_hook(
      jsonb_build_object(
        'user_id', tests.get_supabase_uid('sso_owner')::text,
        'claims', jsonb_build_object(
          'email', 'sso-owner@gmail.test',
          'app_metadata', jsonb_build_object(
            'provider', 'google',
            'providers', jsonb_build_array('google')
          )
        )
      )
    )
  $$,
  'Personal Google accounts are not blocked by another workspace Require SSO policy'
);

select * from finish();
rollback;
