-- Anchored comments: store a W3C-style text quote (exact + prefix + suffix)
-- plus from/to position hints on session share comments so clients can
-- highlight the commented range. Hints are valid at the row's
-- snapshot_content_revision; the quote is the source of truth and renderers
-- must verify hints against it before trusting them (the snapshot may have
-- been republished between capture and insert).

ALTER TABLE public.session_share_comments
  ADD COLUMN anchor_quote_exact text,
  ADD COLUMN anchor_quote_prefix text,
  ADD COLUMN anchor_quote_suffix text,
  ADD COLUMN anchor_from_hint bigint,
  ADD COLUMN anchor_to_hint bigint;

ALTER TABLE public.session_share_comments
  ADD CONSTRAINT session_share_comments_anchor_check CHECK (
    (
      anchor_quote_exact IS NULL
      AND anchor_quote_prefix IS NULL
      AND anchor_quote_suffix IS NULL
      AND anchor_from_hint IS NULL
      AND anchor_to_hint IS NULL
    )
    OR (
      deleted_at IS NULL
      AND anchor_quote_exact IS NOT NULL
      AND anchor_quote_prefix IS NOT NULL
      AND anchor_quote_suffix IS NOT NULL
      AND anchor_quote_exact <> ''
      AND octet_length(anchor_quote_exact) <= 4096
      AND octet_length(anchor_quote_prefix) <= 256
      AND octet_length(anchor_quote_suffix) <= 256
      AND (anchor_from_hint IS NULL) = (anchor_to_hint IS NULL)
      AND (
        anchor_from_hint IS NULL
        OR (anchor_from_hint > 0 AND anchor_to_hint > anchor_from_hint)
      )
    )
  );

-- Adding defaulted parameters or extending RETURNS TABLE cannot be done with
-- CREATE OR REPLACE (ambiguous overload 42725 / return-type change 42P13), so
-- the create/list functions are dropped and recreated under the same names.
DROP FUNCTION public.create_session_share_comment(uuid, text);
DROP FUNCTION private.create_session_share_comment(uuid, text);
DROP FUNCTION public.list_session_share_comments(uuid, timestamptz, uuid, integer);
DROP FUNCTION private.list_session_share_comments(uuid, timestamptz, uuid, integer);

