BEGIN;

CREATE OR REPLACE FUNCTION private.can_manage_session_share(
  p_workspace_id uuid,
  p_created_by_user_id uuid,
  p_user_id uuid
) RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1 FROM public.workspace_memberships AS membership
    WHERE membership.workspace_id = p_workspace_id
      AND membership.user_id = p_user_id
      AND membership.deleted_at IS NULL
      AND (membership.role IN ('owner','admin') OR p_created_by_user_id = p_user_id)
  );
$$;

CREATE OR REPLACE FUNCTION private.is_session_share_manager(
  p_share_id uuid,
  p_user_id uuid
)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor ON actor.id = membership.user_id
    WHERE share.id = p_share_id
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
      AND membership.user_id = p_user_id
      AND (membership.role IN ('owner', 'admin')
        OR share.created_by_user_id = p_user_id)
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  );
$$;

CREATE OR REPLACE FUNCTION private.require_session_share_manager(
  p_share_id uuid
)
RETURNS public.session_shares
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_workspace_id uuid;
  v_created_by_user_id uuid;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.workspace_id, share.created_by_user_id
  INTO v_workspace_id, v_created_by_user_id
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = v_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND
    OR NOT private.can_manage_session_share(
      v_workspace_id,
      v_created_by_user_id,
      v_actor_id
    )
  THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.workspace_id = v_workspace_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_share;
END;
$$;

