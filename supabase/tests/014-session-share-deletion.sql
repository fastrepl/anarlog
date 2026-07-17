begin;
select plan(50);

select tests.create_supabase_user('share_delete_owner', 'share-delete-owner@example.com');
select tests.create_supabase_user('share_delete_recipient', 'share-delete-recipient@example.com');
select tests.create_supabase_user('share_delete_invitee', 'share-delete-invitee@example.com');
select tests.create_supabase_user('share_delete_requester', 'share-delete-requester@example.com');
select tests.create_supabase_user('share_delete_other', 'share-delete-other@example.com');

create temporary table session_share_deletion_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid,
  secret text,
  slug text,
  capability text,
  access_version bigint,
  deleted_at timestamptz,
  was_deleted boolean,
  was_reactivated boolean
);

grant all on session_share_deletion_test_state
  to anon, authenticated, service_role;

insert into session_share_deletion_test_state (name, workspace_id)
values
  ('workspace', gen_random_uuid()),
  ('anonymous_user', gen_random_uuid());

insert into auth.users (
  id,
  raw_user_meta_data,
  raw_app_meta_data,
  is_anonymous,
  created_at,
  updated_at
)
select
  workspace_id,
  jsonb_build_object('test_identifier', 'share_delete_anonymous'),
  '{}'::jsonb,
  true,
  now(),
  now()
from session_share_deletion_test_state
where name = 'anonymous_user';

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('share_delete_owner'),
  tests.get_supabase_uid('share_delete_recipient'),
  tests.get_supabase_uid('share_delete_invitee'),
  tests.get_supabase_uid('share_delete_requester'),
  tests.get_supabase_uid('share_delete_other')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('share_delete_owner'),
  'shared',
  'Share deletion workspace'
from session_share_deletion_test_state
where name = 'workspace';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('share_delete_owner'),
  'owner'
from session_share_deletion_test_state
where name = 'workspace';

select tests.clear_authentication();
reset role;

