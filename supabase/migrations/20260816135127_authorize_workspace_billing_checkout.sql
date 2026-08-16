BEGIN;

CREATE OR REPLACE FUNCTION private.get_workspace_billing_checkout_context(
  p_workspace_id uuid
)
RETURNS TABLE (
  workspace_name text,
  stripe_customer_id text,
  used_seats integer
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RETURN QUERY
  SELECT
    workspace.name,
    workspace.stripe_customer_id,
    usage.used_seats
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  JOIN auth.users AS actor
    ON actor.id = membership.user_id
  CROSS JOIN LATERAL private.workspace_seat_usage(workspace.id) AS usage
  WHERE workspace.id = p_workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
    AND membership.user_id = auth.uid()
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'workspace billing operation not permitted'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.get_workspace_billing_checkout_context(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
GRANT EXECUTE ON FUNCTION private.get_workspace_billing_checkout_context(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.get_workspace_billing_checkout_context(
  p_workspace_id uuid
)
RETURNS TABLE (
  workspace_name text,
  stripe_customer_id text,
  used_seats integer
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.get_workspace_billing_checkout_context(p_workspace_id);
$$;

REVOKE ALL ON FUNCTION public.get_workspace_billing_checkout_context(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
GRANT EXECUTE ON FUNCTION public.get_workspace_billing_checkout_context(uuid)
  TO authenticated;

COMMIT;