CREATE OR REPLACE FUNCTION private.create_session_share(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  public_slug text,
  access_version bigint,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_session_id text := btrim(p_session_id);
  v_existing_share_id uuid;
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF v_session_id IS NULL OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]' OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id' USING ERRCODE = '22023';
  END IF;

  SELECT share.id INTO v_existing_share_id
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id;

  IF v_existing_share_id IS NOT NULL THEN
    PERFORM pg_catalog.pg_advisory_xact_lock(
      pg_catalog.hashtextextended(v_existing_share_id::text, 0)
    );
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE workspace.id = p_workspace_id
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.deleted_at IS NULL
  FOR UPDATE OF workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_existing_share_id IS NOT NULL THEN
    SELECT share.* INTO v_share
    FROM public.session_shares AS share
    WHERE share.id = v_existing_share_id
      AND share.workspace_id = p_workspace_id
      AND share.session_id = v_session_id
    FOR UPDATE;

    IF NOT FOUND OR v_share.deleted_at IS NOT NULL THEN
      RAISE EXCEPTION 'session share is unavailable' USING ERRCODE = '22023';
    END IF;

    IF NOT private.can_manage_session_share(
      p_workspace_id, v_share.created_by_user_id, v_actor_id
    ) THEN
      RAISE EXCEPTION 'session access operation not permitted'
        USING ERRCODE = '42501';
    END IF;

    RETURN QUERY SELECT
      v_share.id, v_share.general_scope, v_share.public_slug,
      v_share.access_version, false;
    RETURN;
  END IF;

  INSERT INTO public.session_shares (
    workspace_id, session_id, created_by_user_id
  ) VALUES (p_workspace_id, v_session_id, v_actor_id)
  ON CONFLICT (workspace_id, session_id) DO NOTHING
  RETURNING * INTO v_share;

  IF FOUND THEN
    PERFORM private.write_session_access_event(
      v_share.id, 'share_created', v_actor_id
    );
    RETURN QUERY SELECT
      v_share.id, v_share.general_scope, v_share.public_slug,
      v_share.access_version, true;
    RETURN;
  END IF;

  SELECT share.* INTO v_share
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
  FOR UPDATE;

  IF NOT FOUND OR v_share.deleted_at IS NOT NULL THEN
    RAISE EXCEPTION 'session share is unavailable' USING ERRCODE = '22023';
  END IF;

  IF NOT private.can_manage_session_share(
    p_workspace_id, v_share.created_by_user_id, v_actor_id
  ) THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY SELECT
    v_share.id, v_share.general_scope, v_share.public_slug,
    v_share.access_version, false;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_create_session_share(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  public_slug text,
  access_version bigint,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
BEGIN
  SELECT * INTO v_result
  FROM private.create_session_share(p_workspace_id, p_session_id);
  IF v_result.was_created THEN
    PERFORM private.require_workspace_or_pro_capability(
      p_workspace_id, 'team.shared_notes'
    );
  END IF;
  RETURN QUERY SELECT
    v_result.share_id, v_result.general_scope, v_result.public_slug,
    v_result.access_version, v_result.was_created;
END;
$$;

CREATE OR REPLACE FUNCTION private.delete_session_share(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  access_version bigint,
  deleted_at timestamptz,
  was_deleted boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_workspace_id uuid;
  v_share public.session_shares%ROWTYPE;
  v_deleted_at timestamptz;
BEGIN
  IF p_share_id IS NULL THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;
  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_share_id::text, 0)
  );
  SELECT share.workspace_id
  INTO v_workspace_id
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND workspace.deleted_at IS NULL;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = v_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.deleted_at IS NULL
  FOR UPDATE;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.workspace_id = v_workspace_id
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;
  IF NOT FOUND OR NOT private.can_manage_session_share(
    v_workspace_id, v_share.created_by_user_id, v_actor_id
  ) THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_share.deleted_at IS NOT NULL THEN
    RETURN QUERY SELECT v_share.id, v_share.access_version,
      v_share.deleted_at, false;
    RETURN;
  END IF;

  v_deleted_at := clock_timestamp();
  UPDATE public.session_share_links AS target_link
  SET revoked_by_user_id = v_actor_id, revoked_at = v_deleted_at
  WHERE target_link.share_id = v_share.id
    AND target_link.revoked_at IS NULL;
  UPDATE public.session_access_grants AS target_grant
  SET revoked_by_user_id = v_actor_id, revoked_at = v_deleted_at,
      updated_at = v_deleted_at
  WHERE target_grant.share_id = v_share.id
    AND target_grant.revoked_at IS NULL;
  UPDATE public.session_access_invitations AS target_invitation
  SET revoked_by_user_id = v_actor_id, revoked_at = v_deleted_at,
      updated_at = v_deleted_at
  WHERE target_invitation.share_id = v_share.id
    AND target_invitation.accepted_at IS NULL
    AND target_invitation.revoked_at IS NULL;
  UPDATE public.session_access_requests AS target_request
  SET status = 'cancelled', updated_at = v_deleted_at
  WHERE target_request.share_id = v_share.id
    AND target_request.status = 'pending';
  DELETE FROM private.session_share_handoffs AS handoff
  WHERE handoff.share_id = v_share.id;
  UPDATE public.session_shares AS target_share
  SET general_scope = 'restricted', general_workspace_id = NULL,
      public_slug = 's_' || encode(extensions.gen_random_bytes(16), 'hex'),
      access_version = target_share.access_version + 1,
      updated_at = v_deleted_at, deleted_at = v_deleted_at
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;
  PERFORM private.write_session_access_event(
    v_share.id, 'share_deleted', v_actor_id, NULL, NULL, NULL, 'restricted'
  );
  RETURN QUERY SELECT
    v_share.id, v_share.access_version, v_share.deleted_at, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_reactivate_session_share(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  public_slug text,
  access_version bigint,
  was_reactivated boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_session_id text := btrim(p_session_id);
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF v_session_id IS NULL OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]' OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id' USING ERRCODE = '22023';
  END IF;

  SELECT share.* INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    PERFORM private.require_workspace_or_pro_capability(
      p_workspace_id, 'team.shared_notes'
    );
    RETURN QUERY
    SELECT created.share_id, created.general_scope, created.public_slug,
      created.access_version, created.was_created
    FROM private.create_session_share(p_workspace_id, v_session_id) AS created;
    RETURN;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_share.id::text, 0)
  );
  PERFORM 1
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = p_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.deleted_at IS NULL
  FOR UPDATE;
  IF NOT FOUND OR NOT private.can_manage_session_share(
    p_workspace_id, v_share.created_by_user_id, v_actor_id
  ) THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;
  PERFORM private.require_workspace_or_pro_capability(
    p_workspace_id, 'team.shared_notes'
  );
  IF v_share.deleted_at IS NULL THEN
    RETURN QUERY SELECT
      v_share.id, v_share.general_scope, v_share.public_slug,
      v_share.access_version, false;
    RETURN;
  END IF;

  UPDATE public.session_share_links AS target_link
  SET revoked_by_user_id = v_actor_id, revoked_at = now()
  WHERE target_link.share_id = v_share.id
    AND target_link.revoked_at IS NULL;
  UPDATE public.session_access_grants AS target_grant
  SET revoked_by_user_id = v_actor_id, revoked_at = now(), updated_at = now()
  WHERE target_grant.share_id = v_share.id
    AND target_grant.revoked_at IS NULL;
  UPDATE public.session_access_invitations AS target_invitation
  SET revoked_by_user_id = v_actor_id, revoked_at = now(), updated_at = now()
  WHERE target_invitation.share_id = v_share.id
    AND target_invitation.accepted_at IS NULL
    AND target_invitation.revoked_at IS NULL;
  UPDATE public.session_access_requests AS target_request
  SET status = 'cancelled', updated_at = now()
  WHERE target_request.share_id = v_share.id
    AND target_request.status = 'pending';
  DELETE FROM private.session_share_handoffs AS handoff
  WHERE handoff.share_id = v_share.id;
  UPDATE public.session_shares AS target_share
  SET general_scope = 'restricted', general_workspace_id = NULL,
      access_version = target_share.access_version + 1,
      updated_at = now(), deleted_at = NULL
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;
  PERFORM private.write_session_access_event(
    v_share.id, 'share_reactivated', v_actor_id
  );
  RETURN QUERY SELECT
    v_share.id, v_share.general_scope, v_share.public_slug,
    v_share.access_version, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.delete_session_share_by_session(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  access_version bigint,
  deleted_at timestamptz,
  was_deleted boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_session_id text := btrim(p_session_id);
  v_share_id uuid;
BEGIN
  IF v_session_id IS NULL OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]' OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id' USING ERRCODE = '22023';
  END IF;
  SELECT share.id INTO v_share_id
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id;
  IF NOT FOUND THEN
    RETURN QUERY SELECT NULL::uuid, NULL::bigint, NULL::timestamptz, false;
    RETURN;
  END IF;
  RETURN QUERY SELECT * FROM private.delete_session_share(v_share_id);
