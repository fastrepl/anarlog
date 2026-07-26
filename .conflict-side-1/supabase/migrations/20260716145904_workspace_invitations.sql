CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA extensions;

ALTER TABLE public.workspaces
  DROP CONSTRAINT workspaces_kind_check,
  DROP CONSTRAINT workspaces_personal_identity_check,
  ADD CONSTRAINT workspaces_kind_check CHECK (
    kind IN ('personal', 'shared')
  ),
  ADD CONSTRAINT workspaces_identity_check CHECK (
    (kind = 'personal' AND id = owner_user_id)
    OR (kind = 'shared' AND id <> owner_user_id)
  );

CREATE TABLE public.workspace_invitations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  invitee_email text NOT NULL,
  invitee_user_id uuid REFERENCES auth.users(id) ON DELETE CASCADE,
  token_hash bytea NOT NULL,
  role text NOT NULL DEFAULT 'member',
  invited_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  revoked_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  accepted_at timestamptz,
  revoked_at timestamptz,
  CONSTRAINT workspace_invitations_email_check CHECK (
    invitee_email = lower(btrim(invitee_email))
    AND invitee_email ~ '^[^[:space:]@]+@[^[:space:]@]+$'
    AND invitee_email !~ '[[:cntrl:]]'
    AND octet_length(invitee_email) <= 320
  ),
  CONSTRAINT workspace_invitations_token_hash_check CHECK (
    octet_length(token_hash) = 32
  ),
  CONSTRAINT workspace_invitations_role_check CHECK (role = 'member'),
  CONSTRAINT workspace_invitations_state_check CHECK (
    (accepted_at IS NULL OR (invitee_user_id IS NOT NULL AND revoked_at IS NULL))
    AND (revoked_at IS NULL OR accepted_at IS NULL)
    AND (revoked_by_user_id IS NULL OR revoked_at IS NOT NULL)
  ),
  CONSTRAINT workspace_invitations_expiry_check CHECK (expires_at > created_at),
  CONSTRAINT workspace_invitations_token_hash_key UNIQUE (token_hash)
);

CREATE UNIQUE INDEX workspace_invitations_pending_key
  ON public.workspace_invitations(workspace_id, invitee_email)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;

ALTER TABLE public.workspace_invitations ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.workspace_invitations FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.workspace_invitations TO service_role;

CREATE POLICY workspace_invitations_service_all
  ON public.workspace_invitations
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

DROP TRIGGER IF EXISTS on_auth_user_workspace_converted ON auth.users;
CREATE TRIGGER on_auth_user_workspace_converted
  AFTER UPDATE OF is_anonymous ON auth.users
  FOR EACH ROW
  WHEN (OLD.is_anonymous IS TRUE AND NEW.is_anonymous IS FALSE)
  EXECUTE FUNCTION private.handle_new_user_workspace();

DROP POLICY workspace_memberships_select_self ON public.workspace_memberships;
CREATE POLICY workspace_memberships_select_self
  ON public.workspace_memberships
  FOR SELECT
  TO authenticated
  USING (
    COALESCE(((SELECT auth.jwt()) ->> 'is_anonymous')::boolean, false) = false
    AND user_id = (SELECT auth.uid())
    AND deleted_at IS NULL
  );

DROP POLICY workspaces_select_member ON public.workspaces;
CREATE POLICY workspaces_select_member
  ON public.workspaces
  FOR SELECT
  TO authenticated
  USING (
    COALESCE(((SELECT auth.jwt()) ->> 'is_anonymous')::boolean, false) = false
    AND deleted_at IS NULL
    AND id IN (
      SELECT membership.workspace_id
      FROM public.workspace_memberships AS membership
      WHERE membership.user_id = (SELECT auth.uid())
        AND membership.deleted_at IS NULL
    )
  );

GRANT USAGE ON SCHEMA private TO authenticated;

