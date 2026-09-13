-- Roster visibility belongs to every active workspace member.
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
