-- Role changes were owner-only, which left a single point of failure: an owner
-- on leave meant nobody could appoint an admin. Admins may now promote, capped
-- at admin so ownership is never granted sideways, and demotion of an admin
-- stays with the owner so peers cannot strip each other.
CREATE OR REPLACE FUNCTION private.set_workspace_membership_role(
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
DECLARE
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_owner_user_id uuid;
  v_target public.workspace_memberships%ROWTYPE;
BEGIN
  IF p_role IS NULL OR p_role NOT IN ('admin', 'member') THEN
    RAISE EXCEPTION 'invalid workspace role'
      USING ERRCODE = '22023';
  END IF;

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
    AND COALESCE(actor.is_anonymous, false) = false
  FOR UPDATE OF workspace;

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
    OR v_target.deleted_at IS NOT NULL
    OR v_target.user_id = v_owner_user_id
    OR v_target.role = 'owner'
  THEN
    RAISE EXCEPTION 'workspace membership operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  -- Admins may raise a member to admin and nothing else: demoting an admin
  -- (including themselves) stays an owner decision.
  IF v_actor_role = 'admin'
    AND NOT (v_target.role = 'member' AND p_role = 'admin')
  THEN
    RAISE EXCEPTION 'workspace membership operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_target.role <> p_role THEN
    UPDATE public.workspace_memberships
    SET
      role = p_role,
      updated_at = now()
    WHERE id = v_target.id;
  END IF;

  RETURN QUERY
  SELECT v_target.id, p_role;
END;
$$;
