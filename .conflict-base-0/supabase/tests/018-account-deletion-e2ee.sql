begin;
select plan(36);

select tests.create_supabase_user(
  'cloudsync_delete_owner',
  'cloudsync-delete-owner@example.com'
);
select tests.create_supabase_user(
  'cloudsync_foreign_owner',
  'cloudsync-foreign-owner@example.com'
);
select tests.create_supabase_user(
  'cloudsync_untracked_owner',
  'cloudsync-untracked-owner@example.com'
);
select tests.create_supabase_user(
  'cloudsync_late_customer_owner',
  'cloudsync-late-customer-owner@example.com'
);

insert into auth.users (
  id,
  raw_user_meta_data,
  raw_app_meta_data,
  is_anonymous,
  created_at,
  updated_at
)
values (
  '00000000-0000-4000-8000-000000000910'::uuid,
  '{}'::jsonb,
  '{}'::jsonb,
  true,
  now(),
  now()
);

create temporary table account_deletion_e2ee_state (
  owner_user_id uuid primary key,
  workspace_ids uuid[]
);
grant all on account_deletion_e2ee_state to service_role;

insert into account_deletion_e2ee_state (owner_user_id)
values (tests.get_supabase_uid('cloudsync_delete_owner'));

update public.profiles as profile
set stripe_customer_id = 'cus_deleteowner901'
from account_deletion_e2ee_state as state
where profile.id = state.owner_user_id;

insert into public.workspaces (id, owner_user_id, kind, name)
select
  '00000000-0000-4000-8000-000000000901'::uuid,
  owner_user_id,
  'shared',
  'Owned shared workspace'
from account_deletion_e2ee_state;

insert into public.workspace_memberships (id, workspace_id, user_id, role)
select
  '00000000-0000-4000-8000-000000000902'::uuid,
  '00000000-0000-4000-8000-000000000901'::uuid,
  owner_user_id,
  'owner'
from account_deletion_e2ee_state;

insert into public.workspace_memberships (id, workspace_id, user_id, role)
select
  '00000000-0000-4000-8000-000000000909'::uuid,
  '00000000-0000-4000-8000-000000000901'::uuid,
  tests.get_supabase_uid('cloudsync_foreign_owner'),
  'member';

insert into public.workspace_memberships (id, workspace_id, user_id, role)
select
  '00000000-0000-4000-8000-000000000903'::uuid,
  tests.get_supabase_uid('cloudsync_foreign_owner'),
  owner_user_id,
  'member'
from account_deletion_e2ee_state;

select has_column(
  'private',
  'account_deletion_jobs',
  'e2ee_workspace_ids',
  'Account deletion durably stores the owned E2EE workspace scope'
);

select has_column(
  'private',
  'account_deletion_jobs',
  'e2ee_purged_at',
  'Account deletion durably stores the E2EE purge checkpoint'
);

select has_column(
  'private',
  'account_deletion_jobs',
  'stripe_customer_id',
  'Account deletion durably snapshots its Stripe customer'
);

select has_column(
  'private',
  'account_deletion_jobs',
  'stripe_deleted_at',
  'Account deletion durably stores the Stripe deletion checkpoint'
);

select ok(
  has_function_privilege(
    'service_role',
    'public.claim_account_deletion_leases_v2(uuid,integer,integer)',
    'EXECUTE'
  )
    and has_function_privilege(
      'service_role',
      'public.mark_account_deletion_stripe_deleted(uuid,uuid,text)',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.mark_account_deletion_e2ee_purged(uuid,uuid,uuid[])',
      'EXECUTE'
    )
    and has_function_privilege(
      'service_role',
      'public.assign_profile_stripe_customer(uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.claim_account_deletion_leases_v2(uuid,integer,integer)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_account_deletion_stripe_deleted(uuid,uuid,text)',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.mark_account_deletion_e2ee_purged(uuid,uuid,uuid[])',
      'EXECUTE'
    )
    and not has_function_privilege(
      'authenticated',
      'public.assign_profile_stripe_customer(uuid,text)',
      'EXECUTE'
    ),
  'Only the service role can operate the E2EE cleanup checkpoint'
);

