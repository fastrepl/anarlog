begin;
select plan(62);

select tests.create_supabase_user('pro_share_owner', 'pro-share-owner@example.com');
select tests.create_supabase_user('pro_share_recipient', 'pro-share-recipient@example.com');
select tests.create_supabase_user('pro_share_requester', 'pro-share-requester@example.com');

create temporary table session_sharing_pro_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid,
  secret text,
  capability text,
  access_version bigint,
  was_created boolean
);

grant all on session_sharing_pro_test_state to anon, authenticated, service_role;

insert into session_sharing_pro_test_state (name, workspace_id)
values ('target_workspace', gen_random_uuid());

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('pro_share_owner'),
  tests.get_supabase_uid('pro_share_recipient'),
  tests.get_supabase_uid('pro_share_requester')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('pro_share_owner'),
  'shared',
  'Pro sharing target workspace'
from session_sharing_pro_test_state
where name = 'target_workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('pro_share_owner'),
  'owner'
from session_sharing_pro_test_state
where name = 'target_workspace';

select tests.clear_authentication();
reset role;

select ok(
  not has_function_privilege(
    'authenticated',
    'private.create_session_share(uuid,text)',
    'EXECUTE'
  )
    and not has_function_privilege(
      'authenticated',
      'private.set_session_share_scope(uuid,text,uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.issue_session_share_link(uuid,boolean)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.create_session_access_invitation(uuid,text,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.resend_session_access_invitation(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.update_session_access_grant(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'private.review_session_access_request(uuid,text,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'private.protected_create_session_share(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'private.protected_set_session_share_scope(uuid,text,uuid)',
      'EXECUTE'
    ),
  'Authenticated callers cannot bypass entitlement-protected implementations'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share',
        'set_session_share_scope',
        'enable_session_share_link',
        'rotate_session_share_link',
        'create_session_access_invitation',
        'resend_session_access_invitation',
        'update_session_access_grant',
        'review_session_access_request'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
  ),
  'Public sharing management wrappers remain security invokers with empty search paths'
);

select tests.authenticate_as('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'missing-entitlement')
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'A permanent owner without a Pro entitlement cannot create a share'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.session_shares
    where session_id = 'missing-entitlement'
  $$,
  array[0::bigint],
  'Rejected share creation leaves no authority row'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('pro_share_owner');

select set_config(
  'request.jwt.claims',
  jsonb_set(
    (select auth.jwt()),
    '{entitlements}',
    '["hyprnote_lite"]'::jsonb,
    true
  )::text,
  true
);

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'wrong-entitlement')
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An unrelated entitlement cannot create a share'
);

select tests.authenticate_as('pro_share_owner');

select set_config(
  'request.jwt.claims',
  jsonb_set(
    (select auth.jwt()),
    '{user_metadata,entitlements}',
    '["hyprnote_pro"]'::jsonb,
    true
  )::text,
  true
);

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'forged-user-metadata')
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'User-editable metadata cannot forge the Pro entitlement'
);

select tests.authenticate_as('pro_share_owner');

select set_config(
  'request.jwt.claims',
  jsonb_set(
    jsonb_set(
      (select auth.jwt()),
      '{subscription_status}',
      '"trialing"'::jsonb,
      true
    ),
    '{trial_end}',
    to_jsonb(extract(epoch from now() + interval '1 day')::bigint),
    true
  )::text,
  true
);

select lives_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'valid-trial-share')
  $$,
  'An unexpired server-issued trial receives Pro sharing access'
);

select tests.authenticate_as('pro_share_owner');

