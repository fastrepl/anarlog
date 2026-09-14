begin;
select plan(11);

select tests.create_supabase_user('roster_owner', 'roster-owner@example.com');
select tests.create_supabase_user('roster_member', 'roster-member@example.com');
select tests.create_supabase_user('roster_peer', 'roster-peer@example.com');
select tests.create_supabase_user('roster_outsider', 'roster-outsider@example.com');

update auth.users set email_confirmed_at = now()
where id in (tests.get_supabase_uid('roster_owner'), tests.get_supabase_uid('roster_member'), tests.get_supabase_uid('roster_peer'), tests.get_supabase_uid('roster_outsider'));
update auth.users set raw_user_meta_data = raw_user_meta_data || '{"full_name":"  Roster Owner  ","avatar_url":"https://example.com/owner.png","secret":"must not be exposed"}'::jsonb
where id = tests.get_supabase_uid('roster_owner');

create temporary table roster_state (workspace_id uuid);
grant all on roster_state to authenticated;
select tests.authenticate_as_hyprnote_pro('roster_owner');
insert into roster_state select workspace_id from public.create_workspace('Roster');

select results_eq(
  $$select user_name, user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state))$$,
  $$values ('Roster Owner'::text, 'https://example.com/owner.png'::text)$$,
  'Owner can read trimmed profile name and avatar'
);
select is(
  (select count(*) from public.list_workspace_memberships((select workspace_id from roster_state))),
  1::bigint, 'Original roster RPC remains compatible'
);

reset role;
insert into public.workspace_memberships(workspace_id, user_id, role)
select roster_state.workspace_id, member.user_id, 'member'
from roster_state
cross join (values (tests.get_supabase_uid('roster_member')), (tests.get_supabase_uid('roster_peer'))) as member(user_id);
update auth.users set raw_user_meta_data = raw_user_meta_data || '{"name":"Fallback Name","picture":"https://example.com/fallback.png"}'::jsonb
where id = tests.get_supabase_uid('roster_member');
select tests.authenticate_as_hyprnote_pro('roster_owner');
select results_eq(
  $$select user_name, user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where user_id = tests.get_supabase_uid('roster_member')$$,
  $$values ('Fallback Name'::text, 'https://example.com/fallback.png'::text)$$,
  'Provider name and picture aliases are supported'
);
reset role;
update auth.users set raw_user_meta_data = raw_user_meta_data - 'name' - 'picture' where id = tests.get_supabase_uid('roster_member');
select tests.authenticate_as_hyprnote_pro('roster_owner');
select results_eq(
  $$select user_name, user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where user_id = tests.get_supabase_uid('roster_member')$$,
  $$values (null::text, null::text)$$,
  'Missing profiles remain null'
);

select tests.clear_authentication();
select tests.authenticate_as('roster_member');
select results_eq(
  $$select user_name, user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where role = 'owner'$$,
  $$values ('Roster Owner'::text, 'https://example.com/owner.png'::text)$$,
  'Members can read other members profiles without a paid plan'
);
select throws_ok(
  $$select * from public.list_workspace_invitations((select workspace_id from roster_state))$$,
  '42501', 'workspace invitation operation not permitted', 'Members cannot list invitations'
);
select throws_ok(
  $$select public.revoke_workspace_membership((select workspace_id from roster_state), tests.get_supabase_uid('roster_peer'))$$,
  '42501', 'workspace membership operation not permitted', 'Members cannot remove other members'
);
select tests.clear_authentication();
select tests.authenticate_as('roster_outsider');
select throws_ok(
  $$select * from public.list_workspace_members_with_profiles((select workspace_id from roster_state))$$,
  '42501', 'workspace membership operation not permitted', 'Outsiders cannot read profiles'
);
reset role;
update public.workspace_memberships set deleted_at = now() where user_id = tests.get_supabase_uid('roster_member');
select tests.authenticate_as_hyprnote_pro('roster_owner');
select is(
  (select count(*) from public.list_workspace_members_with_profiles((select workspace_id from roster_state))),
  2::bigint, 'Removed members are excluded'
);
select tests.clear_authentication();
select tests.authenticate_as('roster_peer');
select is(
  (select count(*) from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where user_id = tests.get_supabase_uid('roster_member')),
  0::bigint, 'Members do not see removed members'
);
select tests.clear_authentication();
select tests.authenticate_as('roster_member');
select throws_ok(
  $$select * from public.list_workspace_members_with_profiles((select workspace_id from roster_state))$$,
  '42501', 'workspace membership operation not permitted', 'Removed members cannot read profiles'
);
select * from finish();
rollback;
