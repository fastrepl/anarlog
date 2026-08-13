begin;
select plan(23);

select tests.create_supabase_user('grant_owner', 'grant-owner@example.com');
select tests.create_supabase_user('grant_member', 'grant-member@example.com');
select tests.create_supabase_user('grant_outsider', 'grant-outsider@example.com');

create temporary table workspace_grant_test_state (
  name text primary key,
  workspace_id uuid,
  invitation_id uuid,
  invite_token text
);

grant all on workspace_grant_test_state to authenticated, service_role;

reset role;

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('grant_owner'),
  tests.get_supabase_uid('grant_member'),
  tests.get_supabase_uid('grant_outsider')
);

select ok(
  not has_table_privilege('authenticated', 'public.e2ee_member_identities', 'SELECT')
    and not has_table_privilege('authenticated', 'public.workspace_e2ee_keys', 'SELECT')
    and not has_table_privilege('authenticated', 'public.workspace_e2ee_key_grants', 'SELECT')
    and not has_table_privilege('authenticated', 'public.workspace_e2ee_key_grants', 'INSERT')
    and not has_table_privilege('authenticated', 'public.workspace_e2ee_key_grants', 'UPDATE'),
  'Clients cannot read or write identity, key, or grant rows directly'
);

select ok(
  not exists (
    select 1
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where (
      namespace.nspname = 'private'
      and proc.proname in (
        'publish_e2ee_member_identity',
        'list_workspace_key_recipients',
        'set_workspace_e2ee_key',
        'list_my_workspace_e2ee_grants',
        'purge_workspace_e2ee_grants_for_membership'
      )
      and (
        not proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
    or (
      namespace.nspname = 'public'
      and proc.proname in (
        'publish_e2ee_member_identity',
        'list_workspace_key_recipients',
        'set_workspace_e2ee_key',
        'list_my_workspace_e2ee_grants'
      )
      and (
        proc.prosecdef
        or not ('search_path=""' = any(coalesce(proc.proconfig, array[]::text[])))
      )
    )
  ),
  'Privileged E2EE grant implementations are private and use an empty search path'
);

select tests.authenticate_as_hyprnote_pro('grant_owner');

select results_eq(
  $$select public_key from public.publish_e2ee_member_identity(rpad('owner', 43, 'A'))$$,
  array[rpad('owner', 43, 'A')],
  'A member can publish an account identity public key'
);

select results_eq(
  $$select public_key from public.publish_e2ee_member_identity(rpad('owner2', 43, 'A'))$$,
  array[rpad('owner2', 43, 'A')],
  'Republishing replaces the stored identity key'
);

select throws_ok(
  $$select * from public.publish_e2ee_member_identity('too-short')$$,
  '22023',
  'E2EE member identity is invalid',
  'Malformed identity keys are rejected'
);

select lives_ok(
  $$
    insert into workspace_grant_test_state (name, workspace_id)
    select 'hq', workspace_id from public.create_workspace('Grant HQ')
  $$,
  'The owner creates a shared workspace to key'
);

select lives_ok(
  $$
    insert into workspace_grant_test_state (name, invitation_id, invite_token)
    select 'member_invite', invitation_id, invite_token
    from public.create_workspace_invitation(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'grant-member@example.com'
    )
  $$,
  'The owner invites a second member'
);

select tests.clear_authentication();
select tests.authenticate_as('grant_member');

select lives_ok(
  $$
    select * from public.accept_workspace_invitation(
      (select invitation_id from workspace_grant_test_state where name = 'member_invite'),
      (select invite_token from workspace_grant_test_state where name = 'member_invite')
    )
  $$,
  'The invited member joins the workspace'
);

select lives_ok(
  $$select * from public.publish_e2ee_member_identity(rpad('member', 43, 'A'))$$,
  'The joining member publishes their identity key'
);

select throws_ok(
  $$
    select * from public.list_workspace_key_recipients(
      (select workspace_id from workspace_grant_test_state where name = 'hq')
    )
  $$,
  '42501',
  'E2EE key operation not permitted',
  'Plain members cannot enumerate key recipients'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('grant_owner');

select results_eq(
  $$
    select user_email, public_key is not null, granted_key_ids
    from public.list_workspace_key_recipients(
      (select workspace_id from workspace_grant_test_state where name = 'hq')
    )
    order by user_email
  $$,
  $$
    values
      ('grant-member@example.com'::text, true, array[]::text[]),
      ('grant-owner@example.com'::text, true, array[]::text[])
  $$,
  'Managers see every active member with their identity key and current coverage'
);

select throws_ok(
  $$
    select * from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'not-a-valid-key-id',
      '[]'::jsonb
    )
  $$,
  '22023',
  'E2EE key identity is invalid',
  'Malformed workspace key identifiers are rejected'
);

select throws_ok(
  $$
    select * from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'AAAAAAAAAAAAAAAAAAAAAA',
      jsonb_build_array(jsonb_build_object('userId', tests.get_supabase_uid('grant_member'), 'ephemeralPublicKey', rpad('ex', 43, 'A'), 'nonce', rpad('nx', 32, 'B'), 'ciphertext', rpad('cx', 64, 'C')))
    )
  $$,
  '22023',
  'E2EE key grants must include the issuing member',
  'A rotation that would lock out its own issuer is rejected'
);

select throws_ok(
  $$
    select * from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'AAAAAAAAAAAAAAAAAAAAAA',
      jsonb_build_array(
        jsonb_build_object('userId', tests.get_supabase_uid('grant_owner'), 'ephemeralPublicKey', rpad('eo', 43, 'A'), 'nonce', rpad('no', 32, 'B'), 'ciphertext', rpad('co', 64, 'C')),
        jsonb_build_object('userId', tests.get_supabase_uid('grant_outsider'), 'ephemeralPublicKey', rpad('ez', 43, 'A'), 'nonce', rpad('nz', 32, 'B'), 'ciphertext', rpad('cz', 64, 'C'))
      )
    )
  $$,
  '22023',
  'E2EE key grants must target active members',
  'Grants cannot be issued to non-members'
);