CREATE OR REPLACE FUNCTION private.create_workspace_invitation(
  p_workspace_id uuid,
  p_invitee_email text
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
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_email text := lower(btrim(p_invitee_email));
  v_existing public.workspace_invitations%ROWTYPE;
  v_expires_at timestamptz;
  v_invitation_id uuid;
  v_invitee_user_id uuid;
  v_token text;
BEGIN
  SELECT membership.role
  INTO v_actor_role
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  JOIN auth.users AS actor
    ON actor.id = membership.user_id
  WHERE workspace.id = p_workspace_id
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

  IF v_email IS NULL
    OR v_email !~ '^[^[:space:]@]+@[^[:space:]@]+$'
    OR v_email ~ '[[:cntrl:]]'
    OR octet_length(v_email) > 320
  THEN
    RAISE EXCEPTION 'invalid invitation email'
      USING ERRCODE = '22023';
  END IF;

  SELECT auth_user.id
  INTO v_invitee_user_id
  FROM auth.users AS auth_user
  WHERE lower(btrim(auth_user.email)) = v_email
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false
  ORDER BY auth_user.created_at, auth_user.id
  LIMIT 1;

  IF EXISTS (
    SELECT 1
    FROM public.workspace_memberships AS membership
    JOIN auth.users AS member_user
      ON member_user.id = membership.user_id
    WHERE membership.workspace_id = p_workspace_id
      AND membership.deleted_at IS NULL
      AND lower(btrim(member_user.email)) = v_email
  ) THEN
    RAISE EXCEPTION 'workspace invitation not needed'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_existing
  FROM public.workspace_invitations AS invitation
  WHERE invitation.workspace_id = p_workspace_id
    AND invitation.invitee_email = v_email
    AND invitation.accepted_at IS NULL
    AND invitation.revoked_at IS NULL
  FOR UPDATE;

  IF FOUND AND v_existing.expires_at > now() THEN
    RETURN QUERY
    SELECT v_existing.id, NULL::text, v_existing.expires_at, false;
    RETURN;
  END IF;

  IF FOUND THEN
    UPDATE public.workspace_invitations
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now(),
      updated_at = now()
    WHERE id = v_existing.id;
  END IF;

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
    p_workspace_id,
    v_email,
    v_invitee_user_id,
    extensions.digest(v_token, 'sha256'),
    'member',
    v_actor_id,
    v_expires_at
  )
  RETURNING id INTO v_invitation_id;

  RETURN QUERY
  SELECT v_invitation_id, v_token, v_expires_at, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.accept_workspace_invitation(
  p_invitation_id uuid,
  p_invite_token text
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
  v_actor_id uuid := auth.uid();
  v_actor_email text;
  v_invitation public.workspace_invitations%ROWTYPE;
  v_membership public.workspace_memberships%ROWTYPE;
BEGIN
  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false;

  IF v_actor_email IS NULL
    OR p_invite_token IS NULL
    OR p_invite_token !~ '^[A-Za-z0-9_-]{43}$'
  THEN
    RAISE EXCEPTION 'workspace invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_invitation
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id
    AND invitation.token_hash = extensions.digest(p_invite_token, 'sha256')
  FOR UPDATE;

  IF NOT FOUND
    OR v_invitation.revoked_at IS NOT NULL
    OR v_invitation.expires_at <= now()
    OR v_invitation.invitee_email <> v_actor_email
    OR (
      v_invitation.invitee_user_id IS NOT NULL
      AND v_invitation.invitee_user_id <> v_actor_id
    )
    OR NOT EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = v_invitation.workspace_id
        AND workspace.kind = 'shared'
        AND workspace.deleted_at IS NULL
    )
  THEN
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

CREATE OR REPLACE FUNCTION private.revoke_workspace_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  revoked_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_invitation public.workspace_invitations%ROWTYPE;
  v_actor_role text;
  v_revoked_at timestamptz;