select ok(
  has_function_privilege(
    'authenticated',
    'public.delete_session_share(uuid)',
    'EXECUTE'
  )
    and has_function_privilege(
      'authenticated',
      'private.delete_session_share(uuid)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.delete_session_share_by_session(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'private.delete_session_share_by_session(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'public.reactivate_session_share(uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'authenticated',
      'private.protected_reactivate_session_share(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.delete_session_share(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'public.delete_session_share(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'private.delete_session_share(uuid)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.delete_session_share_by_session(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'public.delete_session_share_by_session(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'private.delete_session_share_by_session(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'anon',
      'public.reactivate_session_share(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'public.reactivate_session_share(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'service_role',
      'private.protected_reactivate_session_share(uuid,text)',
      'EXECUTE'
    ),
  'Only authenticated callers can reach share deletion authority'
);

select ok(
  not (
    select proc.prosecdef
    from pg_proc as proc
    where proc.oid = 'public.delete_session_share(uuid)'::regprocedure
  )
    and (
      select proc.prosecdef
      from pg_proc as proc
      where proc.oid = 'private.delete_session_share(uuid)'::regprocedure
    )
    and (
      select 'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      from pg_proc as proc
      where proc.oid = 'public.delete_session_share(uuid)'::regprocedure
    )
    and (
      select 'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      from pg_proc as proc
      where proc.oid = 'private.delete_session_share(uuid)'::regprocedure
    )
    and not (
      select proc.prosecdef
      from pg_proc as proc
      where proc.oid = 'public.delete_session_share_by_session(uuid,text)'::regprocedure
    )
    and (
      select proc.prosecdef
      from pg_proc as proc
      where proc.oid = 'private.delete_session_share_by_session(uuid,text)'::regprocedure
    )
    and (
      select 'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      from pg_proc as proc
      where proc.oid = 'public.delete_session_share_by_session(uuid,text)'::regprocedure
    )
    and (
      select 'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      from pg_proc as proc
      where proc.oid = 'private.delete_session_share_by_session(uuid,text)'::regprocedure
    )
    and lower(pg_get_functiondef(
      'private.delete_session_share(uuid)'::regprocedure
    )) not like '%require_hyprnote_pro_entitlement%'
    and lower(pg_get_functiondef(
      'private.delete_session_share_by_session(uuid,text)'::regprocedure
    )) not like '%require_hyprnote_pro_entitlement%'
    and not (
      select proc.prosecdef
      from pg_proc as proc
      where proc.oid = 'public.reactivate_session_share(uuid,text)'::regprocedure
    )
    and (
      select proc.prosecdef
      from pg_proc as proc
      where proc.oid = 'private.protected_reactivate_session_share(uuid,text)'::regprocedure
    )
    and lower(pg_get_functiondef(
      'private.protected_reactivate_session_share(uuid,text)'::regprocedure
    )) like '%require_hyprnote_pro_entitlement%',
  'Deletion is ungated while explicit reactivation is a hardened Pro-only RPC'
);

select ok(
  (
    select pg_get_constraintdef(constraint_record.oid) like '%share_deleted%'
    from pg_constraint as constraint_record
    join pg_class as class
      on class.oid = constraint_record.conrelid
    join pg_namespace as namespace
      on namespace.oid = class.relnamespace
    where namespace.nspname = 'public'
      and class.relname = 'session_access_events'
      and constraint_record.conname = 'session_access_events_type_check'
  ),
  'Share deletion is represented in the access audit log'
);

select ok(
  position(
    'pg_advisory_xact_lock' in lower(pg_get_functiondef(
      'private.delete_session_share(uuid)'::regprocedure
    ))
  ) < position(
    'for update' in lower(pg_get_functiondef(
      'private.delete_session_share(uuid)'::regprocedure
    ))
  )
    and position(
      'pg_advisory_xact_lock' in lower(pg_get_functiondef(
        'private.delete_session_share(uuid)'::regprocedure
      ))
    ) < position(
      'delete from private.session_share_handoffs' in lower(pg_get_functiondef(
        'private.delete_session_share(uuid)'::regprocedure
      ))
    )
    and lower(pg_get_functiondef(
      'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
    )) like '%share.deleted_at is null%'
    and lower(pg_get_functiondef(
      'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
    )) like '%share.access_version = p_access_version%'
    and lower(pg_get_functiondef(
      'private.issue_session_share_handoff(uuid,text,uuid,bigint,bytea)'::regprocedure
    )) not like '%delete from private.session_share_handoffs%',
  'Deletion and issuance share one lock order and issuance revalidates authority'
);

select ok(
  position(
    'pg_advisory_xact_lock' in lower(pg_get_functiondef(
      'private.create_session_share(uuid,text)'::regprocedure
    ))
  ) < position(
    'for update of workspace, membership' in lower(pg_get_functiondef(
      'private.create_session_share(uuid,text)'::regprocedure
    ))
  )
    and position(
      'pg_advisory_xact_lock' in lower(pg_get_functiondef(
        'private.protected_reactivate_session_share(uuid,text)'::regprocedure
      ))
    ) < position(
      'for update;' in lower(pg_get_functiondef(
        'private.protected_reactivate_session_share(uuid,text)'::regprocedure
      ))
    )
    and lower(pg_get_functiondef(
      'private.create_session_share(uuid,text)'::regprocedure
    )) not like '%deleted_at = null%'
    and lower(pg_get_functiondef(
      'private.create_session_share(uuid,text)'::regprocedure
    )) not like '%share_reactivated%'
    and lower(pg_get_functiondef(
      'private.create_session_share(uuid,text)'::regprocedure
    )) like '%session share is unavailable%',
  'Create, delete, and explicit reactivation serialize without implicit resurrection'
);

select tests.authenticate_as('share_delete_owner');

select results_eq(
  $$
    select share_id, access_version, deleted_at, was_deleted
    from public.delete_session_share_by_session(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'never-shared-session'
    )
  $$,
  $$values (NULL::uuid, NULL::bigint, NULL::timestamptz, false)$$,
  'Source identity deletion is a successful no-op when a note was never shared'
);

select tests.authenticate_as_hyprnote_pro('share_delete_owner');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      slug,
      access_version
    )
    select 'link_share', share_id, public_slug, access_version
    from public.create_session_share(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-link-session'
    )
  $$,
  'The owner can create the link-share fixture'
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      entity_id,
      secret,
      access_version
    )
    select 'active_link', share_id, link_id, link_token, access_version
    from public.enable_session_share_link(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  'The deletion fixture has an active bearer link'
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, entity_id, secret)
    select 'accepted_invitation', invitation_id, invite_token
    from public.create_session_access_invitation(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      'share-delete-recipient@example.com',
      'editor'
    )
  $$,
  'The owner can create an invitation that becomes an active grant'
);

select tests.authenticate_as('share_delete_recipient');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, share_id, entity_id)
    select 'active_grant', share_id, grant_id
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_share_deletion_test_state
        where name = 'accepted_invitation'
      ),
      (
        select secret
        from session_share_deletion_test_state
        where name = 'accepted_invitation'
      )
    )
  $$,
  'The recipient can accept the fixture invitation'
);

