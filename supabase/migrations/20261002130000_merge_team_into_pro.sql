-- Pro workspace subscriptions include collaboration; keep legacy Team entitlements.
-- Resolve access from the workspace customer so personal Pro cannot unlock unpaid seats.

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
      COALESCE(bool_or(entitlement.lookup_key IN ('hyprnote_pro', 'hyprnote_team')), false)
        AS has_pro,
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
      'team.shared_resources',
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
    WHEN paid_features.has_pro THEN ARRAY[
      'team.shared_notes',
      'team.shared_resources',
      'team.manage_workspace',
      'team.manage_members'
    ]::text[]
    ELSE ARRAY[]::text[]
  END
  FROM paid_features;
$$;

COMMIT;
