CREATE TABLE public.session_share_comments (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  author_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  snapshot_content_revision bigint NOT NULL,
  body text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  deleted_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  CONSTRAINT session_share_comments_revision_check CHECK (
    snapshot_content_revision > 0
  ),
  CONSTRAINT session_share_comments_body_check CHECK (
    (
      deleted_at IS NULL
      AND deleted_by_user_id IS NULL
      AND body = btrim(body, E' \t\n\r\v\f')
      AND body <> ''
      AND octet_length(body) <= 16384
    )
    OR (
      deleted_at IS NOT NULL
      AND body = ''
    )
  )
);

CREATE INDEX session_share_comments_active_feed_idx
  ON public.session_share_comments(share_id, created_at, id)
  WHERE deleted_at IS NULL;

CREATE INDEX session_access_requests_history_idx
  ON public.session_access_requests(
    share_id,
    requester_user_id,
    updated_at DESC
  );

CREATE INDEX session_access_events_subject_created_idx
  ON public.session_access_events(subject_user_id, created_at DESC, id DESC)
  WHERE event_type IN (
    'request_approved',
    'request_denied',
    'grant_changed',
    'grant_revoked'
  );

ALTER TABLE public.session_share_comments ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_share_comments
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.session_share_comments TO service_role;

CREATE POLICY session_share_comments_service_all
  ON public.session_share_comments
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

ALTER TABLE public.session_access_events
  DROP CONSTRAINT session_access_events_type_check,
  ADD CONSTRAINT session_access_events_type_check CHECK (
    event_type IN (
      'share_created',
      'share_reactivated',
      'share_deleted',
      'scope_changed',
      'link_enabled',
      'link_rotated',
      'invitation_created',
      'invitation_resent',
      'invitation_revoked',
      'invitation_accepted',
      'grant_changed',
      'grant_revoked',
      'request_created',
      'request_cancelled',
      'request_approved',
      'request_denied',
      'snapshot_published',
      'comment_created',
      'comment_deleted'
    )
  );

