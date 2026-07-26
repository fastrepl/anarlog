begin;
select plan(23);

select tests.create_supabase_user('anchor_owner', 'anchor-owner@example.com');
select tests.create_supabase_user('anchor_commenter', 'anchor-commenter@example.com');
select tests.create_supabase_user('anchor_viewer', 'anchor-viewer@example.com');

create temporary table session_share_comment_anchor_test_state (
  name text primary key,
  workspace_id uuid,
  share_id uuid,
  entity_id uuid
);

grant all on session_share_comment_anchor_test_state
  to anon, authenticated, service_role;

insert into session_share_comment_anchor_test_state (
  name,
  workspace_id,
  share_id
) values (
  'share',
  gen_random_uuid(),
  gen_random_uuid()
);

update auth.users
set email_confirmed_at = now()
where id in (
  tests.get_supabase_uid('anchor_owner'),
  tests.get_supabase_uid('anchor_commenter'),
  tests.get_supabase_uid('anchor_viewer')
);

select tests.authenticate_as_service_role();

insert into public.workspaces (id, owner_user_id, kind, name)
select
  workspace_id,
  tests.get_supabase_uid('anchor_owner'),
  'shared',
  'Comment anchor workspace'
from session_share_comment_anchor_test_state
where name = 'share';

insert into public.workspace_memberships (workspace_id, user_id, role)
select
  workspace_id,
  tests.get_supabase_uid('anchor_owner'),
  'owner'
from session_share_comment_anchor_test_state
where name = 'share';

insert into public.session_shares (
  id,
  workspace_id,
  session_id,
  created_by_user_id
)
select
  share_id,
  workspace_id,
  'comment-anchor-session',
  tests.get_supabase_uid('anchor_owner')
from session_share_comment_anchor_test_state
where name = 'share';

insert into public.session_share_snapshots (
  share_id,
  content_revision,
  title,
  body_json,
  published_by_user_id
)
select
  share_id,
  7,
  'Comment anchor fixture',
  '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb,
  tests.get_supabase_uid('anchor_owner')
from session_share_comment_anchor_test_state
where name = 'share';

insert into public.session_access_grants (
  share_id,
  grantee_user_id,
  capability,
  granted_by_user_id
)
select
  share_id,
  tests.get_supabase_uid('anchor_commenter'),
  'commenter',
  tests.get_supabase_uid('anchor_owner')
from session_share_comment_anchor_test_state
where name = 'share'
union all
select
  share_id,
  tests.get_supabase_uid('anchor_viewer'),
  'viewer',
  tests.get_supabase_uid('anchor_owner')
from session_share_comment_anchor_test_state
where name = 'share';

select tests.clear_authentication();
reset role;

select ok(
  (
    select count(*) = 5
    from information_schema.columns
    where table_schema = 'public'
      and table_name = 'session_share_comments'
      and column_name in (
        'anchor_quote_exact',
        'anchor_quote_prefix',
        'anchor_quote_suffix',
        'anchor_from_hint',
        'anchor_to_hint'
      )
  ),
  'Shared comments carry the five anchor columns'
);

select ok(
  exists (
    select 1
    from pg_constraint
    where conrelid = 'public.session_share_comments'::regclass
      and conname = 'session_share_comments_anchor_check'
  ),
  'Shared comment anchors are constrained all-or-nothing with bounded quotes'
);

select ok(
  (
    select count(*) = 2
      and bool_and(not proc.prosecdef)
      and bool_and(
        'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share_comment',
        'list_session_share_comments'
      )
  ),
  'Recreated public comment wrappers stay hardened security invokers'
);

select ok(
  (
    select count(*) = 2
      and bool_and(proc.prosecdef)
      and bool_and(
        'search_path=""' = any(coalesce(proc.proconfig, array[]::text[]))
      )
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'private'
      and proc.proname in (
        'create_session_share_comment',
        'list_session_share_comments'
      )
  ),
  'Recreated private comment implementations stay hardened security definers'
);

select ok(
  (
    select count(*) = 2
      and bool_and(has_function_privilege('authenticated', proc.oid, 'EXECUTE'))
      and bool_and(not has_function_privilege('anon', proc.oid, 'EXECUTE'))
    from pg_proc as proc
    join pg_namespace as namespace
      on namespace.oid = proc.pronamespace
    where namespace.nspname = 'public'
      and proc.proname in (
        'create_session_share_comment',
        'list_session_share_comments'
      )
  ),
  'Recreated comment wrappers keep authenticated-only execute grants'
);

select tests.authenticate_as('anchor_commenter');

