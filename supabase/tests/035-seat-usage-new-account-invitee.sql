begin;
select plan(5);

select tests.create_supabase_user('cap_owner', 'cap-owner@example.com');

create temporary table cap_test_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text
);

grant all on cap_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('cap_owner');

select tests.authenticate_as_hyprnote_pro('cap_owner');

insert into cap_test_state (name, workspace_id)
select 'hq', workspace_id from public.create_workspace('Cap HQ');

select tests.clear_authentication();
reset role;

-- Two seats: the owner plus exactly one teammate.
update public.workspaces
set seat_limit = 2
where id = (select workspace_id from cap_test_state where name = 'hq');

select tests.authenticate_as_hyprnote_pro('cap_owner');

select lives_ok(
  $$
    insert into cap_test_state (name, invitation_id, invite_token)
    select 'newcomer', invitation_id, invite_token
    from public.create_workspace_invitation(
      (select workspace_id from cap_test_state where name = 'hq'),
      'newcomer@example.com'
    )
  $$,
  'The owner can invite someone who has no account yet'
);

select results_eq(
  $$
    select used_seats
    from public.get_workspace_seat_usage(
      (select workspace_id from cap_test_state where name = 'hq')
    )
  $$,
  array[2],
  'The pending invitation holds the second seat'
);

select tests.clear_authentication();
reset role;

-- The invitee only signs up now, so the invitation still has a NULL
-- invitee_user_id when the membership row is created.
select tests.create_supabase_user('cap_newcomer', 'newcomer@example.com');

update auth.users
set email_confirmed_at = now()
where id = tests.get_supabase_uid('cap_newcomer');

select tests.authenticate_as('cap_newcomer');

select lives_ok(
  $$
    select * from public.accept_workspace_invitation(
      (select invitation_id from cap_test_state where name = 'newcomer'),
      (select invite_token from cap_test_state where name = 'newcomer')
    )
  $$,
  'Someone invited before they had an account can still take their seat'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('cap_owner');

select results_eq(
  $$
    select used_seats
    from public.get_workspace_seat_usage(
      (select workspace_id from cap_test_state where name = 'hq')
    )
  $$,
  array[2],
  'Seating the invitee does not double-count the invitation it consumed'
);

select throws_ok(
  $$
    select * from public.create_workspace_invitation(
      (select workspace_id from cap_test_state where name = 'hq'),
      'one-too-many@example.com'
    )
  $$,
  '22023',
  'workspace seat limit reached',
  'The seat cap still holds once every seat is taken'
);

select * from finish();
rollback;