BEGIN
  SELECT invitation.*
  INTO v_invitation
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id
  FOR UPDATE;

  IF NOT FOUND THEN
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
  WHERE workspace.id = v_invitation.workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false;

  IF v_actor_role IS NULL THEN
    RAISE EXCEPTION 'workspace invitation operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_invitation.accepted_at IS NOT NULL THEN
    RAISE EXCEPTION 'accepted invitations require membership revocation'
      USING ERRCODE = '22023';
  END IF;

  IF v_invitation.revoked_at IS NULL THEN
    v_revoked_at := now();

    UPDATE public.workspace_invitations
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = v_revoked_at,
      updated_at = v_revoked_at
    WHERE id = v_invitation.id;
  ELSE
    v_revoked_at := v_invitation.revoked_at;
  END IF;

  RETURN QUERY
  SELECT v_invitation.id, v_revoked_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.revoke_workspace_membership(
  p_workspace_id uuid,
  p_user_id uuid
)
RETURNS TABLE (
  membership_id uuid,
  revoked_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_owner_user_id uuid;
  v_target public.workspace_memberships%ROWTYPE;
  v_revoked_at timestamptz;
BEGIN
  SELECT workspace.owner_user_id, membership.role
  INTO v_owner_user_id, v_actor_role
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  JOIN auth.users AS actor
    ON actor.id = membership.user_id
  WHERE workspace.id = p_workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false;

  IF v_actor_role IS NULL THEN
    RAISE EXCEPTION 'workspace membership operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT membership.*
  INTO v_target
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = p_workspace_id
    AND membership.user_id = p_user_id
  FOR UPDATE;

  IF NOT FOUND
    OR v_target.user_id = v_owner_user_id
    OR v_target.role = 'owner'
    OR (v_actor_role = 'admin' AND v_target.role <> 'member')
  THEN
    RAISE EXCEPTION 'workspace membership operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_target.deleted_at IS NULL THEN
    v_revoked_at := now();

    UPDATE public.workspace_memberships
    SET
      deleted_at = v_revoked_at,
      updated_at = v_revoked_at
    WHERE id = v_target.id;
  ELSE
    v_revoked_at := v_target.deleted_at;
  END IF;

  RETURN QUERY
  SELECT v_target.id, v_revoked_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_workspace_invitations(
  p_workspace_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invitee_email text,
  invitee_user_id uuid,
  invited_by_user_id uuid,
  created_at timestamptz,
  expires_at timestamptz,
  accepted_at timestamptz,
  revoked_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor
      ON actor.id = membership.user_id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = auth.uid()
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'workspace invitation operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    invitation.id,
    invitation.invitee_email,
    invitation.invitee_user_id,
    invitation.invited_by_user_id,
    invitation.created_at,
    invitation.expires_at,
    invitation.accepted_at,
    invitation.revoked_at
  FROM public.workspace_invitations AS invitation
  WHERE invitation.workspace_id = p_workspace_id
  ORDER BY invitation.created_at DESC, invitation.id;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_workspace_memberships(
  p_workspace_id uuid
)
RETURNS TABLE (
  membership_id uuid,
  user_id uuid,
  user_email text,
  role text,
  created_at timestamptz,
  deleted_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor
      ON actor.id = membership.user_id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = auth.uid()
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'workspace membership operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    membership.id,
    membership.user_id,
    lower(btrim(member_user.email)),
    membership.role,
    membership.created_at,
    membership.deleted_at
  FROM public.workspace_memberships AS membership
  LEFT JOIN auth.users AS member_user
    ON member_user.id = membership.user_id
  WHERE membership.workspace_id = p_workspace_id
  ORDER BY membership.created_at, membership.id;
END;
$$;

REVOKE ALL ON FUNCTION private.create_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.accept_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.revoke_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.revoke_workspace_membership(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_workspace_invitations(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_workspace_memberships(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.create_workspace_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.accept_workspace_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.revoke_workspace_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.revoke_workspace_membership(uuid, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_workspace_invitations(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_workspace_memberships(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.create_workspace_invitation(
  p_workspace_id uuid,
  p_invitee_email text
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
  FROM private.create_workspace_invitation(p_workspace_id, p_invitee_email);
$$;

CREATE OR REPLACE FUNCTION public.accept_workspace_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  workspace_id uuid,
  membership_id uuid
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.accept_workspace_invitation(p_invitation_id, p_invite_token);
$$;

CREATE OR REPLACE FUNCTION public.revoke_workspace_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  revoked_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.revoke_workspace_invitation(p_invitation_id);
$$;

CREATE OR REPLACE FUNCTION public.revoke_workspace_membership(
  p_workspace_id uuid,
  p_user_id uuid
)
RETURNS TABLE (
  membership_id uuid,
  revoked_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.revoke_workspace_membership(p_workspace_id, p_user_id);
$$;

CREATE OR REPLACE FUNCTION public.list_workspace_invitations(
  p_workspace_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invitee_email text,
  invitee_user_id uuid,
  invited_by_user_id uuid,
  created_at timestamptz,
  expires_at timestamptz,
  accepted_at timestamptz,
  revoked_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_workspace_invitations(p_workspace_id);
$$;

CREATE OR REPLACE FUNCTION public.list_workspace_memberships(
  p_workspace_id uuid
)
RETURNS TABLE (
  membership_id uuid,
  user_id uuid,
  user_email text,
  role text,
  created_at timestamptz,
  deleted_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_workspace_memberships(p_workspace_id);
$$;

REVOKE ALL ON FUNCTION public.create_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.accept_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.revoke_workspace_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.revoke_workspace_membership(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_workspace_invitations(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_workspace_memberships(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.create_workspace_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.accept_workspace_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.revoke_workspace_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.revoke_workspace_membership(uuid, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_workspace_invitations(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_workspace_memberships(uuid)
  TO authenticated;
