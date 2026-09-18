begin;
select plan(10);

select tests.create_supabase_user(
  'creator_authority_owner',
  'creator-authority-owner@example.com'
);
select tests.create_supabase_user(
  'creator_authority_member',
  'creator-authority-member@example.com'
);
select tests.create_supabase_user(
  'creator_authority_other',
  'creator-authority-other@example.com'
);
select tests.create_supabase_user(
  'creator_authority_admin',
  'creator-authority-admin@example.com'
);

create temporary table creator_authority_test_state (
  workspace_id uuid,
  member_share_id uuid,
  owner_share_id uuid
);
grant all on creator_authority_test_state to anon, authenticated, service_role;

insert into creator_authority_test_state (workspace_id)
values (gen_random_uuid());

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('creator_authority_owner'),
  tests.get_supabase_uid('creator_authority_member'),
  tests.get_supabase_uid('creator_authority_other'),
  tests.get_supabase_uid('creator_authority_admin')
);

select tests.authenticate_as_service_role();
select tests.clear_authentication();
reset role;

insert into stripe.customers (id)
values ('cus_creator_authority')
on conflict (id) do nothing;

insert into stripe.subscriptions (id, customer, status)
values (
  'sub_creator_authority',
  'cus_creator_authority',
  'active'::stripe.subscription_status
)
on conflict (id) do nothing;

insert into stripe.active_entitlements (id, customer, lookup_key)
values (
  'ent_creator_authority_team',
  'cus_creator_authority',
  'hyprnote_team'
)
on conflict (customer, lookup_key) do nothing;

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('creator_authority_owner'),
  'shared',
  'Creator authority workspace'
from creator_authority_test_state;

update public.workspaces
set stripe_customer_id = 'cus_creator_authority'
where id = (select workspace_id from creator_authority_test_state);

insert into public.workspace_memberships (workspace_id, user_id, role)
select workspace_id, tests.get_supabase_uid('creator_authority_owner'), 'owner'
from creator_authority_test_state
union all
select workspace_id, tests.get_supabase_uid('creator_authority_member'), 'member'
from creator_authority_test_state
union all
select workspace_id, tests.get_supabase_uid('creator_authority_other'), 'member'
from creator_authority_test_state
union all
select workspace_id, tests.get_supabase_uid('creator_authority_admin'), 'admin'
from creator_authority_test_state;

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_member');

select lives_ok(
  $$
    update creator_authority_test_state
    set member_share_id = (
      select share_id
      from public.create_session_share(workspace_id, 'member-created-session')
    )
  $$,
  'A member can create a share in a shared workspace'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select share.created_by_user_id = tests.get_supabase_uid(
      'creator_authority_member'
    )
    from public.session_shares as share
    where share.id = (
      select member_share_id from creator_authority_test_state
    )
  ),
  'A member-created share records the member as creator'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_member');

select lives_ok(
  $$
    select *
    from public.get_session_share_management(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  'A member can manage their own share'
);

select results_eq(
  $$
    select capability, manage_access
    from public.resolve_my_session_access(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  $$values ('editor'::text, true)$$,
  'A member-created share resolves with management access for its creator'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_other');

select throws_ok(
  $$
    select *
    from public.get_session_share_management(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Another plain member cannot manage a member-created share'
);

select throws_ok(
  $$
    select *
    from public.delete_session_share(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Another plain member cannot delete a member-created share'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_owner');

select lives_ok(
  $$
    select *
    from public.get_session_share_management(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  'A workspace owner can manage a member-created share'
);

select lives_ok(
  $$
    update creator_authority_test_state
    set owner_share_id = (
      select share_id
      from public.create_session_share(
        workspace_id,
        'owner-created-session'
      )
    )
  $$,
  'A workspace owner can create an owner-created share'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_member');

select throws_ok(
  $$
    select *
    from public.get_session_share_management(
      (select owner_share_id from creator_authority_test_state)
    )
  $$,
  '42501',
  'session access operation not permitted',
  'A plain member cannot manage an owner-created share'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('creator_authority_admin');

select lives_ok(
  $$
    select *
    from public.delete_session_share(
      (select member_share_id from creator_authority_test_state)
    )
  $$,
  'A workspace admin can delete a member-created share'
);

select * from finish();
rollback;
