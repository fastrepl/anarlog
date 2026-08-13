CREATE OR REPLACE FUNCTION private.protected_create_workspace(
  p_name text
)
RETURNS TABLE (
  workspace_id uuid,
  membership_id uuid
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.create_workspace(p_name);
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_rename_workspace(
  p_workspace_id uuid,
  p_name text
)
RETURNS TABLE (
  workspace_id uuid,
  workspace_name text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.rename_workspace(p_workspace_id, p_name);
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_set_workspace_membership_role(
  p_workspace_id uuid,
  p_user_id uuid,
  p_role text
)
RETURNS TABLE (
  membership_id uuid,
  membership_role text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.set_workspace_membership_role(
    p_workspace_id,
    p_user_id,
    p_role
  );
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_transfer_workspace_ownership(
  p_workspace_id uuid,
  p_user_id uuid
)
RETURNS TABLE (
  workspace_id uuid,
  owner_user_id uuid
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.transfer_workspace_ownership(p_workspace_id, p_user_id);
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_create_workspace_invitation(
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
BEGIN
  PERFORM private.require_hyprnote_pro_entitlement();

  RETURN QUERY
  SELECT *
  FROM private.create_workspace_invitation(p_workspace_id, p_invitee_email);
END;
$$;

-- Keep invitation acceptance and access-reducing operations available so
-- invited users can join and expired subscribers can leave or clean up a Team.
REVOKE ALL ON FUNCTION private.create_workspace(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.rename_workspace(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.set_workspace_membership_role(uuid, uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.transfer_workspace_ownership(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.create_workspace_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;

REVOKE ALL ON FUNCTION private.protected_create_workspace(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_rename_workspace(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_set_workspace_membership_role(
  uuid,
  uuid,
  text
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_transfer_workspace_ownership(
  uuid,
  uuid
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.protected_create_workspace_invitation(
  uuid,
  text
) FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.protected_create_workspace(text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_rename_workspace(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_set_workspace_membership_role(
  uuid,
  uuid,
  text
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_transfer_workspace_ownership(
  uuid,
  uuid
) TO authenticated;
GRANT EXECUTE ON FUNCTION private.protected_create_workspace_invitation(
  uuid,
  text
) TO authenticated;

CREATE OR REPLACE FUNCTION public.create_workspace(
  p_name text
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
  FROM private.protected_create_workspace(p_name);
$$;

CREATE OR REPLACE FUNCTION public.rename_workspace(
  p_workspace_id uuid,
  p_name text
)
RETURNS TABLE (
  workspace_id uuid,
  workspace_name text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_rename_workspace(p_workspace_id, p_name);
$$;

CREATE OR REPLACE FUNCTION public.set_workspace_membership_role(
  p_workspace_id uuid,
  p_user_id uuid,
  p_role text
)
RETURNS TABLE (
  membership_id uuid,
  membership_role text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_set_workspace_membership_role(
    p_workspace_id,
    p_user_id,
    p_role
  );
$$;

CREATE OR REPLACE FUNCTION public.transfer_workspace_ownership(
  p_workspace_id uuid,
  p_user_id uuid
)
RETURNS TABLE (
  workspace_id uuid,
  owner_user_id uuid
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.protected_transfer_workspace_ownership(
    p_workspace_id,
    p_user_id
  );
$$;

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
  FROM private.protected_create_workspace_invitation(
    p_workspace_id,
    p_invitee_email
  );
$$;