select lives_ok(
  $$
    insert into session_share_comment_anchor_test_state (name, entity_id)
    select
      'anchored_comment',
      comment_id
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      '  Anchored comment  ',
      'quoted fixture text',
      '',
      'trailing context',
      2,
      9
    )
  $$,
  'A commenter can create an anchored comment with an empty prefix'
);

select tests.authenticate_as_service_role();

select ok(
  (
    select
      comment.body = 'Anchored comment'
      and comment.snapshot_content_revision = 7
      and comment.anchor_quote_exact = 'quoted fixture text'
      and comment.anchor_quote_prefix = ''
      and comment.anchor_quote_suffix = 'trailing context'
      and comment.anchor_from_hint = 2
      and comment.anchor_to_hint = 9
    from public.session_share_comments as comment
    where comment.id = (
      select entity_id
      from session_share_comment_anchor_test_state
      where name = 'anchored_comment'
    )
  ),
  'The stored anchored comment keeps the verbatim quote and hints'
);

select tests.authenticate_as('anchor_commenter');

select lives_ok(
  $$
    insert into session_share_comment_anchor_test_state (name, entity_id)
    select
      'legacy_comment',
      comment_id
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'Legacy comment'
    )
  $$,
  'The legacy two-argument create call still works'
);

select tests.authenticate_as_service_role();

select ok(
  (
    select
      comment.anchor_quote_exact is null
      and comment.anchor_quote_prefix is null
      and comment.anchor_quote_suffix is null
      and comment.anchor_from_hint is null
      and comment.anchor_to_hint is null
    from public.session_share_comments as comment
    where comment.id = (
      select entity_id
      from session_share_comment_anchor_test_state
      where name = 'legacy_comment'
    )
  ),
  'Legacy comments stay unanchored'
);

select tests.authenticate_as('anchor_commenter');

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      '',
      '',
      ''
    )
  $$,
  '22023',
  'invalid session comment',
  'An empty exact quote is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      repeat('x', 4097),
      '',
      ''
    )
  $$,
  '22023',
  'invalid session comment',
  'An oversized exact quote is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      repeat('p', 257),
      ''
    )
  $$,
  '22023',
  'invalid session comment',
  'An oversized quote prefix is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      '',
      repeat('s', 257)
    )
  $$,
  '22023',
  'invalid session comment',
  'An oversized quote suffix is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      'prefix'
    )
  $$,
  '22023',
  'invalid session comment',
  'A partial quote trio is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      null,
      null,
      null,
      1,
      5
    )
  $$,
  '22023',
  'invalid session comment',
  'Position hints without quotes are rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      '',
      '',
      0,
      5
    )
  $$,
  '22023',
  'invalid session comment',
  'A zero from hint is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      '',
      '',
      5,
      5
    )
  $$,
  '22023',
  'invalid session comment',
  'A collapsed hint range is rejected'
);

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'rejection probe',
      'quote',
      '',
      '',
      5,
      null
    )
  $$,
  '22023',
  'invalid session comment',
  'An unpaired from hint is rejected'
);

select tests.authenticate_as('anchor_viewer');

select throws_ok(
  $$
    select *
    from public.create_session_share_comment(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      ),
      'viewer comment',
      'quote',
      '',
      ''
    )
  $$,
  '42501',
  'session comment operation not permitted',
  'A viewer still cannot create comments, anchored or not'
);

select ok(
  (
    select
      count(*) = 2
      and count(*) filter (where result.anchor_quote_exact is not null) = 1
    from public.list_session_share_comments(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      )
    ) as result
  ),
  'A viewer lists anchored and unanchored comments together'
);

select tests.authenticate_as('anchor_commenter');

select lives_ok(
  $$
    select *
    from public.delete_session_share_comment(
      (
        select entity_id
        from session_share_comment_anchor_test_state
        where name = 'anchored_comment'
      )
    )
  $$,
  'The author can delete their anchored comment'
);

select tests.authenticate_as_service_role();

select ok(
  (
    select
      comment.body = ''
      and comment.deleted_at is not null
      and comment.anchor_quote_exact is null
      and comment.anchor_quote_prefix is null
      and comment.anchor_quote_suffix is null
      and comment.anchor_from_hint is null
      and comment.anchor_to_hint is null
    from public.session_share_comments as comment
    where comment.id = (
      select entity_id
      from session_share_comment_anchor_test_state
      where name = 'anchored_comment'
    )
  ),
  'Deleting an anchored comment scrubs the quote and hints'
);

select tests.authenticate_as('anchor_viewer');

select ok(
  (
    select
      count(*) = 1
      and bool_and(result.anchor_quote_exact is null)
    from public.list_session_share_comments(
      (
        select share_id
        from session_share_comment_anchor_test_state
        where name = 'share'
      )
    ) as result
  ),
  'Deleted anchored comments drop out of the list'
);

select finish();
rollback;
