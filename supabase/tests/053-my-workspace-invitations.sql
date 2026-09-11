begin;
select no_plan();

select tests.create_supabase_user('myinv_owner', 'myinv-owner@example.com');
select tests.create_supabase_user('myinv_recipient', 'myinv-recipient@example.com');
select tests.create_supabase_user('myinv_recipient2', 'myinv-recipient2@example.com');
select tests.create_supabase_user('myinv_outsider', 'myinv-outsider@example.com');

update auth.users set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('myinv_owner'),
  tests.get_supabase_uid('myinv_recipient'),
  tests.get_supabase_uid('myinv_recipient2'),
  tests.get_supabase_uid('myinv_outsider')
);

create temporary table my_invitations_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid
);
grant all on my_invitations_state to authenticated;

select tests.authenticate_as('myinv_owner');
insert into my_invitations_state (name, workspace_id)
select 'main', workspace_id from public.create_workspace('My invitations team');
reset role;
select tests.enable_workspace_plan(
  (select workspace_id from my_invitations_state where name = 'main'),
  'team',
  3
);
select tests.authenticate_as('myinv_owner');

insert into my_invitations_state (name, invitation_id)
select 'inv_recipient', invitation_id
from public.create_workspace_invitation(
  (select workspace_id from my_invitations_state where name = 'main'),
  'myinv-recipient@example.com'
);
insert into my_invitations_state (name, invitation_id)
select 'inv_recipient2', invitation_id
from public.create_workspace_invitation(
  (select workspace_id from my_invitations_state where name = 'main'),
  'myinv-recipient2@example.com'
);

select ok(
  has_function_privilege(
    'authenticated', 'public.list_my_workspace_invitations()', 'EXECUTE'
  )
  and has_function_privilege(
    'authenticated', 'public.accept_my_workspace_invitation(uuid)', 'EXECUTE'
  )
  and has_function_privilege(
    'authenticated', 'public.decline_my_workspace_invitation(uuid)', 'EXECUTE'
  )
  and not has_function_privilege(
    'anon', 'public.list_my_workspace_invitations()', 'EXECUTE'
  )
  and not has_function_privilege(
    'anon', 'public.accept_my_workspace_invitation(uuid)', 'EXECUTE'
  )
  and not has_function_privilege(
    'anon', 'public.decline_my_workspace_invitation(uuid)', 'EXECUTE'
  ),
  'Only authenticated clients can execute the invitee inbox RPCs'
);

select tests.clear_authentication();
select tests.authenticate_as('myinv_recipient');

select results_eq(
  $$
    select workspace_name, invited_by_email
    from public.list_my_workspace_invitations()
  $$,
  $$ values ('My invitations team'::text, 'myinv-owner@example.com'::text) $$,
  'The invitee sees the pending invitation with workspace and inviter'
);

select tests.clear_authentication();
select tests.authenticate_as('myinv_outsider');

select results_eq(
  $$ select count(*) from public.list_my_workspace_invitations() $$,
  array[0::bigint],
  'An account with no invitations sees an empty inbox'
);

select throws_ok(
  $$
    select *
    from public.accept_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient')
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'An outsider cannot accept an invitation addressed to someone else'
);

select throws_ok(
  $$
    select public.decline_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient')
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'An outsider cannot decline an invitation addressed to someone else'
);

select tests.clear_authentication();
select tests.authenticate_as('myinv_recipient');

select lives_ok(
  $$
    select *
    from public.accept_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient')
    )
  $$,
  'The invitee can accept their own invitation without the emailed token'
);

select results_eq(
  $$
    select role
    from public.workspace_memberships
    where workspace_id = (
      select workspace_id from my_invitations_state where name = 'main'
    )
      and user_id = auth.uid()
      and deleted_at is null
  $$,
  array['member'::text],
  'Acceptance grants the member role'
);

reset role;
select ok(
  (
    select accepted_at is not null
    from public.workspace_invitations
    where id = (
      select invitation_id from my_invitations_state where name = 'inv_recipient'
    )
  ),
  'The invitation is marked accepted'
);
select tests.authenticate_as('myinv_recipient');

select results_eq(
  $$ select count(*) from public.list_my_workspace_invitations() $$,
  array[0::bigint],
  'An accepted invitation leaves the inbox'
);

select lives_ok(
  $$
    select *
    from public.accept_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient')
    )
  $$,
  'Accepting the same invitation twice is idempotent'
);

select results_eq(
  $$
    select workspace_id
    from public.accept_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient')
    )
  $$,
  $$
    select workspace_id
    from my_invitations_state
    where name = 'main'
  $$,
  'Repeating acceptance returns the same workspace'
);

select tests.clear_authentication();
select tests.authenticate_as('myinv_recipient2');

select lives_ok(
  $$
    select public.decline_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient2')
    )
  $$,
  'The invitee can decline their own invitation'
);

reset role;
select ok(
  (
    select revoked_at is not null
      and revoked_by_user_id = tests.get_supabase_uid('myinv_recipient2')
    from public.workspace_invitations
    where id = (
      select invitation_id from my_invitations_state where name = 'inv_recipient2'
    )
  ),
  'Declining marks the invitation revoked by the invitee'
);
select tests.authenticate_as('myinv_recipient2');

select results_eq(
  $$ select count(*) from public.list_my_workspace_invitations() $$,
  array[0::bigint],
  'A declined invitation leaves the inbox'
);

select throws_ok(
  $$
    select *
    from public.accept_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient2')
    )
  $$,
  '22023',
  'workspace invitation is invalid or unavailable',
  'A declined invitation cannot be accepted afterwards'
);

select lives_ok(
  $$
    select public.decline_my_workspace_invitation(
      (select invitation_id from my_invitations_state where name = 'inv_recipient2')
    )
  $$,
  'Declining the same invitation twice is a no-op'
);

select * from finish();
rollback;
