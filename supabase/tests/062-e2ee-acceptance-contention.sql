create extension if not exists dblink with schema extensions;
-- Loopback uses trust auth; use the local container address so dblink authenticates.
select extensions.dblink_connect('replica_lock', format(
  'hostaddr=%s user=postgres password=postgres dbname=%s', inet_server_addr(), current_database()));
create temporary table contention_owner as
select * from extensions.dblink('replica_lock',
  $$insert into auth.users(id, email, created_at, updated_at)
    select id, 'replica-contention-' || id || '@example.com', now(), now() from (select gen_random_uuid() as id) fixture returning id$$
) as owner(id uuid);
select key_id from public.claim_personal_workspace_e2ee_key(
  (select id from contention_owner), 'abcdefghijklmnopqrstuv');
create temporary table contention_input as
select jsonb_build_array(jsonb_build_object('record_id', repeat('r', 43), 'payload', payload,
  'payload_hash', rtrim(translate(encode(extensions.digest(payload, 'sha256'), 'base64'), '+/', '-_'), '='))) as events
from (select '{"version":1,"key_id":"abcdefghijklmnopqrstuv","nonce":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ciphertext":"opaque"}'::text as payload) source;
create temporary table contention_receipt as
select * from public.accept_e2ee_replica_batch((select id from contention_owner), (select id from contention_owner),
  '10000000-0000-4000-8000-000000000001', 0, false, (select events from contention_input));
select extensions.dblink_exec('replica_lock', 'BEGIN');
select * from extensions.dblink('replica_lock', format(
  'select id from public.workspaces where id = %L for update', (select id from contention_owner)
)) as locked(id uuid);

begin;
set local statement_timeout = '500ms';
select plan(4);
select throws_ok(
  $$select * from public.accept_e2ee_replica_batch((select id from contention_owner), (select id from contention_owner),
    '10000000-0000-4000-8000-000000000002', (select head_sequence from contention_receipt), false, (select events from contention_input))$$,
  '55P03', null, 'A concurrent acceptance fails immediately instead of occupying a waiting connection');
select throws_ok(
  $$select * from public.accept_e2ee_replica_batch((select id from contention_owner), (select id from contention_owner),
    '10000000-0000-4000-8000-000000000002', 0, false, (select events from contention_input))$$,
  '40001', 'Replica base changed; pull before retrying', 'A new stale mutation is rejected without waiting for the workspace lock');
select extensions.dblink_exec('replica_lock', 'ROLLBACK');
select extensions.dblink_exec('replica_lock', 'BEGIN');
select * from extensions.dblink('replica_lock',
  'select pg_advisory_xact_lock(17469, slot)::text from generate_series(1,4) slot') as budget(lock text);
select throws_ok(
  $$select * from public.accept_e2ee_replica_batch((select id from contention_owner), (select id from contention_owner),
    '10000000-0000-4000-8000-000000000002', (select head_sequence from contention_receipt), false, (select events from contention_input))$$,
  '55P03', 'Encrypted sync is busy; retry later', 'The database budget applies across separate service connections');
select extensions.dblink_exec('replica_lock', 'ROLLBACK');
select results_eq(
  $$select * from public.accept_e2ee_replica_batch((select id from contention_owner), (select id from contention_owner),
    '10000000-0000-4000-8000-000000000001', 0, false, (select events from contention_input))$$,
  $$select * from contention_receipt$$, 'A receipt with an old base still replays after contention clears');
select * from finish();
rollback;

select extensions.dblink_exec('replica_lock', format(
  'delete from public.workspaces where id = %L and owner_user_id = %L; delete from auth.users where id = %L',
  (select id from contention_owner), (select id from contention_owner), (select id from contention_owner)));
select extensions.dblink_disconnect('replica_lock');
