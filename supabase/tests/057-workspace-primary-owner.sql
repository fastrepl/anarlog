begin;
select plan(20);

select tests.create_supabase_user('primary', 'primary@example.com');
select tests.create_supabase_user('coowner', 'coowner@example.com');
select tests.create_supabase_user('peerowner', 'peerowner@example.com');
select tests.create_supabase_user('roleadmin', 'roleadmin@example.com');
update auth.users set email_confirmed_at = now()
where id in (tests.get_supabase_uid('primary'), tests.get_supabase_uid('coowner'), tests.get_supabase_uid('peerowner'), tests.get_supabase_uid('roleadmin'));
create temporary table ownership_state (workspace_id uuid);
grant all on ownership_state to authenticated;
select tests.authenticate_as_hyprnote_pro('primary');
insert into ownership_state select workspace_id from public.create_workspace('Owners');
select tests.enable_workspace_plan((select workspace_id from ownership_state));
reset role;
insert into public.workspace_memberships(workspace_id, user_id, role)
select workspace_id, tests.get_supabase_uid('coowner'), 'member' from ownership_state
union all select workspace_id, tests.get_supabase_uid('peerowner'), 'member' from ownership_state
union all select workspace_id, tests.get_supabase_uid('roleadmin'), 'admin' from ownership_state;
select tests.authenticate_as_hyprnote_pro('primary');
select lives_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('coowner'), 'owner')$$, 'Primary can appoint another owner');
select is((select owner_user_id from public.workspaces where id = (select workspace_id from ownership_state)), tests.get_supabase_uid('primary'), 'Appointing owners does not change the primary');
select throws_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('primary'), 'member')$$, '42501', 'workspace membership operation not permitted', 'Primary cannot demote themselves even with another owner');
select throws_ok($$select public.leave_workspace((select workspace_id from ownership_state))$$, '22023', 'owner must transfer ownership before leaving', 'Primary cannot leave even with another owner');

select tests.authenticate_as_hyprnote_pro('coowner');
select lives_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('peerowner'), 'owner')$$, 'Ordinary owners can appoint owners');
select throws_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('primary'), 'admin')$$, '42501', 'workspace membership operation not permitted', 'Ordinary owners cannot demote the primary');
select throws_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('peerowner'), 'admin')$$, '42501', 'workspace membership operation not permitted', 'Ordinary owners cannot strip a peer owner');
select throws_ok($$select public.revoke_workspace_membership((select workspace_id from ownership_state), tests.get_supabase_uid('primary'))$$, '42501', 'workspace membership operation not permitted', 'Primary cannot be removed');
select throws_ok($$select public.transfer_workspace_ownership((select workspace_id from ownership_state), tests.get_supabase_uid('peerowner'))$$, '42501', 'workspace ownership operation not permitted', 'Only primary may request a transfer');
select throws_ok($$select public.delete_workspace((select workspace_id from ownership_state))$$, '42501', 'workspace operation not permitted', 'Only primary may delete the workspace');
select lives_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('coowner'), 'member')$$, 'Ordinary owner can demote themselves');

select tests.authenticate_as_hyprnote_pro('roleadmin');
select throws_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('coowner'), 'owner')$$, '42501', 'workspace membership operation not permitted', 'Admin cannot appoint owners');
select tests.authenticate_as_hyprnote_pro('primary');
select lives_ok($$select public.set_workspace_membership_role((select workspace_id from ownership_state), tests.get_supabase_uid('coowner'), 'owner')$$, 'Primary can restore an owner');
select lives_ok($$select public.transfer_workspace_ownership((select workspace_id from ownership_state), tests.get_supabase_uid('coowner'))$$, 'Primary requests transfer to an existing owner');
select is((select owner_user_id from public.workspaces where id = (select workspace_id from ownership_state)), tests.get_supabase_uid('primary'), 'Pending transfer does not change primary');
select tests.authenticate_as_hyprnote_pro('coowner');
select lives_ok($$select public.respond_workspace_ownership_request((select workspace_id from ownership_state), (select id from public.list_workspace_ownership_requests((select workspace_id from ownership_state))), 'accept')$$, 'Recipient accepts primary ownership');
select is((select owner_user_id from public.workspaces where id = (select workspace_id from ownership_state)), tests.get_supabase_uid('coowner'), 'Acceptance changes the single primary');
select is((select role from public.list_workspace_members_with_profiles((select workspace_id from ownership_state)) where user_id = tests.get_supabase_uid('primary')), 'owner', 'Previous primary remains an owner');
select lives_ok($$select public.revoke_workspace_membership((select workspace_id from ownership_state), tests.get_supabase_uid('peerowner'))$$, 'Primary can remove an ordinary owner');
select tests.authenticate_as_hyprnote_pro('primary');
select lives_ok($$select public.leave_workspace((select workspace_id from ownership_state))$$, 'Previous primary can leave after accepted transfer');
select * from finish();
rollback;
