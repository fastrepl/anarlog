-- Let Enterprise managers check a branded sharing subdomain before claiming it.

BEGIN;

CREATE OR REPLACE FUNCTION public.check_workspace_share_slug_availability(
  p_workspace_id uuid,
  p_slug text
)
RETURNS text
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_slug text := lower(btrim(p_slug));
BEGIN
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.custom_subdomain'
  );

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
      AND membership.user_id = v_actor_id
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'workspace subdomain operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_slug IS NULL
    OR v_slug !~ '^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$'
    OR v_slug IN (
      'admin',
      'api',
      'app',
      'assets',
      'auth',
      'cdn',
      'desktop',
      'dev',
      'docs',
      'mail',
      'models',
      'staging',
      'static',
      'status',
      'support',
      'www'
    )
  THEN
    RETURN 'invalid';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    WHERE workspace.share_slug = v_slug
      AND workspace.id <> p_workspace_id
  ) THEN
    RETURN 'taken';
  END IF;

  RETURN 'available';
END;
$$;

REVOKE ALL ON FUNCTION public.check_workspace_share_slug_availability(
  uuid,
  text
) FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.check_workspace_share_slug_availability(
  uuid,
  text
) TO authenticated;

COMMIT;