select tests.authenticate_as_hyprnote_pro('share_delete_owner');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, entity_id, secret)
    select 'pending_invitation', invitation_id, invite_token
    from public.create_session_access_invitation(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      'share-delete-invitee@example.com',
      'commenter'
    )
  $$,
  'The deletion fixture has an unaccepted invitation'
);

select tests.authenticate_as('share_delete_requester');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, entity_id)
    select 'pending_request', request_id
    from public.request_session_access(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      'viewer'
    )
  $$,
  'The deletion fixture has a pending access request'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      tests.get_supabase_uid('share_delete_owner'),
      'Deleted link share',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Private"}]}]}'::jsonb
    )
  $$,
  'The deletion fixture has a sanitized snapshot'
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, secret)
    select 'link_handoff', request_id
    from public.gateway_create_session_share_link_handoff(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_share_deletion_test_state
        where name = 'active_link'
      ),
      repeat('c', 64)
    )
  $$,
  'The deletion fixture has a pending one-time handoff'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('share_delete_owner');

insert into session_share_deletion_test_state (name, share_id, access_version)
select 'pre_delete', share_id, access_version
from public.get_session_share_management(
  (
    select share_id
    from session_share_deletion_test_state
    where name = 'link_share'
  )
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      access_version,
      deleted_at,
      was_deleted
    )
    select
      'first_delete',
      share_id,
      access_version,
      deleted_at,
      was_deleted
    from public.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  'An owner without a current Pro entitlement can delete a share'
);

select is(
  (
    select
      (
        select count(*) = 4
        from jsonb_object_keys(to_jsonb(deletion))
      )
        and to_jsonb(deletion) ?& array[
          'share_id',
          'access_version',
          'deleted_at',
          'was_deleted'
        ]
        and not deletion.was_deleted
        and deletion.deleted_at = first_delete.deleted_at
        and deletion.access_version = first_delete.access_version
    from public.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    ) as deletion
    join session_share_deletion_test_state as original
      on original.name = 'link_share'
    join session_share_deletion_test_state as first_delete
      on first_delete.name = 'first_delete'
  ),
  true,
  'A repeat call reports the stable four-field result without deleting twice'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select
      share.general_scope = 'restricted'
        and share.general_workspace_id is null
        and share.public_slug <> original.slug
        and share.access_version = deletion.access_version
        and share.access_version = pre_delete.access_version + 1
        and share.deleted_at = deletion.deleted_at
        and deletion.was_deleted
    from public.session_shares as share
    join session_share_deletion_test_state as original
      on original.name = 'link_share'
      and original.share_id = share.id
    join session_share_deletion_test_state as deletion
      on deletion.name = 'first_delete'
    join session_share_deletion_test_state as pre_delete
      on pre_delete.name = 'pre_delete'
  ),
  'Deletion restricts, versions, rotates, and soft-deletes the authority row'
);

select ok(
  (
    select
      link.revoked_at = deletion.deleted_at
        and link.revoked_by_user_id = tests.get_supabase_uid('share_delete_owner')
    from public.session_share_links as link
    join session_share_deletion_test_state as active_link
      on active_link.name = 'active_link'
      and active_link.entity_id = link.id
    join session_share_deletion_test_state as deletion
      on deletion.name = 'first_delete'
  ),
  'Deletion revokes the active bearer link'
);