select set_config(
  'request.jwt.claims',
  jsonb_set(
    jsonb_set(
      jsonb_set(
        (select auth.jwt()),
        '{entitlements}',
        '["hyprnote_pro"]'::jsonb,
        true
      ),
      '{subscription_status}',
      '"trialing"'::jsonb,
      true
    ),
    '{trial_end}',
    to_jsonb(extract(epoch from now() - interval '1 day')::bigint),
    true
  )::text,
  true
);

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'expired-trial-share')
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired trial no longer receives Pro sharing access even before entitlement propagation catches up'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      capability,
      access_version,
      was_created
    )
    select
      'main_share',
      share_id,
      general_scope,
      access_version,
      was_created
    from public.create_session_share(auth.uid(), 'pro-main-session')
  $$,
  'A Pro owner can create sharing authority'
);

select ok(
  (
    select capability = 'restricted'
      and access_version = 1
      and was_created
    from session_sharing_pro_test_state
    where name = 'main_share'
  ),
  'New Pro shares remain restricted by default'
);

select tests.authenticate_as('pro_share_owner');

select results_eq(
  $$
    select share_id, was_created
    from public.create_session_share(auth.uid(), 'pro-main-session')
  $$,
  $$
    select share_id, false
    from session_sharing_pro_test_state
    where name = 'main_share'
  $$,
  'An expired owner can reuse an already-active idempotent share'
);

select results_eq(
  $$
    select general_scope, has_active_link
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$values ('restricted'::text, false)$$,
  'An expired owner can inspect existing share management state'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (name, share_id)
    select 'reactivation_share', share_id
    from public.create_session_share(auth.uid(), 'reactivation-session')
  $$,
  'A Pro owner can create a share used to test reactivation'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

update public.session_shares
set deleted_at = now()
where id = (
  select share_id
  from session_sharing_pro_test_state
  where name = 'reactivation_share'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'reactivation-session')
  $$,
  '22023',
  'session share is unavailable',
  'Ordinary create never reactivates a deleted share'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select deleted_at is not null
    from public.session_shares
    where id = (
      select share_id
      from session_sharing_pro_test_state
      where name = 'reactivation_share'
    )
  $$,
  array[true],
  'Rejected reactivation leaves the share deleted'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.reactivate_session_share(auth.uid(), 'reactivation-session')
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'Explicit reactivation requires a current Pro entitlement'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.create_session_share(auth.uid(), 'reactivation-session')
  $$,
  '22023',
  'session share is unavailable',
  'A Pro create request still cannot implicitly reactivate a deleted share'
);

select lives_ok(
  $$
    select *
    from public.reactivate_session_share(auth.uid(), 'reactivation-session')
  $$,
  'A renewed Pro owner can explicitly reactivate a deleted share'
);

select results_eq(
  $$
    select general_scope
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'reactivation_share'
      )
    )
  $$,
  array['restricted'::text],
  'Reactivated shares return in restricted mode'
);

select tests.authenticate_as('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'public'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot make a share public'
);

select results_eq(
  $$
    select general_scope
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array['restricted'::text],
  'Rejected public access rolls back the scope change'
);

select throws_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_sharing_pro_test_state
        where name = 'target_workspace'
      )
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot expand access to a workspace'
);

select results_eq(
  $$
    select general_scope
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array['restricted'::text],
  'Rejected workspace access rolls back the scope change'
);

