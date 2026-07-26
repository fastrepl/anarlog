begin;
select plan(4);

select tests.create_supabase_user('owner', 'owner@example.com');
select tests.create_supabase_user('other', 'other@example.com');

select tests.authenticate_as('owner');

select lives_ok(
  $$insert into storage.objects (bucket_id, name, owner)
    values ('audio-files', auth.uid()::text || '/test.wav', auth.uid())$$,
  'Owner can upload to own folder'
);

select results_eq(
  $$select count(*) from storage.objects where bucket_id = 'audio-files'$$,
  array[1::bigint],
  'Owner can view own files'
);

select tests.clear_authentication();
select tests.authenticate_as('other');

select throws_ok(
  $$insert into storage.objects (bucket_id, name, owner)
    values ('audio-files', tests.get_supabase_uid('owner')::text || '/hack.wav', auth.uid())$$,
  '42501',
  null,
  'Cannot upload to another user folder'
);

select results_eq(
  $$select count(*) from storage.objects where bucket_id = 'audio-files'$$,
  array[0::bigint],
  'Other user cannot view files in private bucket'
);

select * from finish();
rollback;
