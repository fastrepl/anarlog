begin;
select plan(10);
select tests.create_supabase_user('replica_owner', 'replica-owner@example.com');
select tests.create_supabase_user('replica_other', 'replica-other@example.com');
select tests.authenticate_as_service_role();
select key_id from public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('replica_owner'), 'abcdefghijklmnopqrstuv');
create temporary table replica_input as
select jsonb_build_array(jsonb_build_object('record_id', repeat('r',43), 'payload', payload,
  'payload_hash', rtrim(translate(encode(extensions.digest(payload,'sha256'),'base64'),'+/','-_'),'='))) as events
from (select '{"version":1,"key_id":"abcdefghijklmnopqrstuv","nonce":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ciphertext":"opaque"}'::text as payload) source;
create temporary table first_receipt as
select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'),
 '10000000-0000-4000-8000-000000000001', 0, false, (select events from replica_input));
select ok((select head_sequence > 0 and cloud_authority_after = 0 and jsonb_array_length(receipts) = 1 from first_receipt), 'Acceptance sets cloud authority and returns exact receipts');
select results_eq(
 $$select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), '10000000-0000-4000-8000-000000000001', 0, false, (select events from replica_input))$$,
 $$select * from first_receipt$$, 'A lost response replays its immutable receipt');
select throws_ok(
 $$select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), '10000000-0000-4000-8000-000000000002', 0, false, (select events from replica_input))$$,
 '40001','Replica base changed; pull before retrying','A stale writer cannot append');
select throws_ok(
 $$select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), '10000000-0000-4000-8000-000000000001', 1, false, (select events from replica_input))$$,
 '22023','Mutation identity was reused','Restored counters cannot reuse an identity for a different mutation');
select throws_ok(
 $$select * from public.publish_e2ee_freshness_events(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), false, (select events from replica_input))$$,
 'A0002','This workspace requires a cloud-authoritative sync client','Legacy writes are fenced after cutover');
select throws_ok(
 $$select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_other'), tests.get_supabase_uid('replica_owner'), '10000000-0000-4000-8000-000000000001', 0, false, (select events from replica_input))$$,
 '42501','Replica access denied','Receipts cannot bypass workspace authorization');
create temporary table second_receipt as
select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'),
 '10000000-0000-4000-8000-000000000002', (select head_sequence from first_receipt), false, (select events from replica_input));
select ok((select second_receipt.head_sequence > first_receipt.head_sequence from second_receipt, first_receipt), 'An intentional replay of ciphertext gets a new accepted position');
select results_eq(
 $$select * from public.accept_e2ee_replica_batch(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), '10000000-0000-4000-8000-000000000001', 0, false, (select events from replica_input))$$,
 $$select * from first_receipt$$, 'A delayed retry cannot overwrite a later accepted state');
select ok(not has_table_privilege('authenticated','public.e2ee_replica_receipts','SELECT')
  and not has_function_privilege('authenticated','public.accept_e2ee_replica_batch(uuid,uuid,uuid,bigint,boolean,jsonb)','EXECUTE'), 'Only trusted service code can issue or inspect receipts');
select throws_ok(
 $$select * from public.read_e2ee_freshness_page_v2(tests.get_supabase_uid('replica_owner'), tests.get_supabase_uid('replica_owner'), 0, null, 64, 50331648)$$,
 'A0002','This workspace requires a cloud-authoritative sync client','Legacy receivers cannot retire pending edits after cutover');
select * from finish();
rollback;