select throws_ok(
  $$
    select *
    from public.enable_session_share_link(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot enable link access'
);

select results_eq(
  $$
    select general_scope, has_active_link, access_version
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$values ('restricted'::text, false, 1::bigint)$$,
  'Rejected link access rolls back its token and version changes'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version,
      was_created
    )
    select
      'main_link',
      share_id,
      link_id,
      link_token,
      access_version,
      was_created
    from public.enable_session_share_link(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  'A Pro owner can enable link access'
);

select ok(
  (
    select secret ~ '^[A-Za-z0-9_-]{43}$' and was_created
    from session_sharing_pro_test_state
    where name = 'main_link'
  ),
  'Pro link enablement returns a one-time bearer token'
);

select tests.authenticate_as('pro_share_owner');

select results_eq(
  $$
    select link_id, link_token, was_created
    from public.enable_session_share_link(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$
    select entity_id, null::text, false
    from session_sharing_pro_test_state
    where name = 'main_link'
  $$,
  'An expired owner can make an idempotent call for an existing link'
);

select throws_ok(
  $$
    select *
    from public.rotate_session_share_link(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot rotate a bearer link'
);

select results_eq(
  $$
    select has_active_link, access_version
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$
    select true, access_version
    from session_sharing_pro_test_state
    where name = 'main_link'
  $$,
  'Rejected rotation keeps the active link and access version'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'restricted'
    )
  $$,
  'An expired owner can revoke link access by restricting the share'
);

select results_eq(
  $$
    select general_scope, has_active_link
    from public.get_session_share_management(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$values ('restricted'::text, false)$$,
  'Restricting a share revokes its active bearer link'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select results_eq(
  $$
    select general_scope, general_workspace_id
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'workspace',
      (
        select workspace_id
        from session_sharing_pro_test_state
        where name = 'target_workspace'
      )
    )
  $$,
  $$
    select 'workspace'::text, workspace_id
    from session_sharing_pro_test_state
    where name = 'target_workspace'
  $$,
  'A Pro owner can enable workspace access'
);

select tests.authenticate_as('pro_share_owner');

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'restricted'
    )
  $$,
  'An expired owner can remove workspace access'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select results_eq(
  $$
    select general_scope
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'public'
    )
  $$,
  array['public'::text],
  'A Pro owner can enable public access'
);

select tests.authenticate_as('pro_share_owner');

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'restricted'
    )
  $$,
  'An expired owner can remove public access'
);

select throws_ok(
  $$
    select *
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'pro-share-recipient@example.com',
      'viewer'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot create an invitation'
);

select results_eq(
  $$
    select count(*)
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array[0::bigint],
  'Rejected invitation creation leaves no pending access'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      entity_id,
      secret,
      was_created
    )
    select
      'recipient_invitation',
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      invitation_id,
      invite_token,
      was_created
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'pro-share-recipient@example.com',
      'viewer'
    )
  $$,
  'A Pro owner can create a named invitation'
);

select ok(
  (
    select secret ~ '^[A-Za-z0-9_-]{43}$' and was_created
    from session_sharing_pro_test_state
    where name = 'recipient_invitation'
  ),
  'Pro invitation creation returns a one-time bearer token'
);

select tests.authenticate_as('pro_share_owner');

select results_eq(
  $$
    select invitation_id, invite_token, was_created
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'pro-share-recipient@example.com',
      'viewer'
    )
  $$,
  $$
    select entity_id, null::text, false
    from session_sharing_pro_test_state
    where name = 'recipient_invitation'
  $$,
  'An expired owner can make an idempotent call for a pending invitation'
);

select throws_ok(
  $$
    select *
    from public.resend_session_access_invitation(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot resend an invitation'
);

select results_eq(
  $$
    select entry_id, status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$
    select entity_id, 'pending'::text
    from session_sharing_pro_test_state
    where name = 'recipient_invitation'
  $$,
  'Rejected resend preserves the original pending invitation'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_recipient');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      entity_id,
      capability
    )
    select 'recipient_grant', share_id, grant_id, capability
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_invitation'
      ),
      (
        select secret
        from session_sharing_pro_test_state
        where name = 'recipient_invitation'
      )
    )
  $$,
  'A non-Pro recipient can accept an already-authorized invitation'
);

select ok(
  (
    select capability = 'viewer'
    from session_sharing_pro_test_state
    where name = 'recipient_grant'
  ),
  'Invitation acceptance creates the owner-authorized Viewer grant'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_owner');

select results_eq(
  $$
    select entry_type, capability, status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  $$values ('grant'::text, 'viewer'::text, 'active'::text)$$,
  'An expired owner can inspect named access grants'
);

select throws_ok(
  $$
    select *
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_grant'
      ),
      'commenter'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot increase a named grant capability'
);