select ok(
  (
    select
      access_grant.revoked_at = deletion.deleted_at
        and access_grant.revoked_by_user_id = tests.get_supabase_uid('share_delete_owner')
    from public.session_access_grants as access_grant
    join session_share_deletion_test_state as active_grant
      on active_grant.name = 'active_grant'
      and active_grant.entity_id = access_grant.id
    join session_share_deletion_test_state as deletion
      on deletion.name = 'first_delete'
  ),
  'Deletion revokes every active named grant'
);

select ok(
  (
    select
      invitation.accepted_at is null
        and invitation.revoked_at = deletion.deleted_at
        and invitation.revoked_by_user_id = tests.get_supabase_uid('share_delete_owner')
    from public.session_access_invitations as invitation
    join session_share_deletion_test_state as pending_invitation
      on pending_invitation.name = 'pending_invitation'
      and pending_invitation.entity_id = invitation.id
    join session_share_deletion_test_state as deletion
      on deletion.name = 'first_delete'
  ),
  'Deletion revokes every unaccepted invitation'
);

select ok(
  (
    select
      access_request.status = 'cancelled'
        and access_request.reviewed_at is null
        and access_request.reviewed_by_user_id is null
    from public.session_access_requests as access_request
    join session_share_deletion_test_state as pending_request
      on pending_request.name = 'pending_request'
      and pending_request.entity_id = access_request.id
  ),
  'Deletion cancels pending access requests without fabricating a review'
);

select tests.clear_authentication();
reset role;