CREATE OR REPLACE FUNCTION private.create_session_share_comment(
  p_share_id uuid,
  p_body text
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
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
    body
  ) VALUES (
    p_share_id,
    v_actor_id,
    v_revision,
    v_body
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
    v_comment.created_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_session_share_comments(
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

CREATE OR REPLACE FUNCTION private.get_my_session_access_request(
  p_share_id uuid
)
RETURNS TABLE (
  request_id uuid,
  requested_capability text,
  status text,
  created_at timestamptz,
  reviewed_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
BEGIN
  RETURN QUERY
  SELECT
    request.id,
    request.requested_capability,
    request.status,
    request.created_at,
    request.reviewed_at
  FROM (
    SELECT latest.*
    FROM public.session_access_requests AS latest
    WHERE latest.share_id = p_share_id
      AND latest.requester_user_id = v_actor_id
    ORDER BY latest.created_at DESC, latest.id DESC
    LIMIT 1
  ) AS request
  WHERE request.status <> 'approved'
    OR EXISTS (
      SELECT 1
      FROM public.session_access_grants AS access_grant
      WHERE access_grant.share_id = request.share_id
        AND access_grant.grantee_user_id = v_actor_id
        AND access_grant.revoked_at IS NULL
        AND private.session_capability_rank(access_grant.capability)
          >= private.session_capability_rank(request.requested_capability)
    );
END;
$$;

CREATE OR REPLACE FUNCTION private.request_session_access(
  p_share_id uuid,
  p_requested_capability text
)
RETURNS TABLE (
  request_id uuid,
  requested_capability text,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_request public.session_access_requests%ROWTYPE;
  v_existing_capability text;
  v_manage_access boolean;
BEGIN
  IF private.session_capability_rank(p_requested_capability) = 0 OR NOT EXISTS (
    SELECT 1
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    WHERE share.id = p_share_id
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT access.capability, access.manage_access
  INTO v_existing_capability, v_manage_access
  FROM private.resolve_my_session_access(p_share_id) AS access;

  IF COALESCE(v_manage_access, false)
    OR private.session_capability_rank(v_existing_capability)
      >= private.session_capability_rank(p_requested_capability)
  THEN
    RAISE EXCEPTION 'session access request not needed'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_request.*
  INTO v_request
  FROM public.session_access_requests AS access_request
  WHERE access_request.share_id = p_share_id
    AND access_request.requester_user_id = v_actor_id
    AND access_request.status = 'pending';

  IF FOUND THEN
    RETURN QUERY
    SELECT v_request.id, v_request.requested_capability, false;
    RETURN;
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public.session_access_requests AS recent_request
    WHERE recent_request.share_id = p_share_id
      AND recent_request.requester_user_id = v_actor_id
      AND recent_request.status = 'cancelled'
      AND recent_request.updated_at > now() - interval '15 minutes'
  ) THEN
    RAISE EXCEPTION 'session access request is rate limited'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.session_access_requests (
    share_id,
    requester_user_id,
    requested_capability
  ) VALUES (
    p_share_id,
    v_actor_id,
    p_requested_capability
  )
  RETURNING * INTO v_request;

  PERFORM private.write_session_access_event(
    p_share_id,
    'request_created',
    v_actor_id,
    v_actor_id,
    v_request.id,
    NULL,
    p_requested_capability
  );

  RETURN QUERY
  SELECT v_request.id, v_request.requested_capability, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_session_share_access_page(
  p_share_id uuid,
  p_before_created_at timestamptz DEFAULT NULL,
  p_before_entry_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  entry_type text,
  entry_id uuid,
  user_id uuid,
  user_email text,
  capability text,
  status text,
  created_at timestamptz,
  expires_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_limit integer := LEAST(GREATEST(COALESCE(p_limit, 100), 1), 1000);
BEGIN
  IF (p_before_created_at IS NULL) <> (p_before_entry_id IS NULL) THEN
    RAISE EXCEPTION 'invalid session access cursor'
      USING ERRCODE = '22023';
  END IF;

  IF NOT private.is_session_share_manager(p_share_id, v_actor_id) THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  WITH access_entries AS (
    SELECT
      'grant'::text AS entry_type,
      access_grant.id AS entry_id,
      access_grant.grantee_user_id AS user_id,
      lower(btrim(grantee.email)) AS user_email,
      access_grant.capability,
      'active'::text AS status,
      access_grant.created_at,
      NULL::timestamptz AS expires_at
    FROM public.session_access_grants AS access_grant
    JOIN auth.users AS grantee
      ON grantee.id = access_grant.grantee_user_id
    WHERE access_grant.share_id = p_share_id
      AND access_grant.revoked_at IS NULL

    UNION ALL

    SELECT
      'invitation'::text,
      invitation.id,
      invitation.invitee_user_id,
      invitation.invitee_email,
      invitation.capability,
      'pending'::text,
      invitation.created_at,
      invitation.expires_at
    FROM public.session_access_invitations AS invitation
    WHERE invitation.share_id = p_share_id
      AND invitation.accepted_at IS NULL
      AND invitation.revoked_at IS NULL
      AND invitation.expires_at > now()

    UNION ALL

    SELECT
      'request'::text,
      access_request.id,
      access_request.requester_user_id,
      lower(btrim(requester.email)),
      access_request.requested_capability,
      access_request.status,
      access_request.created_at,
      NULL::timestamptz
    FROM public.session_access_requests AS access_request
    JOIN auth.users AS requester
      ON requester.id = access_request.requester_user_id
    WHERE access_request.share_id = p_share_id
      AND access_request.status = 'pending'
      AND requester.email IS NOT NULL
  )
  SELECT access_entry.*
  FROM access_entries AS access_entry
  WHERE p_before_created_at IS NULL
    OR (access_entry.created_at, access_entry.entry_id)
      < (p_before_created_at, p_before_entry_id)
  ORDER BY access_entry.created_at DESC, access_entry.entry_id DESC
  LIMIT v_limit;
END;
$$;

CREATE OR REPLACE FUNCTION private.inspect_my_session_access_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  status text,
  capability text,
  share_id uuid
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_invitation public.session_access_invitations%ROWTYPE;
  v_status text;
  v_share_id uuid;
BEGIN
  IF p_invite_token IS NULL
    OR p_invite_token !~ '^[A-Za-z0-9_-]{43}$'
  THEN
    RETURN;
  END IF;

  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id;

  SELECT invitation.*
  INTO v_invitation
  FROM public.session_access_invitations AS invitation
  JOIN public.session_shares AS share
    ON share.id = invitation.share_id
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE invitation.id = p_invitation_id
    AND invitation.token_hash = extensions.digest(p_invite_token, 'sha256')
    AND invitation.invitee_email = v_actor_email
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND (
      invitation.invitee_user_id IS NULL
      OR invitation.invitee_user_id = v_actor_id
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  v_status := CASE
    WHEN v_invitation.revoked_at IS NOT NULL THEN 'revoked'
    WHEN v_invitation.accepted_at IS NOT NULL
      AND v_invitation.accepted_by_user_id = v_actor_id
      AND EXISTS (
        SELECT 1
        FROM public.session_access_grants AS access_grant
        WHERE access_grant.share_id = v_invitation.share_id
          AND access_grant.grantee_user_id = v_actor_id
          AND access_grant.revoked_at IS NULL
      )
      THEN 'accepted'
    WHEN v_invitation.accepted_at IS NOT NULL THEN 'revoked'
    WHEN v_invitation.expires_at <= now() THEN 'expired'
    ELSE 'pending'
  END;

  IF v_status = 'accepted' THEN
    v_share_id := v_invitation.share_id;
  END IF;

  RETURN QUERY
  SELECT v_status, v_invitation.capability, v_share_id;
END;
$$;


REVOKE ALL ON FUNCTION private.create_session_share_comment(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_session_share_comments(
  uuid,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.delete_session_share_comment(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.get_my_session_access_request(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_session_share_access_page(
  uuid,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.inspect_my_session_access_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.create_session_share_comment(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_session_share_comments(
  uuid,
  timestamptz,
  uuid,
  integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.delete_session_share_comment(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.get_my_session_access_request(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_session_share_access_page(
  uuid,
  timestamptz,
  uuid,
  integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.inspect_my_session_access_invitation(uuid, text)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.create_session_share_comment(
  p_share_id uuid,
  p_body text
)
RETURNS TABLE (
  comment_id uuid,
  is_author boolean,
  snapshot_content_revision bigint,
  body text,
  created_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.create_session_share_comment(p_share_id, p_body);
$$;

CREATE OR REPLACE FUNCTION public.list_session_share_comments(
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

CREATE OR REPLACE FUNCTION public.delete_session_share_comment(
  p_comment_id uuid
)
RETURNS TABLE (
  comment_id uuid,
  deleted_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.delete_session_share_comment(p_comment_id);
$$;

CREATE OR REPLACE FUNCTION public.get_my_session_access_request(
  p_share_id uuid
)
RETURNS TABLE (
  request_id uuid,
  requested_capability text,
  status text,
  created_at timestamptz,
  reviewed_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.get_my_session_access_request(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.list_session_share_access_page(
  p_share_id uuid,
  p_before_created_at timestamptz DEFAULT NULL,
  p_before_entry_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  entry_type text,
  entry_id uuid,
  user_id uuid,
  user_email text,
  capability text,
  status text,
  created_at timestamptz,
  expires_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_session_share_access_page(
    p_share_id,
    p_before_created_at,
    p_before_entry_id,
    p_limit
  );
$$;

CREATE OR REPLACE FUNCTION public.inspect_my_session_access_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  status text,
  capability text,
  share_id uuid
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.inspect_my_session_access_invitation(
    p_invitation_id,
    p_invite_token
  );
$$;


REVOKE ALL ON FUNCTION public.create_session_share_comment(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_session_share_comments(
  uuid,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.delete_session_share_comment(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.get_my_session_access_request(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_session_share_access_page(
  uuid,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.inspect_my_session_access_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.create_session_share_comment(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_session_share_comments(
  uuid,
  timestamptz,
  uuid,
  integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.delete_session_share_comment(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.get_my_session_access_request(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_session_share_access_page(
  uuid,
  timestamptz,
  uuid,
  integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.inspect_my_session_access_invitation(uuid, text)
  TO authenticated;
