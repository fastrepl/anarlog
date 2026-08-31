-- Resolve paid workspace features from the workspace's own Stripe customer.
-- Personal entitlements must never unlock another workspace's controls.

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
      'team.manage_members',
      'team.manage_policies',
      'team.view_usage',
      'team.custom_subdomain'
    ]::text[]
    ELSE ARRAY[]::text[]
  END
  FROM paid_features;
$$;

CREATE OR REPLACE FUNCTION private.workspace_has_capability(
  p_workspace_id uuid,
  p_capability text
)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT p_capability = ANY (private.workspace_capabilities(p_workspace_id));
$$;

CREATE OR REPLACE FUNCTION private.require_workspace_capability(
  p_workspace_id uuid,
  p_capability text
)
RETURNS void
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT private.workspace_has_capability(p_workspace_id, p_capability) THEN
    RAISE EXCEPTION 'workspace capability required: %', p_capability
      USING ERRCODE = '42501';
  END IF;
END;
$$;

CREATE OR REPLACE FUNCTION private.require_workspace_or_pro_capability(
  p_workspace_id uuid,
  p_workspace_capability text
)
RETURNS void
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_workspace_kind text;
BEGIN
  SELECT workspace.kind
  INTO v_workspace_kind
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id
    AND workspace.deleted_at IS NULL;

  IF v_workspace_kind = 'shared' THEN
    PERFORM private.require_workspace_capability(
      p_workspace_id,
      p_workspace_capability
    );
  ELSIF v_workspace_kind IS NOT NULL THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  ELSE
    RAISE EXCEPTION 'workspace operation not permitted'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.workspace_capabilities(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.workspace_has_capability(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.require_workspace_capability(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.require_workspace_or_pro_capability(uuid, text)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.get_workspace_access(
  p_workspace_id uuid
)
RETURNS TABLE (
  workspace_role text,
  workspace_tier text,
  capabilities text[],
  seat_limit integer,
  used_seats integer
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_capabilities text[];
BEGIN
  PERFORM private.require_workspace_member(p_workspace_id);
  v_capabilities := private.workspace_capabilities(p_workspace_id);

  RETURN QUERY
  SELECT
    membership.role,
    CASE
      WHEN 'enterprise.sso' = ANY (v_capabilities) THEN 'enterprise'
      WHEN 'team.shared_notes' = ANY (v_capabilities) THEN 'team'
      ELSE 'free'
    END,
    v_capabilities,
    usage.seat_limit,
    usage.used_seats
  FROM public.workspace_memberships AS membership
  CROSS JOIN LATERAL private.workspace_seat_usage(p_workspace_id) AS usage
  WHERE membership.workspace_id = p_workspace_id
    AND membership.user_id = auth.uid()
    AND membership.deleted_at IS NULL;
END;
$$;

REVOKE ALL ON FUNCTION public.get_workspace_access(uuid)
  FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.get_workspace_access(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION private.protected_rename_workspace(
  p_workspace_id uuid,
  p_name text
)
RETURNS TABLE (
  workspace_id uuid,
  workspace_name text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_workspace_id uuid;
  v_workspace_name text;
BEGIN
  SELECT renamed.workspace_id, renamed.workspace_name
  INTO v_workspace_id, v_workspace_name
  FROM private.rename_workspace(p_workspace_id, p_name) AS renamed;

  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_workspace'
  );

  RETURN QUERY SELECT v_workspace_id, v_workspace_name;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_set_workspace_membership_role(
  p_workspace_id uuid,
  p_user_id uuid,
  p_role text
)
RETURNS TABLE (
  membership_id uuid,
  membership_role text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_membership_id uuid;
  v_membership_role text;
BEGIN
  SELECT changed.membership_id, changed.membership_role
  INTO v_membership_id, v_membership_role
  FROM private.set_workspace_membership_role(
    p_workspace_id,
    p_user_id,
    p_role
  ) AS changed;

  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_members'
  );

  RETURN QUERY SELECT v_membership_id, v_membership_role;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_transfer_workspace_ownership(
  p_workspace_id uuid,
  p_user_id uuid
)
RETURNS TABLE (
  workspace_id uuid,
  owner_user_id uuid
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_workspace_id uuid;
  v_owner_user_id uuid;
BEGIN
  SELECT transferred.workspace_id, transferred.owner_user_id
  INTO v_workspace_id, v_owner_user_id
  FROM private.transfer_workspace_ownership(
    p_workspace_id,
    p_user_id
  ) AS transferred;

  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_members'
  );

  RETURN QUERY SELECT v_workspace_id, v_owner_user_id;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_create_workspace_invitation(
  p_workspace_id uuid,
  p_invitee_email text
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_invitation_id uuid;
  v_invite_token text;
  v_invitation_expires_at timestamptz;
  v_was_created boolean;
BEGIN
  SELECT
    invitation.invitation_id,
    invitation.invite_token,
    invitation.invitation_expires_at,
    invitation.was_created
  INTO
    v_invitation_id,
    v_invite_token,
    v_invitation_expires_at,
    v_was_created
  FROM private.create_workspace_invitation(
    p_workspace_id,
    p_invitee_email
  ) AS invitation;

  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_members'
  );

  RETURN QUERY SELECT
    v_invitation_id,
    v_invite_token,
    v_invitation_expires_at,
    v_was_created;
END;
$$;

CREATE OR REPLACE FUNCTION private.protected_resend_workspace_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_workspace_id uuid;
BEGIN
  SELECT invitation.workspace_id
  INTO v_workspace_id
  FROM public.workspace_invitations AS invitation
  WHERE invitation.id = p_invitation_id;

  PERFORM private.require_workspace_capability(
    v_workspace_id,
    'team.manage_members'
  );

  RETURN QUERY
  SELECT *
  FROM private.resend_workspace_invitation(p_invitation_id);
END;
$$;

CREATE OR REPLACE FUNCTION private.set_workspace_logo(
  p_workspace_id uuid,
  p_logo_data text
)
RETURNS TABLE (
  workspace_id uuid,
  workspace_logo_data text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_actor_role text;
  v_logo_data text := p_logo_data;
BEGIN
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_workspace'
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
    RAISE EXCEPTION 'workspace logo operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_logo_data IS NOT NULL THEN
    v_logo_data := btrim(v_logo_data);
    IF char_length(v_logo_data) < 30
      OR char_length(v_logo_data) > 120000
      OR v_logo_data !~ '^data:image/jpeg;base64,[A-Za-z0-9+/]+={0,2}$'
    THEN
      RAISE EXCEPTION 'invalid workspace logo'
        USING ERRCODE = '22023';
    END IF;
  END IF;

  UPDATE public.workspaces
  SET
    logo_data = v_logo_data,
    updated_at = now()
  WHERE id = p_workspace_id;

  RETURN QUERY
  SELECT p_workspace_id, v_logo_data;
END;
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
      'dev',
      'docs',
      'mail',
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

CREATE OR REPLACE FUNCTION public.get_workspace_usage_overview(
  p_workspace_id uuid
)
RETURNS TABLE (
  member_count integer,
  pending_invitations integer,
  enrolled_devices integer,
  shares_created_30d integer,
  share_access_events_30d integer,
  seat_limit integer,
  used_seats integer,
  is_billed boolean
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.view_usage'
  );

  RETURN QUERY
  SELECT
    (
      SELECT count(*)::integer
      FROM public.workspace_memberships AS membership
      WHERE membership.workspace_id = p_workspace_id
        AND membership.deleted_at IS NULL
    ),
    (
      SELECT count(*)::integer
      FROM public.workspace_invitations AS invitation
      WHERE invitation.workspace_id = p_workspace_id
        AND invitation.accepted_at IS NULL
        AND invitation.revoked_at IS NULL
        AND invitation.expires_at > now()
    ),
    (
      SELECT count(*)::integer
      FROM public.sync_devices AS device
      JOIN public.workspace_memberships AS membership
        ON membership.user_id = device.user_id
      WHERE membership.workspace_id = p_workspace_id
        AND membership.deleted_at IS NULL
    ),
    (
      SELECT count(*)::integer
      FROM public.session_shares AS share
      WHERE share.workspace_id = p_workspace_id
        AND share.created_at >= now() - interval '30 days'
    ),
    (
      SELECT count(*)::integer
      FROM public.session_access_events AS event
      JOIN public.session_shares AS share
        ON share.id = event.share_id
      WHERE share.workspace_id = p_workspace_id
        AND event.created_at >= now() - interval '30 days'
    ),
    usage.seat_limit,
    usage.used_seats,
    true
  FROM private.workspace_seat_usage(p_workspace_id) AS usage;
END;
$$;

CREATE OR REPLACE FUNCTION public.set_workspace_policy(
  p_workspace_id uuid,
  p_allowed_share_scopes text[],
  p_default_share_scope text,
  p_retention_days integer,
  p_model_training_opt_out boolean,
  p_consent_notification_enabled boolean,
  p_require_sso boolean
)
RETURNS TABLE (
  workspace_id uuid,
  allowed_share_scopes text[],
  default_share_scope text,
  retention_days integer,
  model_training_opt_out boolean,
  consent_notification_enabled boolean,
  require_sso boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
#variable_conflict use_column
DECLARE
  v_existing public.workspace_policies%ROWTYPE;
BEGIN
  PERFORM private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'team.manage_policies'
  );

  SELECT policy.*
  INTO v_existing
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = p_workspace_id;

  IF COALESCE(p_require_sso, false)
    AND NOT COALESCE(v_existing.require_sso, false)
  THEN
    PERFORM private.require_workspace_capability(
      p_workspace_id,
      'enterprise.sso'
    );
  END IF;

  IF p_retention_days IS NOT NULL
    AND p_retention_days IS DISTINCT FROM v_existing.retention_days
  THEN
    PERFORM private.require_workspace_capability(
      p_workspace_id,
      'enterprise.retention'
    );
  END IF;

  IF COALESCE(p_require_sso, false)
    AND NOT EXISTS (
      SELECT 1
      FROM public.workspace_verified_domains AS claimed
      WHERE claimed.workspace_id = p_workspace_id
    )
  THEN
    RAISE EXCEPTION 'claim an email domain before requiring SSO'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.workspace_policies (
    workspace_id,
    allowed_share_scopes,
    default_share_scope,
    retention_days,
    model_training_opt_out,
    consent_notification_enabled,
    require_sso,
    updated_at
  ) VALUES (
    p_workspace_id,
    p_allowed_share_scopes,
    p_default_share_scope,
    p_retention_days,
    COALESCE(p_model_training_opt_out, true),
    COALESCE(p_consent_notification_enabled, true),
    COALESCE(p_require_sso, false),
    now()
  )
  ON CONFLICT (workspace_id) DO UPDATE SET
    allowed_share_scopes = EXCLUDED.allowed_share_scopes,
    default_share_scope = EXCLUDED.default_share_scope,
    retention_days = EXCLUDED.retention_days,
    model_training_opt_out = EXCLUDED.model_training_opt_out,
    consent_notification_enabled = EXCLUDED.consent_notification_enabled,
    require_sso = EXCLUDED.require_sso,
    updated_at = now();

  RETURN QUERY
  SELECT
    policy.workspace_id,
    policy.allowed_share_scopes,
    policy.default_share_scope,
    policy.retention_days,
    policy.model_training_opt_out,
    policy.consent_notification_enabled,
    policy.require_sso
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = p_workspace_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_workspace_domain(
  p_workspace_id uuid,
  p_domain text
)
RETURNS TABLE (
  workspace_id uuid,
  domain text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
#variable_conflict use_column
DECLARE
  v_domain text := lower(btrim(p_domain));
  v_actor_id uuid;
BEGIN
  v_actor_id := private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'enterprise.sso'
  );

  INSERT INTO public.workspace_verified_domains (
    workspace_id,
    domain,
    created_by_user_id
  ) VALUES (
    p_workspace_id,
    v_domain,
    v_actor_id
  )
  ON CONFLICT (workspace_id, domain) DO NOTHING;

  RETURN QUERY
  SELECT claimed.workspace_id, claimed.domain
  FROM public.workspace_verified_domains AS claimed
  WHERE claimed.workspace_id = p_workspace_id
    AND claimed.domain = v_domain;
END;
$$;

CREATE OR REPLACE FUNCTION public.rotate_workspace_scim_token(
  p_workspace_id uuid,
  p_domain text,
  p_token text
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_workspace_capability(
    p_workspace_id,
    'enterprise.scim'
  );

  IF p_token IS NULL OR octet_length(p_token) < 32 THEN
    RAISE EXCEPTION 'invalid scim token'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.workspace_identity_providers (
    workspace_id,
    protocol,
    domain,
    scim_token_hash,
    updated_at
  ) VALUES (
    p_workspace_id,
    'saml',
    lower(btrim(p_domain)),
    extensions.digest(p_token, 'sha256'),
    now()
  )
  ON CONFLICT (workspace_id) DO UPDATE SET
    domain = EXCLUDED.domain,
    scim_token_hash = EXCLUDED.scim_token_hash,
    updated_at = now();
END;
$$;

CREATE OR REPLACE FUNCTION public.scim_apply_user(
  p_token text,
  p_email text,
  p_active boolean
)
RETURNS TABLE (
  user_id uuid,
  workspace_id uuid,
  active boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
#variable_conflict use_column
DECLARE
  v_provider public.workspace_identity_providers%ROWTYPE;
  v_user_id uuid;
BEGIN
  IF p_token IS NULL OR octet_length(p_token) < 32 THEN
    RAISE EXCEPTION 'invalid scim token'
      USING ERRCODE = '42501';
  END IF;

  SELECT provider.*
  INTO v_provider
  FROM public.workspace_identity_providers AS provider
  WHERE provider.scim_token_hash = extensions.digest(p_token, 'sha256');

  IF NOT FOUND OR NOT private.workspace_has_capability(
    v_provider.workspace_id,
    'enterprise.scim'
  ) THEN
    RAISE EXCEPTION 'invalid scim token'
      USING ERRCODE = '42501';
  END IF;

  SELECT users.id
  INTO v_user_id
  FROM auth.users AS users
  WHERE lower(users.email) = lower(btrim(p_email));

  IF v_user_id IS NULL THEN
    RAISE EXCEPTION 'scim user not found'
      USING ERRCODE = 'P0002';
  END IF;

  IF COALESCE(p_active, false) THEN
    INSERT INTO public.workspace_memberships (
      workspace_id,
      user_id,
      role
    ) VALUES (
      v_provider.workspace_id,
      v_user_id,
      'member'
    )
    ON CONFLICT DO NOTHING;

    UPDATE public.workspace_memberships AS membership
    SET
      deleted_at = NULL,
      updated_at = now()
    WHERE membership.workspace_id = v_provider.workspace_id
      AND membership.user_id = v_user_id
      AND membership.deleted_at IS NOT NULL;
  ELSE
    UPDATE public.workspace_memberships AS membership
    SET
      deleted_at = now(),
      updated_at = now()
    WHERE membership.workspace_id = v_provider.workspace_id
      AND membership.user_id = v_user_id
      AND membership.deleted_at IS NULL;

    DELETE FROM public.sync_devices AS device
    WHERE device.user_id = v_user_id;
  END IF;

  RETURN QUERY
  SELECT
    v_user_id,
    v_provider.workspace_id,
    COALESCE(p_active, false);
END;
$$;

CREATE OR REPLACE FUNCTION private.email_domain_requires_sso(p_email text)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.workspace_verified_domains AS claimed
    JOIN public.workspaces AS workspace
      ON workspace.id = claimed.workspace_id
    JOIN public.workspace_policies AS policy
      ON policy.workspace_id = claimed.workspace_id
    WHERE claimed.domain = lower(split_part(btrim(COALESCE(p_email, '')), '@', 2))
      AND position('@' in COALESCE(p_email, '')) > 0
      AND workspace.deleted_at IS NULL
      AND workspace.kind = 'shared'
      AND policy.require_sso
      AND private.workspace_has_capability(
        claimed.workspace_id,
        'enterprise.sso'
      )
  );
$$;

CREATE OR REPLACE FUNCTION private.capture_user_into_verified_domain_workspace()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_email text := lower(COALESCE(NEW.email, ''));
  v_domain text;
  v_workspace_id uuid;
BEGIN
  IF v_email IS NULL OR position('@' in v_email) = 0 THEN
    RETURN NEW;
  END IF;

  v_domain := split_part(v_email, '@', 2);

  SELECT claimed.workspace_id
  INTO v_workspace_id
  FROM public.workspace_verified_domains AS claimed
  JOIN public.workspaces AS workspace
    ON workspace.id = claimed.workspace_id
  WHERE claimed.domain = v_domain
    AND workspace.deleted_at IS NULL
    AND workspace.kind = 'shared'
    AND private.workspace_has_capability(
      claimed.workspace_id,
      'enterprise.sso'
    )
  LIMIT 1;

  IF v_workspace_id IS NULL THEN
    RETURN NEW;
  END IF;

  INSERT INTO public.workspace_memberships (
    workspace_id,
    user_id,
    role
  ) VALUES (
    v_workspace_id,
    NEW.id,
    'member'
  )
  ON CONFLICT DO NOTHING;

  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.enforce_workspace_retention()
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_deleted integer := 0;
BEGIN
  UPDATE public.session_shares AS share
  SET
    deleted_at = now(),
    updated_at = now()
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = share.workspace_id
    AND policy.retention_days IS NOT NULL
    AND private.workspace_has_capability(
      policy.workspace_id,
      'enterprise.retention'
    )
    AND share.deleted_at IS NULL
    AND share.created_at < now() - make_interval(days => policy.retention_days);

  GET DIAGNOSTICS v_deleted = ROW_COUNT;

  DELETE FROM public.session_share_snapshots AS snapshot
  USING public.session_shares AS share
  JOIN public.workspace_policies AS policy
    ON policy.workspace_id = share.workspace_id
  WHERE snapshot.share_id = share.id
    AND policy.retention_days IS NOT NULL
    AND private.workspace_has_capability(
      policy.workspace_id,
      'enterprise.retention'
    )
    AND share.deleted_at IS NOT NULL
    AND share.created_at < now() - make_interval(days => policy.retention_days);

  RETURN v_deleted;
END;
$$;

COMMIT;
