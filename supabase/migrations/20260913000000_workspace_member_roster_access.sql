-- Every active member can read the current roster. Membership history, including
-- removed members, stays manager-only through list_workspace_memberships.
CREATE OR REPLACE FUNCTION public.list_workspace_members_with_profiles(
  p_workspace_id uuid
)
RETURNS TABLE (
  membership_id uuid,
  user_id uuid,
  user_email text,
  role text,
  created_at timestamptz,
  deleted_at timestamptz,
  user_name text,
  user_avatar_url text
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
    membership.deleted_at,
    COALESCE(
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'full_name'), ''),
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'name'), '')
    ),
    COALESCE(
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'avatar_url'), ''),
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'picture'), '')
    )
  FROM public.workspace_memberships AS membership
  LEFT JOIN auth.users AS member_user
    ON member_user.id = membership.user_id
  WHERE membership.workspace_id = p_workspace_id
    AND membership.deleted_at IS NULL
  ORDER BY membership.created_at, membership.id;
END;
$$;
