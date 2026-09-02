begin;
select plan(37);

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

create temporary table workspace_witness_test_event as
select
  key_id,
  repeat(lower(left(key_id, 1)), 43)::text as record_id,
  payload,
  rtrim(
    translate(encode(extensions.digest(payload, 'sha256'), 'base64'), '+/', '-_'),
    '='
  )::text as payload_hash
from (
  select key_id,
    jsonb_build_object(
      'version', 1,
      'key_id', key_id,
      'nonce', repeat('N', 32),
      'ciphertext', 'opaque-' || key_id
    )::text as payload
  from (
    values
      ('AAAAAAAAAAAAAAAAAAAAAA'::text),
      ('BBBBBBBBBBBBBBBBBBBBBB'::text),
      ('CCCCCCCCCCCCCCCCCCCCCC'::text)
  ) as key_ids(key_id)
) as payloads;

grant all on workspace_witness_test_event to authenticated, service_role;

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
        'list_all_my_workspace_e2ee_grants',
        'purge_workspace_e2ee_grants_for_membership',
        'initialize_shared_workspace_e2ee_witness'
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
        'list_my_workspace_e2ee_grants',
        'list_all_my_workspace_e2ee_grants'
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

select tests.enable_workspace_plan(
  (select workspace_id from workspace_grant_test_state where name = 'hq')
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
select tests.authenticate_as_service_role();

select isnt(
  (
    select e2ee_freshness_initialized_at
    from public.workspaces
    where id = (select workspace_id from workspace_grant_test_state where name = 'hq')
  ),
  null,
  'The first shared workspace key initializes its authoritative witness'
);

select lives_ok(
  $$
    select * from public.read_e2ee_freshness_page(
      tests.get_supabase_uid('grant_member'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      0,
      null,
      64
    )
  $$,
  'An active shared member can read the workspace witness through trusted service code'
);

select lives_ok(
  $$
    select * from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('grant_member'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from workspace_witness_test_event
        where key_id = 'AAAAAAAAAAAAAAAAAAAAAA'
      )
    )
  $$,
  'An active shared member can publish ciphertext sealed by the active generation'
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

select results_eq(
  $$
    select workspace_id, key_id, is_active
    from public.list_all_my_workspace_e2ee_grants()
  $$,
  $$
    values (
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'AAAAAAAAAAAAAAAAAAAAAA'::text,
      true
    )
  $$,
  'Credential delivery can fetch all wrapped keys addressed to a member'
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

select results_eq(
  $$select count(*) from public.list_all_my_workspace_e2ee_grants()$$,
  array[0::bigint],
  'Credential delivery reveals no grants from workspaces the caller does not belong to'
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

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select throws_ok(
  $$
    select * from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('grant_owner'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from workspace_witness_test_event
        where key_id = 'AAAAAAAAAAAAAAAAAAAAAA'
      )
    )
  $$,
  '22023',
  'E2EE freshness event is invalid',
  'A retired shared key generation can no longer publish witness events'
);

select lives_ok(
  $$
    select * from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('grant_owner'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from workspace_witness_test_event
        where key_id = 'BBBBBBBBBBBBBBBBBBBBBB'
      )
    )
  $$,
  'The active rotated generation can publish witness events'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('grant_owner');

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

select results_eq(
  $$
    select count(*)
    from public.workspace_e2ee_keys
    where workspace_id = (
      select workspace_id from workspace_grant_test_state where name = 'hq'
    )
      and retired_at is null
  $$,
  array[0::bigint],
  'Revoking membership retires the exposed generation before future writes can sync'
);

select throws_ok(
  $$
    select * from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('grant_owner'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      false,
      '[]'::jsonb
    )
  $$,
  '42501',
  'E2EE freshness publication is not permitted',
  'A membership change freezes publication until a replacement key is active'
);

select throws_ok(
  $$
    select * from public.read_e2ee_freshness_page(
      tests.get_supabase_uid('grant_owner'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      0,
      null,
      64
    )
  $$,
  '42501',
  'E2EE freshness read is not permitted',
  'A workspace without an active generation cannot consume witness history'
);

select tests.clear_authentication();
select tests.authenticate_as_hyprnote_pro('grant_owner');

select results_eq(
  $$
    select key_id, granted_member_count
    from public.set_workspace_e2ee_key(
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      'CCCCCCCCCCCCCCCCCCCCCC',
      jsonb_build_array(
        jsonb_build_object('userId', tests.get_supabase_uid('grant_owner'), 'ephemeralPublicKey', rpad('eo3', 43, 'A'), 'nonce', rpad('no3', 32, 'B'), 'ciphertext', rpad('co3', 64, 'C'))
      )
    )
  $$,
  $$values ('CCCCCCCCCCCCCCCCCCCCCC'::text, 1)$$,
  'The remaining manager can rotate to a key that excludes the removed member'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $$
    select * from public.publish_e2ee_freshness_events(
      tests.get_supabase_uid('grant_owner'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      false,
      (
        select jsonb_build_array(jsonb_build_object(
          'record_id', record_id,
          'payload_hash', payload_hash,
          'payload', payload
        ))
        from workspace_witness_test_event
        where key_id = 'CCCCCCCCCCCCCCCCCCCCCC'
      )
    )
  $$,
  'The replacement generation resumes witness publication for remaining members'
);

select throws_ok(
  $$
    select * from public.read_e2ee_freshness_page(
      tests.get_supabase_uid('grant_member'),
      (select workspace_id from workspace_grant_test_state where name = 'hq'),
      0,
      null,
      64
    )
  $$,
  '42501',
  'E2EE freshness read is not permitted',
  'A removed member cannot read witness history after the replacement key is active'
);

select tests.clear_authentication();
select tests.authenticate_as('grant_member');

select results_eq(
  $$select count(*) from public.list_all_my_workspace_e2ee_grants()$$,
  array[0::bigint],
  'A removed member cannot fetch any prior workspace grant'
);

select * from finish();
rollback;