END;
$$;

CREATE OR REPLACE FUNCTION private.require_session_share_attachment_manager(
  p_share_id uuid,
  p_actor_user_id uuid
)
RETURNS public.session_shares
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF p_share_id IS NULL OR p_actor_user_id IS NULL THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;
  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1 FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;
  SELECT share.* INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND membership.user_id = p_actor_user_id
    AND membership.deleted_at IS NULL
    AND private.can_manage_session_share(
      share.workspace_id, share.created_by_user_id, p_actor_user_id
    )
  FOR UPDATE OF share, workspace, membership;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session attachment operation not permitted'
      USING ERRCODE = '42501';
  END IF;
  RETURN v_share;
END;
$$;

CREATE OR REPLACE FUNCTION private.prepare_my_session_share_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_actor_user_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM 1 FROM public.session_shares AS share
  WHERE share.id = p_share_id FOR SHARE OF share;
  IF NOT FOUND THEN RETURN; END IF;
  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1 FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );
  IF NOT FOUND THEN RETURN; END IF;
  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND source_workspace.deleted_at IS NULL
    AND (
      private.can_manage_session_share(
        share.workspace_id, share.created_by_user_id, p_actor_user_id
      )
      OR EXISTS (
        SELECT 1 FROM public.session_access_grants AS access_grant
        WHERE access_grant.share_id = share.id
          AND access_grant.grantee_user_id = p_actor_user_id
          AND access_grant.revoked_at IS NULL
      )
      OR (
        share.general_scope = 'workspace'
        AND EXISTS (
          SELECT 1
          FROM public.workspaces AS target_workspace
          JOIN public.workspace_memberships AS target_membership
            ON target_membership.workspace_id = target_workspace.id
          WHERE target_workspace.id = share.general_workspace_id
            AND target_workspace.deleted_at IS NULL
            AND target_membership.user_id = p_actor_user_id
            AND target_membership.deleted_at IS NULL
        )
      )
      OR share.general_scope = 'public'
    );
  IF NOT FOUND THEN RETURN; END IF;
  RETURN QUERY SELECT *
  FROM private.prepare_session_share_attachment_download(
    p_share_id, p_attachment_id, p_download_expires_at
  );