select ok(
  (
    select count(*) = 0
    from private.session_share_handoffs as handoff
    where handoff.share_id = (
      select share_id
      from session_share_deletion_test_state
      where name = 'link_share'
    )
  )
    and (
      select count(*) = 1
      from public.session_share_snapshots as snapshot
      where snapshot.share_id = (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    ),
  'Deletion cancels handoffs while retaining the soft-deleted snapshot data'
);

select results_eq(
  $$
    select event_type, actor_user_id, new_value
    from public.session_access_events
    where share_id = (
      select share_id
      from session_share_deletion_test_state
      where name = 'link_share'
    )
      and event_type = 'share_deleted'
  $$,
  $$
    values (
      'share_deleted'::text,
      tests.get_supabase_uid('share_delete_owner'),
      'restricted'::text
    )
  $$,
  'Deletion records one auditable authority transition'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('share_delete_owner');

select results_eq(
  $$
    select share_id, access_version, deleted_at, was_deleted
    from public.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  $$
    select share_id, access_version, deleted_at, false
    from session_share_deletion_test_state
    where name = 'first_delete'
  $$,
  'The same source-workspace manager can repeat deletion idempotently'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select share.access_version = deletion.access_version
    from public.session_shares as share
    join session_share_deletion_test_state as deletion
      on deletion.name = 'first_delete'
      and deletion.share_id = share.id
  )
    and (
      select count(*) = 1
      from public.session_access_events
      where share_id = (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
        and event_type = 'share_deleted'
    ),
  'Idempotent deletion does not bump access version or duplicate the event'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('share_delete_owner');

select throws_ok(
  $$
    select *
    from public.get_session_share_management(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Deleted authority is unavailable through management reads'
);

select tests.authenticate_as('share_delete_other');

select throws_ok(
  $$
    select *
    from public.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'A nonmanager cannot delete an existing share'
);

select throws_ok(
  $$
    select *
    from private.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Direct private execution does not bypass manager authorization'
);

select throws_ok(
  $$select * from public.delete_session_share(gen_random_uuid())$$,
  '42501',
  'session access operation not permitted',
  'Unknown and unauthorized share ids use the same denial'
);

select throws_ok(
  $$
    select *
    from public.delete_session_share_by_session(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-link-session'
    )
  $$,
  '42501',
  'session access operation not permitted',
  'A nonmanager cannot use source identity to probe or delete shares'
);

select tests.authenticate_as('share_delete_anonymous');

select throws_ok(
  $$
    select *
    from public.delete_session_share(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  $$,
  '42501',
  'session access operation not permitted',
  'Anonymous Auth identities cannot delete shares'
);

select tests.clear_authentication();
reset role;
set local role anon;

select throws_ok(
  $$select * from public.delete_session_share(gen_random_uuid())$$,
  '42501',
  'permission denied for function delete_session_share',
  'The unauthenticated database role cannot execute share deletion'
);

reset role;
select tests.authenticate_as('share_delete_recipient');

select ok(
  (
    select count(*) = 0
    from public.resolve_my_session_access(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    )
  )
    and (
      select count(*) = 0
      from public.read_my_session_share_snapshot(
        (
          select share_id
          from session_share_deletion_test_state
          where name = 'link_share'
        )
      )
    )
    and (
      select count(*) = 0
      from public.list_my_session_share_snapshots()
      where share_id = (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      )
    ),
  'Named account resolution and snapshot reads disappear after deletion'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select count(*) = 0
    from public.gateway_read_session_share_link_snapshot(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      (
        select secret
        from session_share_deletion_test_state
        where name = 'active_link'
      )
    )
  )
    and (
      select count(*) = 0
      from public.gateway_create_session_share_link_handoff(
        (
          select share_id
          from session_share_deletion_test_state
          where name = 'link_share'
        ),
        (
          select secret
          from session_share_deletion_test_state
          where name = 'active_link'
        ),
        repeat('d', 64)
      )
    ),
  'Bearer reads and new handoffs disappear after deletion'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      (
        select secret
        from session_share_deletion_test_state
        where name = 'link_handoff'
      ),
      '11111111-1111-4111-8111-111111111111'
    )
  $$,
  array[0::bigint],
  'A handoff issued before deletion cannot be claimed'
);

select throws_ok(
  $$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'link_share'
      ),
      tests.get_supabase_uid('share_delete_owner'),
      'Unavailable',
      '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb
    )
  $$,
  '42501',
  'session snapshot publication not permitted',
  'Trusted publication cannot update a deleted share snapshot'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('share_delete_invitee');

select throws_ok(
  $$
    select *
    from public.accept_session_access_invitation(
      (
        select entity_id
        from session_share_deletion_test_state
        where name = 'pending_invitation'
      ),
      (
        select secret
        from session_share_deletion_test_state
        where name = 'pending_invitation'
      )
    )
  $$,
  '22023',
  'session access invitation is invalid or unavailable',
  'A revoked pending invitation cannot be accepted after deletion'
);

select tests.authenticate_as_hyprnote_pro('share_delete_owner');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      slug,
      access_version
    )
    select 'public_share', share_id, public_slug, access_version
    from public.create_session_share(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-public-session'
    )
  $$,
  'The owner can create the public-share fixture'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'public_share'
      ),
      'public'
    )
  $$,
  'The public-share fixture is accessible by slug'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select lives_ok(
  $$
    select *
    from public.publish_session_share_snapshot(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'public_share'
      ),
      tests.get_supabase_uid('share_delete_owner'),
      'Deleted public share',
      '{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Public"}]}]}'::jsonb
    )
  $$,
  'The public-share fixture has a sanitized snapshot'
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (name, secret)
    select 'public_handoff', request_id
    from public.gateway_create_public_session_share_handoff(
      (
        select slug
        from session_share_deletion_test_state
        where name = 'public_share'
      ),
      repeat('e', 64)
    )
  $$,
  'The public-share fixture has a pending one-time handoff'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as('share_delete_owner');

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      access_version,
      deleted_at,
      was_deleted
    )
    select
      'public_delete',
      share_id,
      access_version,
      deleted_at,
      was_deleted
    from public.delete_session_share_by_session(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      '  share-delete-public-session  '
    )
  $$,
  'An expired owner can delete public access using only local session identity'
);