CREATE FUNCTION private.create_session_share_comment(
  p_share_id uuid,
  p_body text,
  p_anchor_quote_exact text DEFAULT NULL,
  p_anchor_quote_prefix text DEFAULT NULL,
  p_anchor_quote_suffix text DEFAULT NULL,
  p_anchor_from_hint bigint DEFAULT NULL,
  p_anchor_to_hint bigint DEFAULT NULL
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
  anchor_quote_exact text,
  anchor_quote_prefix text,
  anchor_quote_suffix text,
  anchor_from_hint bigint,
  anchor_to_hint bigint,
  created_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_body text := btrim(p_body, E' \t\n\r\v\f');
  v_capability text;
  v_manage_access boolean;
  v_revision bigint;
  v_comment public.session_share_comments%ROWTYPE;
BEGIN
  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT access.capability, access.manage_access
  INTO v_capability, v_manage_access
  FROM private.resolve_my_session_access(p_share_id) AS access;

  IF COALESCE(private.session_capability_rank(v_capability), 0) < 2
    AND NOT COALESCE(v_manage_access, false)
  THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_body IS NULL
    OR v_body = ''
    OR octet_length(v_body) > 16384
  THEN
    RAISE EXCEPTION 'invalid session comment'
      USING ERRCODE = '22023';
  END IF;

  IF (p_anchor_quote_exact IS NULL) <> (p_anchor_quote_prefix IS NULL)
    OR (p_anchor_quote_exact IS NULL) <> (p_anchor_quote_suffix IS NULL)
  THEN
    RAISE EXCEPTION 'invalid session comment'
      USING ERRCODE = '22023';
  END IF;

  IF p_anchor_quote_exact IS NOT NULL AND (
    p_anchor_quote_exact = ''
    OR octet_length(p_anchor_quote_exact) > 4096
    OR octet_length(p_anchor_quote_prefix) > 256
    OR octet_length(p_anchor_quote_suffix) > 256
  )
  THEN
    RAISE EXCEPTION 'invalid session comment'
      USING ERRCODE = '22023';
  END IF;

  IF (p_anchor_from_hint IS NULL) <> (p_anchor_to_hint IS NULL)
    OR (p_anchor_from_hint IS NOT NULL AND (
      p_anchor_quote_exact IS NULL
      OR p_anchor_from_hint < 1
      OR p_anchor_to_hint <= p_anchor_from_hint
    ))
  THEN
    RAISE EXCEPTION 'invalid session comment'
      USING ERRCODE = '22023';
  END IF;

  SELECT snapshot.content_revision
  INTO v_revision
  FROM public.session_share_snapshots AS snapshot
  WHERE snapshot.share_id = p_share_id;

  IF v_revision IS NULL THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  INSERT INTO public.session_share_comments (
    share_id,
    author_user_id,
    snapshot_content_revision,
    body,
    anchor_quote_exact,
    anchor_quote_prefix,
    anchor_quote_suffix,
    anchor_from_hint,
    anchor_to_hint
  ) VALUES (
    p_share_id,
    v_actor_id,
    v_revision,
    v_body,
    p_anchor_quote_exact,
    p_anchor_quote_prefix,
    p_anchor_quote_suffix,
    p_anchor_from_hint,
    p_anchor_to_hint
  )
  RETURNING * INTO v_comment;

  PERFORM private.write_session_access_event(
    p_share_id,
    'comment_created',
    v_actor_id,
    v_actor_id,
    v_comment.id,
    NULL,
    v_revision::text
  );

  RETURN QUERY
  SELECT
    v_comment.id,
    true,
    v_comment.snapshot_content_revision,
    v_comment.body,
    v_comment.anchor_quote_exact,
    v_comment.anchor_quote_prefix,
    v_comment.anchor_quote_suffix,
    v_comment.anchor_from_hint,
    v_comment.anchor_to_hint,
    v_comment.created_at;
END;
$$;

CREATE FUNCTION private.list_session_share_comments(
  p_share_id uuid,
  p_before_created_at timestamptz DEFAULT NULL,
  p_before_comment_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
  anchor_quote_exact text,
  anchor_quote_prefix text,
  anchor_quote_suffix text,
  anchor_from_hint bigint,
  anchor_to_hint bigint,
  created_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_limit integer := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 101);
BEGIN
  IF (p_before_created_at IS NULL) <> (p_before_comment_id IS NULL) THEN
    RAISE EXCEPTION 'invalid session comment cursor'
      USING ERRCODE = '22023';
  END IF;

  IF NOT private.is_session_share_manager(p_share_id, v_actor_id)
    AND NOT EXISTS (
      SELECT 1
      FROM public.session_access_grants AS access_grant
      JOIN public.session_shares AS share
        ON share.id = access_grant.share_id
      JOIN public.workspaces AS workspace
        ON workspace.id = share.workspace_id
      WHERE access_grant.share_id = p_share_id
        AND access_grant.grantee_user_id = v_actor_id
        AND access_grant.capability IN ('viewer', 'commenter', 'editor')
        AND access_grant.revoked_at IS NULL
        AND share.deleted_at IS NULL
        AND workspace.deleted_at IS NULL
    )
  THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    comment.id,
    comment.author_user_id = v_actor_id,
    comment.snapshot_content_revision,
    comment.body,
    comment.anchor_quote_exact,
    comment.anchor_quote_prefix,
    comment.anchor_quote_suffix,
    comment.anchor_from_hint,
    comment.anchor_to_hint,
    comment.created_at
  FROM public.session_share_comments AS comment
  WHERE comment.share_id = p_share_id
    AND comment.deleted_at IS NULL
    AND (
      p_before_created_at IS NULL
      OR (comment.created_at, comment.id)
        < (p_before_created_at, p_before_comment_id)
    )
  ORDER BY comment.created_at DESC, comment.id DESC
  LIMIT v_limit;
END;
$$;

-- Soft delete scrubs the anchor alongside the body: anchors quote note
-- content, so a deleted comment must not retain it.
CREATE OR REPLACE FUNCTION private.delete_session_share_comment(
  p_comment_id uuid
)
RETURNS TABLE (
  comment_id uuid,
  deleted_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_share_id uuid;
  v_comment public.session_share_comments%ROWTYPE;
  v_deleted_at timestamptz;
BEGIN
  SELECT comment.share_id
  INTO v_share_id
  FROM public.session_share_comments AS comment
  WHERE comment.id = p_comment_id;

  IF v_share_id IS NULL THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = v_share_id
  FOR UPDATE;

  SELECT comment.*
  INTO v_comment
  FROM public.session_share_comments AS comment
  WHERE comment.id = p_comment_id
    AND comment.share_id = v_share_id
  FOR UPDATE;

  IF NOT FOUND
    OR (
      v_comment.author_user_id <> v_actor_id
      AND NOT private.is_session_share_manager(v_share_id, v_actor_id)
    )
  THEN
    RAISE EXCEPTION 'session comment operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_comment.deleted_at IS NULL THEN
    v_deleted_at := now();

    UPDATE public.session_share_comments
    SET
      body = '',
      anchor_quote_exact = NULL,
      anchor_quote_prefix = NULL,
      anchor_quote_suffix = NULL,
      anchor_from_hint = NULL,
      anchor_to_hint = NULL,
      deleted_at = v_deleted_at,
      deleted_by_user_id = v_actor_id
    WHERE id = v_comment.id;

    PERFORM private.write_session_access_event(
      v_share_id,
      'comment_deleted',
      v_actor_id,
      v_comment.author_user_id,
      v_comment.id,
      v_comment.snapshot_content_revision::text,
      NULL
    );
  ELSE
    v_deleted_at := v_comment.deleted_at;
  END IF;

  RETURN QUERY SELECT v_comment.id, v_deleted_at;
END;
$$;

CREATE FUNCTION public.create_session_share_comment(
  p_share_id uuid,
  p_body text,
  p_anchor_quote_exact text DEFAULT NULL,
  p_anchor_quote_prefix text DEFAULT NULL,
  p_anchor_quote_suffix text DEFAULT NULL,
  p_anchor_from_hint bigint DEFAULT NULL,
  p_anchor_to_hint bigint DEFAULT NULL
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
  anchor_quote_exact text,
  anchor_quote_prefix text,
  anchor_quote_suffix text,
  anchor_from_hint bigint,
  anchor_to_hint bigint,
  created_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.create_session_share_comment(
    p_share_id,
    p_body,
    p_anchor_quote_exact,
    p_anchor_quote_prefix,
    p_anchor_quote_suffix,
    p_anchor_from_hint,
    p_anchor_to_hint
  );
$$;

CREATE FUNCTION public.list_session_share_comments(
  p_share_id uuid,
  p_before_created_at timestamptz DEFAULT NULL,
  p_before_comment_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
  anchor_quote_exact text,
  anchor_quote_prefix text,
  anchor_quote_suffix text,
  anchor_from_hint bigint,
  anchor_to_hint bigint,
  created_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_session_share_comments(
    p_share_id,
    p_before_created_at,
    p_before_comment_id,
    p_limit
  );
$$;

-- Dropped functions lose their ACLs and new functions default to EXECUTE for
-- PUBLIC, so the grants must be re-established explicitly.
REVOKE ALL ON FUNCTION private.create_session_share_comment(
  uuid, text, text, text, text, bigint, bigint
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_session_share_comments(
  uuid, timestamptz, uuid, integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.create_session_share_comment(
  uuid, text, text, text, text, bigint, bigint
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_session_share_comments(
  uuid, timestamptz, uuid, integer
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.create_session_share_comment(
  uuid, text, text, text, text, bigint, bigint
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_session_share_comments(
  uuid, timestamptz, uuid, integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.create_session_share_comment(
  uuid, text, text, text, text, bigint, bigint
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_session_share_comments(
  uuid, timestamptz, uuid, integer
) TO authenticated;
