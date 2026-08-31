-- Personal Pro pays for personal sharing. Shared workspace expansion is paid
-- by that workspace and is therefore checked against its Team capability.

BEGIN;

SET LOCAL lock_timeout = '30s';

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
    PERFORM private.require_workspace_or_pro_capability(
      p_workspace_id,
      'team.shared_notes'
    );
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

  PERFORM private.require_workspace_or_pro_capability(
    p_workspace_id,
    'team.shared_notes'
  );

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

CREATE OR REPLACE FUNCTION private.protected_set_session_share_scope(
  p_share_id uuid,
  p_general_scope text,
  p_general_workspace_id uuid DEFAULT NULL
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  general_workspace_id uuid,
  public_slug text,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.id = p_share_id;

  IF FOUND THEN
    PERFORM private.assert_allowed_share_scope(
      v_share.workspace_id,
      p_general_scope
    );
  END IF;

  SELECT *
  INTO v_result
  FROM private.set_session_share_scope(
    p_share_id,
    p_general_scope,
    p_general_workspace_id
  );

  IF p_general_scope <> 'restricted' THEN
    PERFORM private.require_workspace_or_pro_capability(
      v_share.workspace_id,
      'team.shared_notes'
    );
  END IF;

  RETURN QUERY
  SELECT
    v_result.share_id,
    v_result.general_scope,
    v_result.general_workspace_id,
    v_result.public_slug,
    v_result.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_issue_session_share_link(
  p_share_id uuid,
  p_force_rotate boolean
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.id = p_share_id;

  IF FOUND THEN
    PERFORM private.assert_allowed_share_scope(v_share.workspace_id, 'link');
  END IF;

  SELECT *
  INTO v_result
  FROM private.issue_session_share_link(p_share_id, p_force_rotate);

  IF p_force_rotate OR v_result.was_created THEN
    PERFORM private.require_workspace_or_pro_capability(
      v_share.workspace_id,
      'team.shared_notes'
    );
  END IF;

  RETURN QUERY
  SELECT
    v_result.share_id,
    v_result.link_id,
    v_result.link_token,
    v_result.access_version,
    v_result.was_created;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_create_session_access_invitation(
  p_share_id uuid,
  p_invitee_email text,
  p_capability text
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_workspace_id uuid;
BEGIN
  SELECT share.workspace_id
  INTO v_workspace_id
  FROM public.session_shares AS share
  WHERE share.id = p_share_id;

  SELECT *
  INTO v_result
  FROM private.create_session_access_invitation(
    p_share_id,
    p_invitee_email,
    p_capability
  );

  IF v_result.was_created THEN
    PERFORM private.require_workspace_or_pro_capability(
      v_workspace_id,
      'team.shared_notes'
    );
  END IF;

  RETURN QUERY
  SELECT
    v_result.invitation_id,
    v_result.invite_token,
    v_result.invitation_expires_at,
    v_result.was_created;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_resend_session_access_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_workspace_id uuid;
BEGIN
  SELECT share.workspace_id
  INTO v_workspace_id
  FROM public.session_access_invitations AS invitation
  JOIN public.session_shares AS share
    ON share.id = invitation.share_id
  WHERE invitation.id = p_invitation_id;

  SELECT *
  INTO v_result
  FROM private.resend_session_access_invitation(p_invitation_id);

  PERFORM private.require_workspace_or_pro_capability(
    v_workspace_id,
    'team.shared_notes'
  );

  RETURN QUERY
  SELECT
    v_result.invitation_id,
    v_result.invite_token,
    v_result.invitation_expires_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_update_session_access_grant(
  p_grant_id uuid,
  p_capability text
)
RETURNS TABLE (
  grant_id uuid,
  capability text,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_grant public.session_access_grants%ROWTYPE;
  v_workspace_id uuid;
BEGIN
  IF private.session_capability_rank(p_capability) = 0 THEN
    RAISE EXCEPTION 'invalid session access capability'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_grant.share_id);

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id
  FOR UPDATE;

  IF v_grant.revoked_at IS NOT NULL THEN
    RAISE EXCEPTION 'session access grant is unavailable'
      USING ERRCODE = '22023';
  END IF;

  IF private.session_capability_rank(p_capability)
    > private.session_capability_rank(v_grant.capability)
  THEN
    SELECT share.workspace_id
    INTO v_workspace_id
    FROM public.session_shares AS share
    WHERE share.id = v_grant.share_id;

    PERFORM private.require_workspace_or_pro_capability(
      v_workspace_id,
      'team.shared_notes'
    );
  END IF;

  RETURN QUERY
  SELECT *
  FROM private.update_session_access_grant(p_grant_id, p_capability);
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_review_session_access_request(
  p_request_id uuid,
  p_decision text,
  p_capability text DEFAULT NULL
)
RETURNS TABLE (
  request_id uuid,
  status text,
  grant_id uuid,
  capability text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_workspace_id uuid;
BEGIN
  SELECT share.workspace_id
  INTO v_workspace_id
  FROM public.session_access_requests AS access_request
  JOIN public.session_shares AS share
    ON share.id = access_request.share_id
  WHERE access_request.id = p_request_id;

  SELECT *
  INTO v_result
  FROM private.review_session_access_request(
    p_request_id,
    p_decision,
    p_capability
  );

  IF p_decision = 'approved' THEN
    PERFORM private.require_workspace_or_pro_capability(
      v_workspace_id,
      'team.shared_notes'
    );
  END IF;

  RETURN QUERY
  SELECT
    v_result.request_id,
    v_result.status,
    v_result.grant_id,
    v_result.capability;
END;
$$;

COMMIT;