END;
$$;

CREATE OR REPLACE FUNCTION private.require_session_share_editor(
  p_share_id uuid,
  p_actor_user_id uuid
)
RETURNS TABLE (manage_access boolean, access_version bigint)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_manage_access boolean;
  v_has_editor_grant boolean;
BEGIN
  IF p_share_id IS NULL OR p_actor_user_id IS NULL THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;
  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1 FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;
  SELECT share.* INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;
  SELECT private.can_manage_session_share(
    v_share.workspace_id, v_share.created_by_user_id, p_actor_user_id
  ) INTO v_manage_access;
  SELECT EXISTS (
    SELECT 1 FROM public.session_access_grants AS access_grant
    WHERE access_grant.share_id = v_share.id
      AND access_grant.grantee_user_id = p_actor_user_id
      AND access_grant.capability = 'editor'
      AND access_grant.revoked_at IS NULL
  ) INTO v_has_editor_grant;
  IF NOT v_manage_access AND NOT v_has_editor_grant THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;
  RETURN QUERY SELECT v_manage_access, v_share.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.resolve_my_session_access(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
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
  WITH access_candidates AS (
    SELECT 3 AS capability_rank, true AS can_manage
    FROM public.session_shares AS candidate_share
    JOIN public.workspaces AS source_workspace
      ON source_workspace.id = candidate_share.workspace_id
    JOIN public.workspace_memberships AS source_membership
      ON source_membership.workspace_id = source_workspace.id
    WHERE candidate_share.id = p_share_id
      AND candidate_share.deleted_at IS NULL
      AND source_workspace.deleted_at IS NULL
      AND source_membership.user_id = v_actor_id
      AND source_membership.deleted_at IS NULL
      AND private.can_manage_session_share(
        candidate_share.workspace_id,
        candidate_share.created_by_user_id,
        v_actor_id
      )
    UNION ALL
    SELECT private.session_capability_rank(access_grant.capability), false
    FROM public.session_access_grants AS access_grant
    WHERE access_grant.share_id = p_share_id
      AND access_grant.grantee_user_id = v_actor_id
      AND access_grant.revoked_at IS NULL
    UNION ALL
    SELECT 1, false
    FROM public.session_shares AS candidate_share
    JOIN public.workspaces AS target_workspace
      ON target_workspace.id = candidate_share.general_workspace_id
    JOIN public.workspace_memberships AS target_membership
      ON target_membership.workspace_id = target_workspace.id
    WHERE candidate_share.id = p_share_id
      AND candidate_share.general_scope = 'workspace'
      AND candidate_share.deleted_at IS NULL
      AND target_workspace.deleted_at IS NULL
      AND target_membership.user_id = v_actor_id
      AND target_membership.deleted_at IS NULL
    UNION ALL
    SELECT 1, false
    FROM public.session_shares AS candidate_share
    WHERE candidate_share.id = p_share_id
      AND candidate_share.general_scope = 'public'
      AND candidate_share.deleted_at IS NULL
  ), effective_access AS (
    SELECT max(capability_rank) AS capability_rank,
      bool_or(can_manage) AS can_manage
    FROM access_candidates
  )
  SELECT share.id, share.workspace_id, share.session_id,
    CASE effective_access.capability_rank
      WHEN 1 THEN 'viewer' WHEN 2 THEN 'commenter' WHEN 3 THEN 'editor'
    END,
    effective_access.can_manage, share.access_version
  FROM public.session_shares AS share
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  CROSS JOIN effective_access
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND source_workspace.deleted_at IS NULL
    AND effective_access.capability_rank IS NOT NULL;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_my_accessible_sessions()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
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
  WITH access_candidates AS (
    SELECT share.id AS candidate_share_id, 3 AS capability_rank,
      true AS can_manage
    FROM public.session_shares AS share
    JOIN public.workspaces AS source_workspace
      ON source_workspace.id = share.workspace_id
    JOIN public.workspace_memberships AS source_membership
      ON source_membership.workspace_id = source_workspace.id
    WHERE share.deleted_at IS NULL
      AND source_workspace.deleted_at IS NULL
      AND source_membership.user_id = v_actor_id
      AND source_membership.deleted_at IS NULL
      AND private.can_manage_session_share(
        share.workspace_id, share.created_by_user_id, v_actor_id
      )
    UNION ALL
    SELECT access_grant.share_id,
      private.session_capability_rank(access_grant.capability), false
    FROM public.session_access_grants AS access_grant
    JOIN public.session_shares AS share ON share.id = access_grant.share_id
    WHERE access_grant.grantee_user_id = v_actor_id
      AND access_grant.revoked_at IS NULL
      AND share.deleted_at IS NULL
    UNION ALL
    SELECT share.id, 1, false
    FROM public.session_shares AS share
    JOIN public.workspaces AS target_workspace
      ON target_workspace.id = share.general_workspace_id
    JOIN public.workspace_memberships AS target_membership
      ON target_membership.workspace_id = target_workspace.id
    WHERE share.general_scope = 'workspace'
      AND share.deleted_at IS NULL
      AND target_workspace.deleted_at IS NULL
      AND target_membership.user_id = v_actor_id
      AND target_membership.deleted_at IS NULL
  ), effective_access AS (
    SELECT candidate_share_id, max(capability_rank) AS capability_rank,
      bool_or(can_manage) AS can_manage
    FROM access_candidates
    GROUP BY candidate_share_id
  )
  SELECT share.id, share.workspace_id, share.session_id,
    CASE effective_access.capability_rank
      WHEN 1 THEN 'viewer' WHEN 2 THEN 'commenter' WHEN 3 THEN 'editor'
    END,
    effective_access.can_manage, share.access_version
  FROM effective_access
  JOIN public.session_shares AS share
    ON share.id = effective_access.candidate_share_id
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  WHERE share.deleted_at IS NULL AND source_workspace.deleted_at IS NULL
  ORDER BY share.updated_at DESC, share.id;
END;
$$;

REVOKE ALL ON FUNCTION private.can_manage_session_share(uuid, uuid, uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.is_session_share_manager(uuid, uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.require_session_share_manager(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.protected_create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.delete_session_share(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.protected_reactivate_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.delete_session_share_by_session(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.require_session_share_attachment_manager(uuid, uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.require_session_share_editor(uuid, uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.resolve_my_session_access(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.list_my_accessible_sessions()
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.protected_create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.delete_session_share(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_reactivate_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.delete_session_share_by_session(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.resolve_my_session_access(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_accessible_sessions()
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.require_session_share_attachment_manager(
  uuid, uuid
) TO service_role;
GRANT EXECUTE ON FUNCTION private.prepare_my_session_share_attachment_download(
  uuid, uuid, uuid, timestamptz
) TO service_role;

COMMIT;
