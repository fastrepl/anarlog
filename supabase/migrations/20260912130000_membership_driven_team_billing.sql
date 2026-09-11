BEGIN;

-- Stripe quantity is a billing snapshot, not permission to invite or join.
DROP TRIGGER on_workspace_invitation_seat_limit ON public.workspace_invitations;
DROP TRIGGER on_workspace_membership_seat_limit ON public.workspace_memberships;

CREATE OR REPLACE FUNCTION private.workspace_seat_usage(p_workspace_id uuid)
RETURNS TABLE (seat_limit integer, used_seats integer)
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = ''
AS $$
  SELECT workspace.seat_limit, CASE WHEN workspace.deleted_at IS NULL
    THEN count(membership.user_id)::integer ELSE 0 END
  FROM public.workspaces AS workspace
  LEFT JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id AND membership.deleted_at IS NULL
  WHERE workspace.id = p_workspace_id
  GROUP BY workspace.id;
$$;

CREATE TABLE private.workspace_seat_billing_events (
  id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  workspace_id uuid NOT NULL,
  customer_id text NOT NULL,
  quantity integer NOT NULL CHECK (quantity >= 0),
  occurred_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  processed_at timestamptz,
  attempts integer NOT NULL DEFAULT 0,
  next_attempt_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX workspace_seat_billing_pending ON private.workspace_seat_billing_events (id)
  WHERE processed_at IS NULL;
CREATE INDEX workspace_seat_billing_order ON private.workspace_seat_billing_events (workspace_id, id)
  WHERE processed_at IS NULL;
REVOKE ALL ON private.workspace_seat_billing_events FROM PUBLIC, anon, authenticated, service_role;

-- Serialize every membership writer, including direct provisioning and deletion.
CREATE FUNCTION private.lock_workspace_seat_billing()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
BEGIN
  PERFORM 1 FROM public.workspaces
  WHERE id = COALESCE(NEW.workspace_id, OLD.workspace_id) FOR UPDATE;
  RETURN COALESCE(NEW, OLD);
END;
$$;
REVOKE ALL ON FUNCTION private.lock_workspace_seat_billing() FROM PUBLIC, anon, authenticated;
CREATE TRIGGER before_workspace_seat_billing
BEFORE INSERT OR UPDATE OR DELETE ON public.workspace_memberships
FOR EACH ROW EXECUTE FUNCTION private.lock_workspace_seat_billing();

CREATE FUNCTION private.enqueue_workspace_seat_billing()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
DECLARE
  v_workspace_id uuid;
BEGIN
  IF TG_TABLE_NAME = 'workspace_memberships' THEN
    IF TG_OP = 'UPDATE' AND (OLD.deleted_at IS NULL) = (NEW.deleted_at IS NULL) THEN
      RETURN NEW;
    END IF;
    v_workspace_id := COALESCE(NEW.workspace_id, OLD.workspace_id);
  ELSE
    v_workspace_id := NEW.id;
  END IF;
  INSERT INTO private.workspace_seat_billing_events (workspace_id, customer_id, quantity)
  SELECT workspace.id, workspace.stripe_customer_id, usage.used_seats
  FROM public.workspaces AS workspace
  CROSS JOIN LATERAL private.workspace_seat_usage(workspace.id) AS usage
  WHERE workspace.id = v_workspace_id AND workspace.kind = 'shared'
    AND workspace.stripe_customer_id IS NOT NULL;
  RETURN COALESCE(NEW, OLD);
END;
$$;
REVOKE ALL ON FUNCTION private.enqueue_workspace_seat_billing() FROM PUBLIC, anon, authenticated;
CREATE TRIGGER after_workspace_seat_billing
AFTER INSERT OR UPDATE OR DELETE ON public.workspace_memberships
FOR EACH ROW EXECUTE FUNCTION private.enqueue_workspace_seat_billing();
CREATE TRIGGER after_workspace_deleted_seat_billing
AFTER UPDATE OF deleted_at ON public.workspaces
FOR EACH ROW WHEN (NEW.deleted_at IS DISTINCT FROM OLD.deleted_at)
EXECUTE FUNCTION private.enqueue_workspace_seat_billing();

CREATE FUNCTION private.reconcile_workspace_seat_snapshot()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = '' AS $$
BEGIN
  -- Membership events already carry exact change times. A Stripe echo must not
  -- replace them; reconcile external quantity edits only after the queue drains.
  IF NEW.stripe_customer_id IS NOT NULL AND NEW.deleted_at IS NULL THEN
    INSERT INTO private.workspace_seat_billing_events (workspace_id, customer_id, quantity)
    SELECT NEW.id, NEW.stripe_customer_id, usage.used_seats
    FROM private.workspace_seat_usage(NEW.id) AS usage
    WHERE (NEW.stripe_customer_id IS DISTINCT FROM OLD.stripe_customer_id
      OR NEW.seat_limit IS DISTINCT FROM usage.used_seats)
      AND NOT EXISTS (
        SELECT 1 FROM private.workspace_seat_billing_events
        WHERE workspace_id = NEW.id AND processed_at IS NULL
      );
  END IF;
  RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION private.reconcile_workspace_seat_snapshot() FROM PUBLIC, anon, authenticated;
CREATE TRIGGER after_workspace_billing_customer
AFTER UPDATE OF stripe_customer_id, seat_limit ON public.workspaces
FOR EACH ROW WHEN (NEW.stripe_customer_id IS DISTINCT FROM OLD.stripe_customer_id
  OR NEW.seat_limit IS DISTINCT FROM OLD.seat_limit)
EXECUTE FUNCTION private.reconcile_workspace_seat_snapshot();

CREATE OR REPLACE FUNCTION private.auto_join_email_workspace()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_domain text := private.email_auto_join_domain(NEW.id);
  v_workspace_id uuid;
BEGIN
  DELETE FROM public.workspace_email_auto_join
  WHERE enabled_by_user_id = NEW.id AND domain IS DISTINCT FROM v_domain;
  IF v_domain IS NULL THEN RETURN NEW; END IF;

  SELECT workspace_id INTO v_workspace_id
  FROM public.workspace_email_auto_join WHERE domain = v_domain;
  IF v_workspace_id IS NULL THEN RETURN NEW; END IF;

  -- Invitation acceptance and membership changes take this same lock.
  PERFORM 1 FROM public.workspaces WHERE id = v_workspace_id FOR UPDATE;
  IF NOT EXISTS (
    SELECT 1 FROM public.workspaces AS workspace
    JOIN public.workspace_email_auto_join AS joining ON joining.workspace_id = workspace.id
    WHERE workspace.id = v_workspace_id
      AND workspace.kind = 'shared' AND workspace.deleted_at IS NULL
      AND joining.domain = v_domain
      AND joining.enabled_by_user_id = workspace.owner_user_id
      AND private.email_auto_join_domain(workspace.owner_user_id) = v_domain
      AND private.workspace_has_capability(workspace.id, 'team.manage_members')
  ) OR EXISTS (
    -- A removed member must use an explicit invitation to rejoin.
    SELECT 1 FROM public.workspace_memberships
    WHERE workspace_id = v_workspace_id AND user_id = NEW.id
  ) OR EXISTS (
    SELECT 1 FROM public.workspace_verified_domains
    WHERE domain = v_domain AND workspace_id <> v_workspace_id
  ) THEN
    RETURN NEW;
  END IF;

  INSERT INTO public.workspace_memberships (workspace_id, user_id, role)
  VALUES (v_workspace_id, NEW.id, 'member') ON CONFLICT DO NOTHING;
  UPDATE public.workspace_invitations
  SET accepted_at = now(), invitee_user_id = NEW.id, updated_at = now()
  WHERE workspace_id = v_workspace_id AND invitee_email = lower(btrim(NEW.email))
    AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now();
  RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION private.auto_join_email_workspace() FROM PUBLIC, anon, authenticated;


-- Reconcile existing teams from rollout time, never retroactively before rollout.
INSERT INTO private.workspace_seat_billing_events (workspace_id, customer_id, quantity)
SELECT workspace.id, workspace.stripe_customer_id, usage.used_seats
FROM public.workspaces AS workspace
CROSS JOIN LATERAL private.workspace_seat_usage(workspace.id) AS usage
WHERE workspace.kind = 'shared' AND workspace.deleted_at IS NULL
  AND workspace.stripe_customer_id IS NOT NULL;

COMMIT;
