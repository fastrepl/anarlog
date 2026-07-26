begin;
select plan(1);

select tests.create_supabase_user(
  'deletion_upgrade_owner',
  'deletion-upgrade-owner@example.com'
);

select tests.authenticate_as_service_role();

select *
from public.begin_account_deletion(
  tests.get_supabase_uid('deletion_upgrade_owner')
);

update private.account_deletion_jobs
set
  requested_at = clock_timestamp() - interval '3 hours',
  final_sweep_not_before = clock_timestamp() - interval '2 hours',
  prefix_swept_at = clock_timestamp() - interval '1 hour',
  lease_id = '00000000-0000-7000-8000-000000000919'::uuid,
  lease_expires_at = clock_timestamp() + interval '5 minutes',
  updated_at = clock_timestamp()
where owner_user_id = tests.get_supabase_uid('deletion_upgrade_owner');

do $$
declare
  v_now timestamptz := clock_timestamp();
begin
  update private.account_deletion_jobs as deletion
  set
    final_sweep_not_before = greatest(
      deletion.final_sweep_not_before,
      v_now + interval '24 hours 5 minutes'
    ),
    prefix_swept_at = null,
    lease_id = null,
    lease_expires_at = null,
    updated_at = greatest(deletion.updated_at, v_now);
end;
$$;

select ok(
  (
    select deletion.final_sweep_not_before
        >= clock_timestamp() + interval '24 hours 4 minutes'
      and deletion.prefix_swept_at is null
      and deletion.lease_id is null
      and deletion.lease_expires_at is null
    from private.account_deletion_jobs as deletion
    where deletion.owner_user_id = tests.get_supabase_uid(
      'deletion_upgrade_owner'
    )
  ),
  'The E2EE migration upgrade invalidates stale prefix and lease checkpoints'
);

select * from finish();
rollback;
