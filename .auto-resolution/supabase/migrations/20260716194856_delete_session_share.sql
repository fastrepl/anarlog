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
      'snapshot_published'
    )
  );

CREATE OR REPLACE FUNCTION private.issue_session_share_handoff(
  p_share_id uuid,
  p_access_kind text,
  p_link_id uuid,
  p_access_version bigint
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_created_at timestamptz := clock_timestamp();
  v_request_id text;
  v_slot smallint;
BEGIN
  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_share_id::text, 0)
  );

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND share.access_version = p_access_version
    AND (
      (
        p_access_kind = 'public'
        AND p_link_id IS NULL
        AND share.general_scope = 'public'
      )
      OR (
        p_access_kind = 'link'
        AND share.general_scope = 'link'
        AND EXISTS (
          SELECT 1
          FROM public.session_share_links AS link
          WHERE link.id = p_link_id
            AND link.share_id = share.id
            AND link.revoked_at IS NULL
        )
      )
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT candidate.slot::smallint
  INTO v_slot
  FROM pg_catalog.generate_series(0, 3) AS candidate(slot)
  LEFT JOIN private.session_share_handoffs AS handoff
    ON handoff.share_id = p_share_id
    AND handoff.slot = candidate.slot
  ORDER BY
    (handoff.slot IS NULL) DESC,
    (handoff.expires_at <= v_created_at) DESC,
    handoff.created_at ASC NULLS FIRST,
    candidate.slot
  LIMIT 1;

  v_request_id := gen_random_uuid()::text;

  INSERT INTO private.session_share_handoffs (
    request_hash,
    share_id,
    slot,
    access_kind,
    link_id,
    access_version,
    created_at,
    expires_at
  ) VALUES (
    extensions.digest(v_request_id, 'sha256'),
    p_share_id,
    v_slot,
    p_access_kind,
    p_link_id,
    p_access_version,
    v_created_at,
    v_created_at + interval '60 seconds'
  )
  ON CONFLICT (share_id, slot) DO UPDATE SET
    request_hash = excluded.request_hash,
    access_kind = excluded.access_kind,
    link_id = excluded.link_id,
    access_version = excluded.access_version,
    created_at = excluded.created_at,
    expires_at = excluded.expires_at;

  RETURN QUERY
  SELECT v_request_id, v_created_at + interval '60 seconds';
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
  IF v_session_id IS NULL
    OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]'
    OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id'
      USING ERRCODE = '22023';
  END IF;

  SELECT share.id
  INTO v_existing_share_id
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
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE OF workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_existing_share_id IS NOT NULL THEN
    SELECT share.*
    INTO v_share
    FROM public.session_shares AS share
    WHERE share.id = v_existing_share_id
      AND share.workspace_id = p_workspace_id
      AND share.session_id = v_session_id
    FOR UPDATE;

    IF NOT FOUND OR v_share.deleted_at IS NOT NULL THEN
      RAISE EXCEPTION 'session share is unavailable'
        USING ERRCODE = '22023';
    END IF;

    RETURN QUERY
    SELECT
      v_share.id,
      v_share.general_scope,
      v_share.public_slug,
      v_share.access_version,
      false;
    RETURN;
  END IF;

  INSERT INTO public.session_shares (
    workspace_id,
    session_id,
    created_by_user_id
  ) VALUES (
    p_workspace_id,
    v_session_id,
    v_actor_id
  )
  ON CONFLICT (workspace_id, session_id) DO NOTHING
  RETURNING * INTO v_share;

  IF FOUND THEN
    PERFORM private.write_session_access_event(
      v_share.id,
      'share_created',
      v_actor_id
    );

    RETURN QUERY
    SELECT
      v_share.id,
      v_share.general_scope,
      v_share.public_slug,
      v_share.access_version,
      true;
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
  FOR UPDATE;

  IF NOT FOUND OR v_share.deleted_at IS NOT NULL THEN
    RAISE EXCEPTION 'session share is unavailable'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.general_scope,
    v_share.public_slug,
    v_share.access_version,
    false;
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
  SELECT *
  INTO v_result
  FROM private.create_session_share(p_workspace_id, p_session_id);

  IF v_result.was_created THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  END IF;

  RETURN QUERY
  SELECT
    v_result.share_id,
    v_result.general_scope,
    v_result.public_slug,
    v_result.access_version,
    v_result.was_created;
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
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
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
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.workspace_id = v_workspace_id
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_share.deleted_at IS NOT NULL THEN
    RETURN QUERY
    SELECT v_share.id, v_share.access_version, v_share.deleted_at, false;
    RETURN;
  END IF;

  v_deleted_at := clock_timestamp();

  UPDATE public.session_share_links AS target_link
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = v_deleted_at
  WHERE target_link.share_id = v_share.id
    AND target_link.revoked_at IS NULL;

  UPDATE public.session_access_grants AS target_grant
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = v_deleted_at,
    updated_at = v_deleted_at
  WHERE target_grant.share_id = v_share.id
    AND target_grant.revoked_at IS NULL;

  UPDATE public.session_access_invitations AS target_invitation
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = v_deleted_at,
    updated_at = v_deleted_at
  WHERE target_invitation.share_id = v_share.id
    AND target_invitation.accepted_at IS NULL
    AND target_invitation.revoked_at IS NULL;

  UPDATE public.session_access_requests AS target_request
  SET
    status = 'cancelled',
    updated_at = v_deleted_at
  WHERE target_request.share_id = v_share.id
    AND target_request.status = 'pending';

  DELETE FROM private.session_share_handoffs AS handoff
  WHERE handoff.share_id = v_share.id;

  UPDATE public.session_shares AS target_share
  SET
    general_scope = 'restricted',
    general_workspace_id = NULL,
    public_slug = 's_' || encode(extensions.gen_random_bytes(16), 'hex'),
    access_version = target_share.access_version + 1,
    updated_at = v_deleted_at,
    deleted_at = v_deleted_at
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;

  PERFORM private.write_session_access_event(
    v_share.id,
    'share_deleted',
    v_actor_id,
    NULL,
    NULL,
    NULL,
    'restricted'
  );

  RETURN QUERY
  SELECT v_share.id, v_share.access_version, v_share.deleted_at, true;
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
  v_share_id uuid;
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF v_session_id IS NULL
    OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]'
    OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE workspace.id = p_workspace_id
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.id
  INTO v_share_id
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session share is unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(v_share_id::text, 0)
  );

  PERFORM 1
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = p_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = v_share_id
    AND share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session share is unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_hyprnote_pro_entitlement();

  IF v_share.deleted_at IS NULL THEN
    RETURN QUERY
    SELECT
      v_share.id,
      v_share.general_scope,
      v_share.public_slug,
      v_share.access_version,
      false;
    RETURN;
  END IF;

  UPDATE public.session_share_links AS target_link
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now()
  WHERE target_link.share_id = v_share.id
    AND target_link.revoked_at IS NULL;

  UPDATE public.session_access_grants AS target_grant
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now(),
    updated_at = now()
  WHERE target_grant.share_id = v_share.id
    AND target_grant.revoked_at IS NULL;

  UPDATE public.session_access_invitations AS target_invitation
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now(),
    updated_at = now()
  WHERE target_invitation.share_id = v_share.id
    AND target_invitation.accepted_at IS NULL
    AND target_invitation.revoked_at IS NULL;

  UPDATE public.session_access_requests AS target_request
  SET
    status = 'cancelled',
    updated_at = now()
  WHERE target_request.share_id = v_share.id
    AND target_request.status = 'pending';

  DELETE FROM private.session_share_handoffs AS handoff
  WHERE handoff.share_id = v_share.id;

  UPDATE public.session_shares AS target_share
  SET
    general_scope = 'restricted',
    general_workspace_id = NULL,
    access_version = target_share.access_version + 1,
    updated_at = now(),
    deleted_at = NULL
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;

  PERFORM private.write_session_access_event(
    v_share.id,
    'share_reactivated',
    v_actor_id
  );

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.general_scope,
    v_share.public_slug,
    v_share.access_version,
    true;
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
  IF v_session_id IS NULL
    OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]'
    OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE workspace.id = p_workspace_id
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.id
  INTO v_share_id
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id;

  IF NOT FOUND THEN
    RETURN QUERY
    SELECT NULL::uuid, NULL::bigint, NULL::timestamptz, false;
    RETURN;
  END IF;

  RETURN QUERY
  SELECT *
  FROM private.delete_session_share(v_share_id);
END;
$$;

REVOKE ALL ON FUNCTION private.issue_session_share_handoff(
  uuid,
  text,
  uuid,
  bigint
) FROM PUBLIC, anon, authenticated, service_role;
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

GRANT EXECUTE ON FUNCTION private.protected_create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.delete_session_share(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_reactivate_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.delete_session_share_by_session(uuid, text)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.delete_session_share(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  access_version bigint,
  deleted_at timestamptz,
  was_deleted boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.delete_session_share(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.reactivate_session_share(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_reactivate_session_share(
    p_workspace_id,
    p_session_id
  );
$$;

CREATE OR REPLACE FUNCTION public.delete_session_share_by_session(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  access_version bigint,
  deleted_at timestamptz,
  was_deleted boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.delete_session_share_by_session(p_workspace_id, p_session_id);
$$;

REVOKE ALL ON FUNCTION public.delete_session_share(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.reactivate_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.delete_session_share_by_session(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.delete_session_share(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.reactivate_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.delete_session_share_by_session(uuid, text)
  TO authenticated;
