CREATE OR REPLACE FUNCTION private.require_hyprnote_pro_entitlement()
RETURNS void
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
BEGIN
  IF NOT (
    COALESCE(
      (SELECT auth.jwt()) -> 'entitlements',
      '[]'::jsonb
    ) @> '["hyprnote_pro"]'::jsonb
  ) THEN
    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;
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
  v_actor_id uuid := private.require_permanent_user();
  v_session_id text := btrim(p_session_id);
  v_was_active boolean := false;
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
    AND membership.deleted_at IS NULL
  FOR UPDATE OF workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.deleted_at IS NULL
  INTO v_was_active
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
  FOR UPDATE;

  IF NOT COALESCE(v_was_active, false) THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  END IF;

  RETURN QUERY
  SELECT *
  FROM private.create_session_share(p_workspace_id, v_session_id);
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
BEGIN
  SELECT *
  INTO v_result
  FROM private.set_session_share_scope(
    p_share_id,
    p_general_scope,
    p_general_workspace_id
  );

  IF p_general_scope <> 'restricted' THEN
    PERFORM private.require_hyprnote_pro_entitlement();
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
BEGIN
  SELECT *
  INTO v_result
  FROM private.issue_session_share_link(p_share_id, p_force_rotate);

  IF p_force_rotate OR v_result.was_created THEN
    PERFORM private.require_hyprnote_pro_entitlement();
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
BEGIN
  SELECT *
  INTO v_result
  FROM private.create_session_access_invitation(
    p_share_id,
    p_invitee_email,
    p_capability
  );

  IF v_result.was_created THEN
    PERFORM private.require_hyprnote_pro_entitlement();
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
BEGIN
  SELECT *
  INTO v_result
  FROM private.resend_session_access_invitation(p_invitation_id);

  PERFORM private.require_hyprnote_pro_entitlement();

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
    PERFORM private.require_hyprnote_pro_entitlement();
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
BEGIN
  SELECT *
  INTO v_result
  FROM private.review_session_access_request(
    p_request_id,
    p_decision,
    p_capability
  );

  IF p_decision = 'approved' THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  END IF;

  RETURN QUERY
  SELECT
    v_result.request_id,
    v_result.status,
    v_result.grant_id,
    v_result.capability;
END;
$$;

REVOKE ALL ON FUNCTION private.require_hyprnote_pro_entitlement()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.set_session_share_scope(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.issue_session_share_link(uuid, boolean)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.create_session_access_invitation(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.resend_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.update_session_access_grant(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.review_session_access_request(uuid, text, text)
  FROM PUBLIC, anon, authenticated;

REVOKE ALL ON FUNCTION private.protected_create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_set_session_share_scope(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_issue_session_share_link(uuid, boolean)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_create_session_access_invitation(
  uuid,
  text,
  text
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_resend_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_update_session_access_grant(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_review_session_access_request(
  uuid,
  text,
  text
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.protected_create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_set_session_share_scope(
  uuid,
  text,
  uuid
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_issue_session_share_link(
  uuid,
  boolean
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_create_session_access_invitation(
  uuid,
  text,
  text
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_resend_session_access_invitation(
  uuid
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_update_session_access_grant(
  uuid,
  text
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_review_session_access_request(
  uuid,
  text,
  text
) TO authenticated;

CREATE OR REPLACE FUNCTION public.create_session_share(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_create_session_share(p_workspace_id, p_session_id);
$$;

CREATE OR REPLACE FUNCTION public.set_session_share_scope(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_set_session_share_scope(
    p_share_id,
    p_general_scope,
    p_general_workspace_id
  );
$$;

CREATE OR REPLACE FUNCTION public.enable_session_share_link(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_issue_session_share_link(p_share_id, false);
$$;

CREATE OR REPLACE FUNCTION public.rotate_session_share_link(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_issue_session_share_link(p_share_id, true);
$$;

CREATE OR REPLACE FUNCTION public.create_session_access_invitation(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_create_session_access_invitation(
    p_share_id,
    p_invitee_email,
    p_capability
  );
$$;

CREATE OR REPLACE FUNCTION public.resend_session_access_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_resend_session_access_invitation(p_invitation_id);
$$;

CREATE OR REPLACE FUNCTION public.update_session_access_grant(
  p_grant_id uuid,
  p_capability text
)
RETURNS TABLE (
  grant_id uuid,
  capability text,
  access_version bigint
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_update_session_access_grant(
    p_grant_id,
    p_capability
  );
$$;

CREATE OR REPLACE FUNCTION public.review_session_access_request(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_review_session_access_request(
    p_request_id,
    p_decision,
    p_capability
  );
$$;

REVOKE ALL ON FUNCTION public.create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.set_session_share_scope(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.enable_session_share_link(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.rotate_session_share_link(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.create_session_access_invitation(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.resend_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.update_session_access_grant(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.review_session_access_request(uuid, text, text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.set_session_share_scope(uuid, text, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.enable_session_share_link(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.rotate_session_share_link(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.create_session_access_invitation(
  uuid,
  text,
  text
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.resend_session_access_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.update_session_access_grant(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.review_session_access_request(uuid, text, text)
  TO authenticated;