select results_eq(
  $$
    select key_id, granted_member_count
    from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'AAAAAAAAAAAAAAAAAAAAAA',
      jsonb_build_array(
        jsonb_build_object('userId', tests.get_supabase_uid('grant_owner'), 'ephemeralPublicKey', rpad('eo1', 43, 'A'), 'nonce', rpad('no1', 32, 'B'), 'ciphertext', rpad('co1', 64, 'C')),
        jsonb_build_object('userId', tests.get_supabase_uid('grant_member'), 'ephemeralPublicKey', rpad('em1', 43, 'A'), 'nonce', rpad('nm1', 32, 'B'), 'ciphertext', rpad('cm1', 64, 'C'))
      )
    )
  $$,
  $$values ('AAAAAAAAAAAAAAAAAAAAAA'::text, 2)$$,
  'The first workspace key is stored with a grant per member'
);

select tests.clear_authentication();
select tests.authenticate_as('grant_member');

select results_eq(
  $$
    select key_id, is_active
    from public.list_my_workspace_e2ee_grants(
      (select workspace_id from workspace_grant_test_state where name = 'hq')
    )
  $$,
  $$values ('AAAAAAAAAAAAAAAAAAAAAA'::text, true)$$,
  'A member can fetch the wrapped key addressed to them'
);

select throws_ok(
  $$
    select * from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'BBBBBBBBBBBBBBBBBBBBBB',
      jsonb_build_array(jsonb_build_object('userId', tests.get_supabase_uid('grant_member'), 'ephemeralPublicKey', rpad('em2', 43, 'A'), 'nonce', rpad('nm2', 32, 'B'), 'ciphertext', rpad('cm2', 64, 'C')))
    )
  $$,
  '42501',
  'E2EE key operation not permitted',
  'Plain members cannot rotate the workspace key'
);

select tests.clear_authentication();
select tests.authenticate_as('grant_outsider');

select throws_ok(
  $$
    select * from public.list_my_workspace_e2ee_grants(
      (select workspace_id from workspace_grant_test_state where name = 'hq')
    )
  $$,
  '42501',
  'E2EE key operation not permitted',
  'Non-members cannot read wrapped keys for a workspace'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('grant_owner');

-- Rotation: mint a second generation covering only the remaining member.
select lives_ok(
  $$
    select * from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'BBBBBBBBBBBBBBBBBBBBBB',
      jsonb_build_array(
        jsonb_build_object('userId', tests.get_supabase_uid('grant_owner'), 'ephemeralPublicKey', rpad('eo2', 43, 'A'), 'nonce', rpad('no2', 32, 'B'), 'ciphertext', rpad('co2', 64, 'C')),
        jsonb_build_object('userId', tests.get_supabase_uid('grant_member'), 'ephemeralPublicKey', rpad('em2', 43, 'A'), 'nonce', rpad('nm2', 32, 'B'), 'ciphertext', rpad('cm2', 64, 'C'))
      )
    )
  $$,
  'A manager can rotate the workspace to a new key generation'
);

select results_eq(
  $$
    select key_id, is_active
    from public.list_my_workspace_e2ee_grants(
      (select workspace_id from workspace_grant_test_state where name = 'hq')
    )
  $$,
  $$
    values
      ('AAAAAAAAAAAAAAAAAAAAAA'::text, false),
      ('BBBBBBBBBBBBBBBBBBBBBB'::text, true)
  $$,
  'Rotation retires the previous generation while history stays readable'
);

select lives_ok(
  $$
    select * from public.revoke_workspace_membership(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      tests.get_supabase_uid('grant_member')
    )
  $$,
  'The owner revokes the member'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select results_eq(
  $$
    select count(*)
    from public.workspace_e2ee_key_grants
    where member_user_id = tests.get_supabase_uid('grant_member')
  $$,
  array[0::bigint],
  'Revoking membership deletes every wrapped key that member could fetch'
);

select results_eq(
  $$
    select count(*)
    from public.workspace_e2ee_key_grants
    where member_user_id = tests.get_supabase_uid('grant_owner')
  $$,
  array[2::bigint],
  'Remaining members keep their grants across both generations'
);

select * from finish();
rollback;
