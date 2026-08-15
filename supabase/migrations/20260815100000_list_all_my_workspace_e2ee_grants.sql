CREATE OR REPLACE FUNCTION private.list_all_my_workspace_e2ee_grants()
RETURNS TABLE (
  workspace_id uuid,
  key_id text,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  is_active boolean,
  created_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS actor
    WHERE actor.id = v_actor_id
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'E2EE key operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    grant_row.workspace_id,
    grant_row.key_id,
    grant_row.ephemeral_public_key,
    grant_row.nonce,
    grant_row.ciphertext,
    workspace_key.retired_at IS NULL,
    grant_row.created_at
  FROM public.workspace_e2ee_key_grants AS grant_row
  JOIN public.workspace_e2ee_keys AS workspace_key
    ON workspace_key.workspace_id = grant_row.workspace_id
    AND workspace_key.key_id = grant_row.key_id
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = grant_row.workspace_id
    AND membership.user_id = grant_row.member_user_id
    AND membership.deleted_at IS NULL
  JOIN public.workspaces AS workspace
    ON workspace.id = grant_row.workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
  WHERE grant_row.member_user_id = v_actor_id
  ORDER BY grant_row.workspace_id, workspace_key.created_at, grant_row.created_at;
END;
$$;

REVOKE ALL ON FUNCTION private.list_all_my_workspace_e2ee_grants()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.list_all_my_workspace_e2ee_grants()
  TO authenticated;

CREATE OR REPLACE FUNCTION public.list_all_my_workspace_e2ee_grants()
RETURNS TABLE (
  workspace_id uuid,
  key_id text,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  is_active boolean,
  created_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT * FROM private.list_all_my_workspace_e2ee_grants();
$$;

REVOKE ALL ON FUNCTION public.list_all_my_workspace_e2ee_grants()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.list_all_my_workspace_e2ee_grants()
  TO authenticated;
