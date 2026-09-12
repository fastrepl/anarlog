-- Preserve the original roster RPC for older clients.
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
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    membership.*,
    COALESCE(
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'full_name'), ''),
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'name'), '')
    ),
    COALESCE(
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'avatar_url'), ''),
      NULLIF(btrim(member_user.raw_user_meta_data ->> 'picture'), '')
    )
  FROM private.list_workspace_memberships(p_workspace_id) AS membership
  LEFT JOIN auth.users AS member_user ON member_user.id = membership.user_id
  WHERE membership.deleted_at IS NULL
  ORDER BY membership.created_at, membership.membership_id;
$$;

REVOKE ALL ON FUNCTION public.list_workspace_members_with_profiles(uuid)
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.list_workspace_members_with_profiles(uuid)
  TO authenticated;
