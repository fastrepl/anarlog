begin;
select plan(8);

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

reset role;
update auth.users set raw_user_meta_data = raw_user_meta_data || '{"profile_avatar":{"url":"https://example.com/custom.jpg"}}'::jsonb
where id = tests.get_supabase_uid('roster_owner');
insert into public.workspace_memberships(workspace_id, user_id, role)
select workspace_id, tests.get_supabase_uid('roster_member'), 'member' from roster_state;
select tests.authenticate_as_hyprnote_pro('roster_member');
select is((select user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where role = 'owner'), 'https://example.com/custom.jpg', 'Teammate sees the shared custom photo');
reset role;
update auth.users set raw_user_meta_data = raw_user_meta_data || '{"profile_avatar":{"url":null}}'::jsonb
where id = tests.get_supabase_uid('roster_owner');
select tests.authenticate_as_hyprnote_pro('roster_member');
select is((select user_avatar_url from public.list_workspace_members_with_profiles((select workspace_id from roster_state)) where role = 'owner'), null::text, 'Removal suppresses the provider photo for teammates');
select lives_ok($$insert into storage.objects(bucket_id, name) values ('profile-avatars', tests.get_supabase_uid('roster_member')::text || '/test.jpg')$$, 'Owner can create their photo');
select throws_ok($$insert into storage.objects(bucket_id, name) values ('profile-avatars', tests.get_supabase_uid('roster_owner')::text || '/test.jpg')$$, '42501', 'new row violates row-level security policy for table "objects"', 'Cannot upload into another account');
select is((select count(*) from storage.objects where bucket_id = 'profile-avatars'), 1::bigint, 'Owner can list their photo');
select tests.authenticate_as_hyprnote_pro('roster_outsider');
select is((select count(*) from storage.objects where bucket_id = 'profile-avatars'), 0::bigint, 'Other accounts cannot list photos');
reset role;
select is(private.account_deletion_extension_prefix_empty(tests.get_supabase_uid('roster_member')), false, 'Deletion waits for avatar object cleanup');
select tests.authenticate_as_hyprnote_pro('roster_member');
set local storage.allow_delete_query = 'true';
select lives_ok($$delete from storage.objects where bucket_id = 'profile-avatars'$$, 'Owner can delete their photo');
select * from finish();
rollback;
