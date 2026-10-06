BEGIN;

SET LOCAL lock_timeout = '30s';

-- Sync access remains gated by Pro entitlement. Raising the included allowance
-- applies to existing accounts without changing devices or purchased add-ons.
CREATE OR REPLACE FUNCTION private.sync_device_limit(p_actor_user_id uuid)
RETURNS integer
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT 5 + private.sync_device_addon_count(p_actor_user_id);
$$;

REVOKE ALL ON FUNCTION private.sync_device_limit(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.sync_device_limit(uuid) TO service_role;

COMMIT;