select results_eq(
  $$
    select share_id, access_version, deleted_at, was_deleted
    from public.delete_session_share_by_session(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-public-session'
    )
  $$,
  $$
    select share_id, access_version, deleted_at, false
    from session_share_deletion_test_state
    where name = 'public_delete'
  $$,
  'Source identity deletion remains idempotent after the share is soft-deleted'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select count(*) = 0
    from public.gateway_read_public_session_share_snapshot(
      (
        select slug
        from session_share_deletion_test_state
        where name = 'public_share'
      )
    )
  )
    and (
      select count(*) = 0
      from public.gateway_create_public_session_share_handoff(
        (
          select slug
          from session_share_deletion_test_state
          where name = 'public_share'
        ),
        repeat('f', 64)
      )
    ),
  'Public reads and new handoffs disappear after deletion'
);

select results_eq(
  $$
    select count(*)
    from public.gateway_lease_session_share_handoff(
      (
        select secret
        from session_share_deletion_test_state
        where name = 'public_handoff'
      ),
      '22222222-2222-4222-8222-222222222222'
    )
  $$,
  array[0::bigint],
  'A public handoff issued before deletion cannot be claimed'
);

insert into session_share_deletion_test_state (
  name,
  share_id,
  slug,
  access_version,
  deleted_at
)
select
  'deleted_public_state',
  share.id,
  share.public_slug,
  share.access_version,
  share.deleted_at
from public.session_shares as share
where share.id = (
  select share_id
  from session_share_deletion_test_state
  where name = 'public_share'
);

select tests.clear_authentication();
reset role;
select tests.authenticate_as_hyprnote_pro('share_delete_owner');

select throws_ok(
  $$
    select *
    from public.create_session_share(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-public-session'
    )
  $$,
  '22023',
  'session share is unavailable',
  'A stale Pro create request cannot race deletion into reactivation'
);

select lives_ok(
  $$
    insert into session_share_deletion_test_state (
      name,
      share_id,
      slug,
      capability,
      access_version,
      was_reactivated
    )
    select
      'reactivated_public_share',
      share_id,
      public_slug,
      general_scope,
      access_version,
      was_reactivated
    from public.reactivate_session_share(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-public-session'
    )
  $$,
  'A renewed Pro owner can explicitly reactivate the share'
);

select ok(
  (
    select
      reactivated.share_id = original.share_id
        and reactivated.slug = deleted_state.slug
        and reactivated.slug <> original.slug
        and reactivated.capability = 'restricted'
        and reactivated.was_reactivated
        and reactivated.access_version = deleted.access_version + 1
        and repeated.share_id = reactivated.share_id
        and repeated.general_scope = 'restricted'
        and repeated.public_slug = reactivated.slug
        and repeated.access_version = reactivated.access_version
        and not repeated.was_reactivated
    from public.reactivate_session_share(
      (
        select workspace_id
        from session_share_deletion_test_state
        where name = 'workspace'
      ),
      'share-delete-public-session'
    ) as repeated
    cross join session_share_deletion_test_state as reactivated
    join session_share_deletion_test_state as original
      on original.name = 'public_share'
    join session_share_deletion_test_state as deleted
      on deleted.name = 'public_delete'
    join session_share_deletion_test_state as deleted_state
      on deleted_state.name = 'deleted_public_state'
    where reactivated.name = 'reactivated_public_share'
  ),
  'Explicit reactivation preserves the rotated restricted identity and is idempotent'
);

select lives_ok(
  $$
    select *
    from public.set_session_share_scope(
      (
        select share_id
        from session_share_deletion_test_state
        where name = 'reactivated_public_share'
      ),
      'public'
    )
  $$,
  'The reactivated share can be explicitly published again'
);

select tests.clear_authentication();
select tests.authenticate_as_service_role();

select ok(
  (
    select count(*) = 0
    from public.gateway_read_public_session_share_snapshot(
      (
        select slug
        from session_share_deletion_test_state
        where name = 'public_share'
      )
    )
  )
    and (
      select count(*) = 1
      from public.gateway_read_public_session_share_snapshot(
        (
          select slug
          from session_share_deletion_test_state
          where name = 'reactivated_public_share'
        )
      )
    ),
  'Only the newly issued public URL works after explicit reactivation'
);

select * from finish();
rollback;