select results_eq(
  $$
    select capability
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array['viewer'::text],
  'Rejected grant increase preserves the previous capability'
);

select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select results_eq(
  $$
    select capability
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_grant'
      ),
      'editor'
    )
  $$,
  array['editor'::text],
  'A Pro owner can increase a named grant capability'
);

select tests.authenticate_as('pro_share_owner');

select results_eq(
  $$
    select capability
    from public.update_session_access_grant(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_grant'
      ),
      'commenter'
    )
  $$,
  array['commenter'::text],
  'An expired owner can downgrade a named grant'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_recipient');

select set_config(
  'request.jwt.claims',
  jsonb_set(
    jsonb_set(
      (select auth.jwt()),
      '{subscription_status}',
      '"trialing"'::jsonb,
      true
    ),
    '{trial_end}',
    to_jsonb(extract(epoch from now() - interval '1 day')::bigint),
    true
  )::text,
  true
);

select results_eq(
  $$
    select capability
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array['commenter'::text],
  'Trial expiry does not revoke a separately granted collaboration role'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_owner');

select lives_ok(
  $$
    select *
    from public.revoke_session_access_grant(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'recipient_grant'
      )
    )
  $$,
  'An expired owner can revoke a named grant'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_recipient');

select results_eq(
  $$
    select count(*)
    from public.resolve_my_session_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array[0::bigint],
  'Grant revocation removes the recipient access immediately'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (name, share_id, entity_id)
    select
      'revocable_invitation',
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      invitation_id
    from public.create_session_access_invitation(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'pro-share-requester@example.com',
      'viewer'
    )
  $$,
  'A Pro owner can create another pending invitation'
);

select tests.authenticate_as('pro_share_owner');

select lives_ok(
  $$
    select *
    from public.revoke_session_access_invitation(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'revocable_invitation'
      )
    )
  $$,
  'An expired owner can revoke a pending invitation'
);

select results_eq(
  $$
    select count(*)
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
  $$,
  array[0::bigint],
  'Revoked invitations disappear from active access management'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_requester');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      entity_id,
      capability
    )
    select
      'first_request',
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      request_id,
      requested_capability
    from public.request_session_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'editor'
    )
  $$,
  'A permanent requester can ask for named access without Pro'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_owner');

select throws_ok(
  $$
    select *
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'first_request'
      ),
      'approved',
      'editor'
    )
  $$,
  '42501',
  'hyprnote pro entitlement required',
  'An expired owner cannot approve an access request'
);

select results_eq(
  $$
    select status
    from public.list_session_share_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      )
    )
    where entry_id = (
      select entity_id
      from session_sharing_pro_test_state
      where name = 'first_request'
    )
  $$,
  array['pending'::text],
  'Rejected approval leaves the request pending'
);

select results_eq(
  $$
    select status
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'first_request'
      ),
      'denied'
    )
  $$,
  array['denied'::text],
  'An expired owner can deny an access request'
);

select tests.clear_authentication();
select tests.authenticate_as('pro_share_requester');

select lives_ok(
  $$
    insert into session_sharing_pro_test_state (
      name,
      share_id,
      entity_id,
      capability
    )
    select
      'second_request',
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      request_id,
      requested_capability
    from public.request_session_access(
      (
        select share_id
        from session_sharing_pro_test_state
        where name = 'main_share'
      ),
      'viewer'
    )
  $$,
  'A requester can submit a new request after denial'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('pro_share_owner');

select results_eq(
  $$
    select status, capability
    from public.review_session_access_request(
      (
        select entity_id
        from session_sharing_pro_test_state
        where name = 'second_request'
      ),
      'approved',
      'viewer'
    )
  $$,
  $$values ('approved'::text, 'viewer'::text)$$,
  'A Pro owner can approve an access request'
);

select * from finish();
rollback;
