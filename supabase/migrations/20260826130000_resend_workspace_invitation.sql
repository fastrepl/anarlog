CREATE OR REPLACE FUNCTION private.resend_workspace_invitation(
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
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_workspace_id uuid;
  v_existing public.workspace_invitations%ROWTYPE;
  v_expires_at timestamptz;
  v_invitation_id uuid;
  v_invitee_user_id uuid;
  v_token text;
BEGIN
  SELECT invitation.workspace_id
  INTO v_workspace_id
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id;

  IF v_workspace_id IS NULL THEN
    RAISE EXCEPTION 'workspace invitation operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT membership.role
  INTO v_actor_role
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  JOIN auth.users AS actor
    ON actor.id = membership.user_id
  WHERE workspace.id = v_workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
  FOR UPDATE OF workspace;

  IF v_actor_role IS NULL THEN
    RAISE EXCEPTION 'workspace invitation operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT invitation.*
  INTO v_existing
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id
  FOR UPDATE;

  IF NOT FOUND
    OR v_existing.accepted_at IS NOT NULL
    OR v_existing.revoked_at IS NOT NULL
  THEN
    RAISE EXCEPTION 'workspace invitation is unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT auth_user.id
  INTO v_invitee_user_id
  FROM auth.users AS auth_user
  WHERE lower(btrim(auth_user.email)) = v_existing.invitee_email
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false
  ORDER BY auth_user.created_at, auth_user.id
  LIMIT 1;

  IF EXISTS (
    SELECT 1
    FROM public.workspace_memberships AS membership
    JOIN auth.users AS member_user
      ON member_user.id = membership.user_id
    WHERE membership.workspace_id = v_workspace_id
      AND membership.deleted_at IS NULL
      AND lower(btrim(member_user.email)) = v_existing.invitee_email
  ) THEN
    RAISE EXCEPTION 'workspace invitation not needed'
      USING ERRCODE = '22023';
  END IF;

  UPDATE public.workspace_invitations
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now(),
    updated_at = now()
  WHERE id = v_existing.id;

  v_token := rtrim(
    translate(encode(extensions.gen_random_bytes(32), 'base64'), '+/', '-_'),
    '='
  );
  v_expires_at := now() + interval '30 days';

  INSERT INTO public.workspace_invitations (
    workspace_id,
    invitee_email,
    invitee_user_id,
    token_hash,
    role,
    invited_by_user_id,
    expires_at
  ) VALUES (
    v_workspace_id,
    v_existing.invitee_email,
    v_invitee_user_id,
    extensions.digest(v_token, 'sha256'),
    'member',
    v_actor_id,
    v_expires_at
  )
  RETURNING id INTO v_invitation_id;

  RETURN QUERY
  SELECT v_invitation_id, v_token, v_expires_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_resend_workspace_invitation(
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
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.resend_workspace_invitation(p_invitation_id);
END;
$$;

REVOKE ALL ON FUNCTION private.resend_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_resend_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.protected_resend_workspace_invitation(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.resend_workspace_invitation(
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
  FROM private.protected_resend_workspace_invitation(p_invitation_id);
$$;

REVOKE ALL ON FUNCTION public.resend_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.resend_workspace_invitation(uuid)
  TO authenticated;