select ok(
  not has_table_privilege('authenticated', 'public.profiles', 'INSERT')
    and not has_table_privilege('authenticated', 'public.profiles', 'UPDATE')
    and not has_table_privilege('authenticated', 'public.profiles', 'DELETE')
    and has_table_privilege('service_role', 'public.profiles', 'INSERT')
    and has_table_privilege('service_role', 'public.profiles', 'UPDATE')
    and has_table_privilege('service_role', 'public.profiles', 'DELETE'),
  'Only trusted services can mutate Stripe customer authority'
);

select tests.authenticate_as_service_role();

select results_eq(
  format(
    $$
      select assigned_customer_id
      from public.assign_profile_stripe_customer(
        %L::uuid,
        'cus_foreignowner901'
      )
    $$,
    tests.get_supabase_uid('cloudsync_foreign_owner')
  ),
  array['cus_foreignowner901'::text],
  'The service role durably assigns the canonical Stripe customer'
);

select tests.clear_authentication();
reset role;

update public.profiles
set stripe_customer_id = 'cus_anonymous901'
where id = '00000000-0000-4000-8000-000000000910'::uuid;

select throws_ok(
  $$
    delete from auth.users
    where id = '00000000-0000-4000-8000-000000000910'::uuid
  $$,
  '55000',
  'account durable cleanup is incomplete',
  'A billable user without a workspace cannot bypass durable deletion'
);

update public.profiles
set stripe_customer_id = null
where id = '00000000-0000-4000-8000-000000000910'::uuid;

select tests.authenticate_as_service_role();

