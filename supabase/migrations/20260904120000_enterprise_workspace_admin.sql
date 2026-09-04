-- Keep workspace administration and branded sharing on Enterprise only.
-- A retained slug becomes inactive immediately when Enterprise billing ends.

BEGIN;

SET LOCAL lock_timeout = '30s';

CREATE OR REPLACE FUNCTION private.workspace_capabilities(
  p_workspace_id uuid
)
RETURNS text[]
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  WITH paid_features AS (
    SELECT
      COALESCE(bool_or(entitlement.lookup_key = 'hyprnote_team'), false)
        AS has_team,
      COALESCE(bool_or(entitlement.lookup_key = 'hyprnote_enterprise'), false)
        AS has_enterprise
    FROM public.workspaces AS workspace
    JOIN stripe.subscriptions AS subscription
      ON subscription.customer = workspace.stripe_customer_id
      AND subscription.status IN ('trialing', 'active')
    JOIN stripe.active_entitlements AS entitlement
      ON entitlement.customer = workspace.stripe_customer_id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
  )
  SELECT CASE
    WHEN paid_features.has_enterprise THEN ARRAY[
      'team.shared_notes',
      'team.manage_workspace',
      'team.manage_members',
      'team.manage_policies',
      'team.view_usage',
      'team.custom_subdomain',
      'enterprise.sso',
      'enterprise.scim',
      'enterprise.retention',
      'enterprise.audit_logs',
      'enterprise.capture'
    ]::text[]
    WHEN paid_features.has_team THEN ARRAY[
      'team.shared_notes',
      'team.manage_workspace',
      'team.manage_members'
    ]::text[]
    ELSE ARRAY[]::text[]
  END
  FROM paid_features;
$$;

CREATE OR REPLACE FUNCTION private.set_workspace_share_slug(
  p_workspace_id uuid,
  p_slug text
)
RETURNS TABLE (
  workspace_id uuid,
  workspace_share_slug text,
  share_base_url text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_slug text := lower(btrim(p_slug));
BEGIN
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.custom_subdomain'
  );

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
    RAISE EXCEPTION 'invalid workspace subdomain'
      USING ERRCODE = '22023';
  END IF;

  BEGIN
    UPDATE public.workspaces
    SET
      share_slug = v_slug,
      updated_at = now()
    WHERE id = p_workspace_id;
  EXCEPTION
    WHEN unique_violation THEN
      RAISE EXCEPTION 'workspace subdomain is already taken'
        USING ERRCODE = '23505';
  END;

  RETURN QUERY
  SELECT
    p_workspace_id,
    v_slug,
    format('https://%s.anarlog.so', v_slug);
END;
$$;

CREATE OR REPLACE FUNCTION private.get_session_share_workspace_slug(
  p_share_id uuid
)
RETURNS TABLE (
  workspace_share_slug text
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
BEGIN
  IF NOT private.is_session_share_manager(p_share_id, v_actor_id) THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT CASE
    WHEN private.workspace_has_capability(
      workspace.id,
      'team.custom_subdomain'
    ) THEN workspace.share_slug
    ELSE NULL
  END
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;
END;
$$;

CREATE OR REPLACE FUNCTION public.workspace_share_slug_is_active(
  p_slug text
)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    WHERE workspace.share_slug = lower(btrim(p_slug))
      AND lower(btrim(p_slug)) <> 'models'
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND private.workspace_has_capability(
        workspace.id,
        'team.custom_subdomain'
      )
  );
$$;

REVOKE ALL ON FUNCTION public.workspace_share_slug_is_active(text)
  FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.workspace_share_slug_is_active(text)
  TO anon, authenticated, service_role;

COMMIT;
