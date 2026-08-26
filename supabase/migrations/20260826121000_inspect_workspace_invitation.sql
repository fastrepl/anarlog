CREATE OR REPLACE FUNCTION private.inspect_my_workspace_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  status text,
  workspace_id uuid,
  workspace_name text
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_invitation public.workspace_invitations%ROWTYPE;
  v_workspace public.workspaces%ROWTYPE;
  v_status text;
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
  FROM public.workspace_invitations AS invitation
  JOIN public.workspaces AS workspace
    ON workspace.id = invitation.workspace_id
  WHERE invitation.id = p_invitation_id
    AND invitation.token_hash = extensions.digest(p_invite_token, 'sha256')
    AND invitation.invitee_email = v_actor_email
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND (
      invitation.invitee_user_id IS NULL
      OR invitation.invitee_user_id = v_actor_id
    );

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT workspace.*
  INTO v_workspace
  FROM public.workspaces AS workspace
  WHERE workspace.id = v_invitation.workspace_id;

  v_status := CASE
    WHEN v_invitation.revoked_at IS NOT NULL THEN 'revoked'
    WHEN v_invitation.accepted_at IS NOT NULL
      AND EXISTS (
        SELECT 1
        FROM public.workspace_memberships AS membership
        WHERE membership.workspace_id = v_invitation.workspace_id
          AND membership.user_id = v_actor_id
          AND membership.deleted_at IS NULL
      )
      THEN 'accepted'
    WHEN v_invitation.accepted_at IS NOT NULL THEN 'revoked'
    WHEN v_invitation.expires_at <= now() THEN 'expired'
    ELSE 'pending'
  END;

  RETURN QUERY
  SELECT v_status, v_workspace.id, v_workspace.name;
END;
$$;

REVOKE ALL ON FUNCTION private.inspect_my_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.inspect_my_workspace_invitation(uuid, text)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.inspect_my_workspace_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  status text,
  workspace_id uuid,
  workspace_name text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.inspect_my_workspace_invitation(
    p_invitation_id,
    p_invite_token
  );
$$;

REVOKE ALL ON FUNCTION public.inspect_my_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.inspect_my_workspace_invitation(uuid, text)
  TO authenticated;
