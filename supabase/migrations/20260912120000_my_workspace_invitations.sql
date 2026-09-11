-- Invitees can see and act on their own pending invitations from inside the
-- app, without needing the emailed token: the account email is confirmed, so it
-- already proves ownership of the invited address.
CREATE OR REPLACE FUNCTION public.list_my_workspace_invitations()
RETURNS TABLE (
  invitation_id uuid,
  workspace_id uuid,
  workspace_name text,
  workspace_logo_data text,
  invited_by_email text,
  expires_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
BEGIN
  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id
    AND auth_user.email_confirmed_at IS NOT NULL;

  IF v_actor_email IS NULL THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT
    invitation.id,
    workspace.id,
    workspace.name,
    workspace.logo_data,
    lower(btrim(inviter.email)),
    invitation.expires_at
  FROM public.workspace_invitations AS invitation
  JOIN public.workspaces AS workspace
    ON workspace.id = invitation.workspace_id
  LEFT JOIN auth.users AS inviter
    ON inviter.id = invitation.invited_by_user_id
  WHERE invitation.invitee_email = v_actor_email
    AND (
      invitation.invitee_user_id IS NULL
      OR invitation.invitee_user_id = v_actor_id
    )
    AND invitation.accepted_at IS NULL
    AND invitation.revoked_at IS NULL
    AND invitation.expires_at > now()
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND NOT EXISTS (
      SELECT 1
      FROM public.workspace_memberships AS membership
      WHERE membership.workspace_id = workspace.id
        AND membership.user_id = v_actor_id
        AND membership.deleted_at IS NULL
    )
  ORDER BY invitation.created_at DESC, invitation.id;
END;
$$;

REVOKE ALL ON FUNCTION public.list_my_workspace_invitations()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_workspace_invitations()
  TO authenticated;

CREATE OR REPLACE FUNCTION public.accept_my_workspace_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  workspace_id uuid,
  membership_id uuid
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_invitation public.workspace_invitations%ROWTYPE;
  v_membership public.workspace_memberships%ROWTYPE;
BEGIN
  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id
    AND auth_user.email_confirmed_at IS NOT NULL;

  IF v_actor_email IS NULL THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_invitation
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id
    AND invitation.invitee_email = v_actor_email
  FOR UPDATE;

  IF NOT FOUND
    OR v_invitation.revoked_at IS NOT NULL
    OR v_invitation.expires_at <= now()
    OR (
      v_invitation.invitee_user_id IS NOT NULL
      AND v_invitation.invitee_user_id <> v_actor_id
    )
  THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  -- Email auto-join and membership changes take this same lock.
  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = v_invitation.workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT membership.*
  INTO v_membership
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = v_invitation.workspace_id
    AND membership.user_id = v_actor_id
  FOR UPDATE;

  IF v_invitation.accepted_at IS NOT NULL THEN
    IF v_membership.id IS NULL OR v_membership.deleted_at IS NOT NULL THEN
      RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
        USING ERRCODE = '22023';
    END IF;

    RETURN QUERY
    SELECT v_invitation.workspace_id, v_membership.id;
    RETURN;
  END IF;

  IF v_membership.id IS NULL THEN
    INSERT INTO public.workspace_memberships (
      workspace_id,
      user_id,
      role
    ) VALUES (
      v_invitation.workspace_id,
      v_actor_id,
      'member'
    )
    RETURNING * INTO v_membership;
  ELSIF v_membership.deleted_at IS NOT NULL THEN
    IF v_membership.role <> 'member' THEN
      RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
        USING ERRCODE = '22023';
    END IF;

    UPDATE public.workspace_memberships
    SET
      deleted_at = NULL,
      updated_at = now()
    WHERE id = v_membership.id
    RETURNING * INTO v_membership;
  END IF;

  UPDATE public.workspace_invitations
  SET
    invitee_user_id = v_actor_id,
    accepted_at = now(),
    updated_at = now()
  WHERE id = v_invitation.id;

  RETURN QUERY
  SELECT v_invitation.workspace_id, v_membership.id;
END;
$$;

REVOKE ALL ON FUNCTION public.accept_my_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.accept_my_workspace_invitation(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.decline_my_workspace_invitation(
  p_invitation_id uuid
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_invitation public.workspace_invitations%ROWTYPE;
BEGIN
  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id
    AND auth_user.email_confirmed_at IS NOT NULL;

  IF v_actor_email IS NULL THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_invitation
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id
    AND invitation.invitee_email = v_actor_email
    AND invitation.accepted_at IS NULL
    AND (
      invitation.invitee_user_id IS NULL
      OR invitation.invitee_user_id = v_actor_id
    )
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  IF v_invitation.revoked_at IS NOT NULL THEN
    RETURN;
  END IF;

  UPDATE public.workspace_invitations
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now(),
    updated_at = now()
  WHERE id = v_invitation.id;
END;
$$;

REVOKE ALL ON FUNCTION public.decline_my_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.decline_my_workspace_invitation(uuid)
  TO authenticated;
