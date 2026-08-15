CREATE OR REPLACE FUNCTION private.purge_workspace_e2ee_grants_for_membership()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF TG_OP = 'UPDATE'
    AND NOT (OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL)
  THEN
    RETURN NEW;
  END IF;

  DELETE FROM public.workspace_e2ee_key_grants AS grant_row
  WHERE grant_row.workspace_id = OLD.workspace_id
    AND grant_row.member_user_id = OLD.user_id;

  -- The removed member can retain an already-unwrapped key. Retiring that
  -- generation forces every remaining client to stop shared-workspace sync
  -- until a manager publishes a fresh key for the reduced member set.
  UPDATE public.workspace_e2ee_keys AS workspace_key
  SET retired_at = now()
  WHERE workspace_key.workspace_id = OLD.workspace_id
    AND workspace_key.retired_at IS NULL;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.purge_workspace_e2ee_grants_for_membership()
  FROM PUBLIC, anon, authenticated;