select lives_ok(
  format(
    'select * from public.begin_account_deletion(%L::uuid)',
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  'Account deletion captures its E2EE cleanup scope'
);

select ok(
  (
    select bool_and(workspace.deleted_at is not null)
    from public.workspaces as workspace
    cross join account_deletion_e2ee_state as state
    where workspace.owner_user_id = state.owner_user_id
  ),
  'Account deletion tombstones every workspace owned by the deleting user'
);

select ok(
  not exists (
    select 1
    from public.workspace_memberships as membership
    join public.workspaces as workspace
      on workspace.id = membership.workspace_id
    cross join account_deletion_e2ee_state as state
    where (
      membership.user_id = state.owner_user_id
      or workspace.owner_user_id = state.owner_user_id
    )
      and membership.deleted_at is null
  ),
  'Account deletion revokes the deleting user and every member of an owned workspace'
);

select ok(
  (
    select deletion.final_sweep_not_before
      >= clock_timestamp() + interval '24 hours 4 minutes'
    from private.account_deletion_jobs as deletion
    join account_deletion_e2ee_state as state
      on state.owner_user_id = deletion.owner_user_id
  ),
  'The cleanup horizon outlives every one-hour CloudSync token after access is revoked'
);

select ok(
  (
    select deletion.stripe_customer_id = 'cus_deleteowner901'
      and deletion.stripe_deleted_at is null
    from private.account_deletion_jobs as deletion
    join account_deletion_e2ee_state as state
      on state.owner_user_id = deletion.owner_user_id
  ),
  'Account deletion snapshots the Stripe customer before profile cascade'
);

select throws_ok(
  format(
    $sql$
      update public.profiles
      set stripe_customer_id = 'cus_reassigned901'
      where id = %L::uuid
    $sql$,
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  '55000',
  'Stripe customer assignment is unavailable during account deletion',
  'A deleting profile cannot be assigned a different Stripe customer'
);

update account_deletion_e2ee_state as state
set workspace_ids = deletion.e2ee_workspace_ids
from private.account_deletion_jobs as deletion
where deletion.owner_user_id = state.owner_user_id;

select ok(
  (
    select state.workspace_ids = (
      select array_agg(workspace.id order by workspace.id)
      from public.workspaces as workspace
      where workspace.owner_user_id = state.owner_user_id
    )
      and cardinality(state.workspace_ids) = 2
      and not tests.get_supabase_uid('cloudsync_foreign_owner') = any(state.workspace_ids)
    from account_deletion_e2ee_state as state
  ),
  'The cleanup scope includes owned workspaces and excludes foreign memberships'
);

select throws_ok(
  format(
    $sql$
      insert into public.workspaces (id, owner_user_id, kind, name)
      values (
        '00000000-0000-4000-8000-000000000904'::uuid,
        %L::uuid,
        'shared',
        'Too late'
      )
    $sql$,
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  '55000',
  'workspace owner is pending deletion',
  'No new owned workspace can escape the captured cleanup scope'
);

select ok(
  (
    select not retried.was_created
      and deletion.e2ee_workspace_ids = state.workspace_ids
    from account_deletion_e2ee_state as state
    cross join lateral public.begin_account_deletion(state.owner_user_id) as retried
    join private.account_deletion_jobs as deletion
      on deletion.owner_user_id = state.owner_user_id
  ),
  'A retry preserves the original E2EE cleanup scope'
);

select ok(
  (
    select lease.stripe_customer_id = 'cus_deleteowner901'
      and not lease.stripe_deleted
      and not lease.cleanup_ready
      and lease.final_sweep_not_before > clock_timestamp()
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-7000-8000-000000000907'::uuid,
      1,
      300
    ) as lease
  ),
  'A pending Stripe stage can be leased before the destructive cleanup horizon'
);

select throws_ok(
  format(
    $sql$
      select public.mark_account_deletion_stripe_deleted(
        %L::uuid,
        '00000000-0000-7000-8000-000000000907'::uuid,
        'cus_wrong901'
      )
    $sql$,
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  '55000',
  'account deletion Stripe checkpoint is unavailable',
  'The Stripe checkpoint rejects a customer outside the durable snapshot'
);

select ok(
  public.mark_account_deletion_stripe_deleted(
    (select owner_user_id from account_deletion_e2ee_state),
    '00000000-0000-7000-8000-000000000907'::uuid,
    'cus_deleteowner901'
  ),
  'The current worker checkpoints the exact Stripe customer deletion'
);

select ok(
  (
    select deletion.stripe_deleted_at is not null
      and deletion.stripe_customer_id = 'cus_deleteowner901'
    from private.account_deletion_jobs as deletion
    join account_deletion_e2ee_state as state
      on state.owner_user_id = deletion.owner_user_id
  ),
  'The Stripe deletion checkpoint and customer snapshot remain durable'
);

update private.account_deletion_jobs as deletion
set
  requested_at = clock_timestamp() - interval '2 hours',
  final_sweep_not_before = clock_timestamp() - interval '1 hour',
  lease_id = null,
  lease_expires_at = null,
  updated_at = clock_timestamp()
from account_deletion_e2ee_state as state
where deletion.owner_user_id = state.owner_user_id;

select is(
  (
    select count(*)
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-7000-8000-000000000905'::uuid,
      1,
      300
    )
  ),
  1::bigint,
  'The E2EE-aware worker can claim the deletion job'
);

select ok(
  (
    select lease.e2ee_workspace_ids = state.workspace_ids
      and not lease.e2ee_purged
    from public.claim_account_deletion_leases_v2(
      '00000000-0000-7000-8000-000000000906'::uuid,
      1,
      300
    ) as lease
    cross join account_deletion_e2ee_state as state
  ) is null,
  'An active lease fences out a second E2EE cleanup worker'
);

select ok(
  public.mark_account_deletion_prefix_swept(
    (select owner_user_id from account_deletion_e2ee_state),
    '00000000-0000-7000-8000-000000000905'::uuid
  ),
  'The current worker first checkpoints the Storage sweep'
);

select tests.clear_authentication();
reset role;

select throws_ok(
  format(
    'delete from auth.users where id = %L::uuid',
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  '55000',
  'account durable cleanup is incomplete',
  'Auth deletion is blocked before the E2EE checkpoint'
);

select throws_ok(
  format(
    'delete from auth.users where id = %L::uuid',
    tests.get_supabase_uid('cloudsync_untracked_owner')
  ),
  '55000',
  'account durable cleanup is incomplete',
  'A permanent workspace owner cannot bypass the durable deletion job'
);

select lives_ok(
  $$
    delete from auth.users
    where id = '00000000-0000-4000-8000-000000000910'::uuid
  $$,
  'An anonymous user without a workspace can still be deleted directly'
);

select tests.authenticate_as_service_role();

select throws_ok(
  format(
    $sql$
      select public.mark_account_deletion_e2ee_purged(
        %L::uuid,
        '00000000-0000-7000-8000-000000000906'::uuid,
        %L::uuid[]
      )
    $sql$,
    (select owner_user_id from account_deletion_e2ee_state),
    (select workspace_ids from account_deletion_e2ee_state)
  ),
  '55000',
  'account deletion E2EE checkpoint is unavailable',
  'A stale worker cannot checkpoint E2EE cleanup'
);

select ok(
  public.mark_account_deletion_e2ee_purged(
    (select owner_user_id from account_deletion_e2ee_state),
    '00000000-0000-7000-8000-000000000905'::uuid,
    (select workspace_ids from account_deletion_e2ee_state)
  ),
  'The current worker checkpoints the exact purged workspace scope'
);

select ok(
  (
    select deletion.e2ee_purged_at is not null
      and deletion.e2ee_workspace_ids = state.workspace_ids
    from private.account_deletion_jobs as deletion
    join account_deletion_e2ee_state as state
      on state.owner_user_id = deletion.owner_user_id
  ),
  'The E2EE purge checkpoint and scope remain durable'
);

select tests.clear_authentication();
reset role;

select lives_ok(
  format(
    'delete from auth.users where id = %L::uuid',
    (select owner_user_id from account_deletion_e2ee_state)
  ),
  'Auth deletion can proceed after E2EE cleanup is confirmed'
);

select tests.authenticate_as_service_role();

select ok(
  public.finish_account_deletion(
    (select owner_user_id from account_deletion_e2ee_state),
    '00000000-0000-7000-8000-000000000905'::uuid
  ),
  'The worker can finish only after Auth and E2EE deletion'
);

select ok(
  not public.finish_account_deletion(
    (select owner_user_id from account_deletion_e2ee_state),
    '00000000-0000-7000-8000-000000000905'::uuid
  ),
  'Finishing the E2EE-aware deletion job remains idempotent'
);

select lives_ok(
  format(
    'select * from public.begin_account_deletion(%L::uuid)',
    tests.get_supabase_uid('cloudsync_late_customer_owner')
  ),
  'A second deletion job can begin before its customer-created webhook arrives'
);

update private.account_deletion_jobs
set
  stripe_deleted_at = clock_timestamp(),
  lease_id = '00000000-0000-7000-8000-000000000911'::uuid,
  lease_expires_at = clock_timestamp() + interval '5 minutes'
where owner_user_id = tests.get_supabase_uid('cloudsync_late_customer_owner');

select is(
  (
    select assigned_customer_id
    from public.assign_profile_stripe_customer(
      tests.get_supabase_uid('cloudsync_late_customer_owner'),
      'cus_latecustomer901'
    )
  ),
  null::text,
  'A customer-created race is handed off to the durable deletion job'
);

select ok(
  (
    select deletion.stripe_customer_id = 'cus_latecustomer901'
      and deletion.stripe_deleted_at is null
      and deletion.lease_id is null
      and deletion.lease_expires_at is null
    from private.account_deletion_jobs as deletion
    where deletion.owner_user_id = tests.get_supabase_uid(
      'cloudsync_late_customer_owner'
    )
  ),
  'A late customer is adopted into deletion and resets stale Stripe and lease checkpoints'
);

select * from finish();
rollback;
