-- Additive enterprise admin controls: workspace policies, usage analytics,
-- SSO/SCIM identity, and domain capture. Older clients ignore unknown tables.

BEGIN;

SET LOCAL lock_timeout = '30s';

CREATE TABLE public.workspace_policies (
  workspace_id uuid PRIMARY KEY REFERENCES public.workspaces(id) ON DELETE CASCADE,
  allowed_share_scopes text[] NOT NULL DEFAULT ARRAY[
    'restricted',
    'workspace',
    'link',
    'public'
  ],
  default_share_scope text NOT NULL DEFAULT 'restricted',
  retention_days integer,
  model_training_opt_out boolean NOT NULL DEFAULT true,
  consent_notification_enabled boolean NOT NULL DEFAULT true,
  require_sso boolean NOT NULL DEFAULT false,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT workspace_policies_scopes_check CHECK (
    allowed_share_scopes <@ ARRAY['restricted', 'workspace', 'link', 'public']::text[]
    AND allowed_share_scopes @> ARRAY['restricted']::text[]
    AND default_share_scope = ANY (allowed_share_scopes)
  ),
  CONSTRAINT workspace_policies_retention_check CHECK (
    retention_days IS NULL OR retention_days > 0
  )
);

ALTER TABLE public.workspace_policies ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.workspace_policies
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.workspace_policies TO service_role;

CREATE TABLE public.workspace_verified_domains (
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  domain text NOT NULL,
  verified_at timestamptz NOT NULL DEFAULT now(),
  created_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  PRIMARY KEY (workspace_id, domain),
  CONSTRAINT workspace_verified_domains_format_check CHECK (
    domain = lower(btrim(domain))
    AND domain ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)+$'
  )
);

ALTER TABLE public.workspace_verified_domains ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON TABLE public.workspace_verified_domains
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.workspace_verified_domains TO service_role;

CREATE UNIQUE INDEX workspace_verified_domains_domain_key
  ON public.workspace_verified_domains(domain);

CREATE TABLE public.workspace_identity_providers (
  workspace_id uuid PRIMARY KEY REFERENCES public.workspaces(id) ON DELETE CASCADE,
  protocol text NOT NULL DEFAULT 'saml',
  domain text NOT NULL,
  scim_token_hash bytea,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT workspace_identity_providers_protocol_check CHECK (
    protocol IN ('saml', 'oidc')
  ),
  CONSTRAINT workspace_identity_providers_token_hash_check CHECK (
    scim_token_hash IS NULL OR octet_length(scim_token_hash) = 32
  )
);

ALTER TABLE public.workspace_identity_providers ENABLE ROW LEVEL SECURITY;
REVOKE ALL ON TABLE public.workspace_identity_providers
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.workspace_identity_providers TO service_role;

CREATE OR REPLACE FUNCTION private.require_workspace_manager(
  p_workspace_id uuid
)
RETURNS uuid
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
BEGIN
  IF v_actor_id IS NULL THEN
    RAISE EXCEPTION 'workspace policy operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = v_actor_id
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'workspace policy operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_actor_id;
END;
$$;

REVOKE ALL ON FUNCTION private.require_workspace_manager(uuid)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.require_workspace_member(
  p_workspace_id uuid
)
RETURNS uuid
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
BEGIN
  IF v_actor_id IS NULL THEN
    RAISE EXCEPTION 'workspace policy operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = v_actor_id
      AND membership.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'workspace policy operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_actor_id;
END;
$$;

REVOKE ALL ON FUNCTION private.require_workspace_member(uuid)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.get_workspace_policy(
  p_workspace_id uuid
)
RETURNS public.workspace_policies
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_policy public.workspace_policies;
BEGIN
  PERFORM private.require_workspace_member(p_workspace_id);

  SELECT policy.*
  INTO v_policy
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = p_workspace_id;

  IF NOT FOUND THEN
    v_policy.workspace_id := p_workspace_id;
    v_policy.allowed_share_scopes := ARRAY['restricted', 'workspace', 'link', 'public'];
    v_policy.default_share_scope := 'restricted';
    v_policy.retention_days := NULL;
    v_policy.model_training_opt_out := true;
    v_policy.consent_notification_enabled := true;
    v_policy.require_sso := false;
    v_policy.updated_at := now();
  END IF;

  RETURN v_policy;
END;
$$;

REVOKE ALL ON FUNCTION private.get_workspace_policy(uuid)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.get_workspace_policy(
  p_workspace_id uuid
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
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RETURN QUERY
  SELECT
    policy.workspace_id,
    policy.allowed_share_scopes,
    policy.default_share_scope,
    policy.retention_days,
    policy.model_training_opt_out,
    policy.consent_notification_enabled,
    policy.require_sso
  FROM private.get_workspace_policy(p_workspace_id) AS policy;
END;
$$;

REVOKE ALL ON FUNCTION public.get_workspace_policy(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.get_workspace_policy(uuid)
  TO authenticated;

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
BEGIN
  PERFORM private.require_workspace_manager(p_workspace_id);
  PERFORM private.require_hyprnote_pro_entitlement();

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

REVOKE ALL ON FUNCTION public.set_workspace_policy(uuid, text[], text, integer, boolean, boolean, boolean)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.set_workspace_policy(uuid, text[], text, integer, boolean, boolean, boolean)
  TO authenticated;

CREATE OR REPLACE FUNCTION private.assert_allowed_share_scope(
  p_workspace_id uuid,
  p_general_scope text
)
RETURNS void
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_policy public.workspace_policies%ROWTYPE;
BEGIN
  IF p_workspace_id IS NULL THEN
    RETURN;
  END IF;

  SELECT policy.*
  INTO v_policy
  FROM public.workspace_policies AS policy
  WHERE policy.workspace_id = p_workspace_id;

  IF FOUND
    AND NOT (p_general_scope = ANY (v_policy.allowed_share_scopes))
  THEN
    RAISE EXCEPTION 'workspace policy forbids this share scope'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.assert_allowed_share_scope(uuid, text)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.protected_set_session_share_scope(
  p_share_id uuid,
  p_general_scope text,
  p_general_workspace_id uuid DEFAULT NULL
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  general_workspace_id uuid,
  public_slug text,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.id = p_share_id;

  IF FOUND THEN
    PERFORM private.assert_allowed_share_scope(
      v_share.workspace_id,
      p_general_scope
    );
  END IF;

  SELECT *
  INTO v_result
  FROM private.set_session_share_scope(
    p_share_id,
    p_general_scope,
    p_general_workspace_id
  );

  IF p_general_scope <> 'restricted' THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  END IF;

  RETURN QUERY
  SELECT
    v_result.share_id,
    v_result.general_scope,
    v_result.general_workspace_id,
    v_result.public_slug,
    v_result.access_version;
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
    AND share.deleted_at IS NULL
    AND share.created_at < now() - make_interval(days => policy.retention_days);

  GET DIAGNOSTICS v_deleted = ROW_COUNT;

  DELETE FROM public.session_share_snapshots AS snapshot
  USING public.session_shares AS share
  JOIN public.workspace_policies AS policy
    ON policy.workspace_id = share.workspace_id
  WHERE snapshot.share_id = share.id
    AND share.deleted_at IS NOT NULL
    AND policy.retention_days IS NOT NULL
    AND share.created_at < now() - make_interval(days => policy.retention_days);

  RETURN v_deleted;
END;
$$;

REVOKE ALL ON FUNCTION private.enforce_workspace_retention()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.enforce_workspace_retention()
  TO service_role;

CREATE OR REPLACE FUNCTION public.enforce_workspace_retention()
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  RETURN private.enforce_workspace_retention();
END;
$$;

REVOKE ALL ON FUNCTION public.enforce_workspace_retention()
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.enforce_workspace_retention()
  TO service_role;

CREATE OR REPLACE FUNCTION private.protected_issue_session_share_link(
  p_share_id uuid,
  p_force_rotate boolean
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_result record;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.id = p_share_id;

  IF FOUND THEN
    PERFORM private.assert_allowed_share_scope(v_share.workspace_id, 'link');
  END IF;

  SELECT *
  INTO v_result
  FROM private.issue_session_share_link(p_share_id, p_force_rotate);

  IF p_force_rotate OR v_result.was_created THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  END IF;

  RETURN QUERY
  SELECT
    v_result.share_id,
    v_result.link_id,
    v_result.link_token,
    v_result.access_version,
    v_result.was_created;
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
    usage.is_billed
  FROM private.get_workspace_seat_usage(p_workspace_id) AS usage;
END;
$$;

REVOKE ALL ON FUNCTION public.get_workspace_usage_overview(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.get_workspace_usage_overview(uuid)
  TO authenticated;

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
  PERFORM private.require_hyprnote_pro_entitlement();

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

REVOKE ALL ON FUNCTION public.claim_workspace_domain(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.claim_workspace_domain(uuid, text)
  TO authenticated;

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

DROP TRIGGER IF EXISTS on_auth_user_domain_capture ON auth.users;
CREATE TRIGGER on_auth_user_domain_capture
  AFTER INSERT OR UPDATE OF email ON auth.users
  FOR EACH ROW
  EXECUTE FUNCTION private.capture_user_into_verified_domain_workspace();

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
  PERFORM private.require_hyprnote_pro_entitlement();

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

REVOKE ALL ON FUNCTION public.rotate_workspace_scim_token(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.rotate_workspace_scim_token(uuid, text, text)
  TO authenticated;

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

  IF NOT FOUND THEN
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

REVOKE ALL ON FUNCTION public.scim_apply_user(text, text, boolean)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.scim_apply_user(text, text, boolean)
  TO service_role;

CREATE OR REPLACE FUNCTION public.scim_apply_user_id(
  p_token text,
  p_user_id uuid,
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
DECLARE
  v_email text;
BEGIN
  SELECT users.email
  INTO v_email
  FROM auth.users AS users
  WHERE users.id = p_user_id;

  IF v_email IS NULL THEN
    RAISE EXCEPTION 'scim user not found'
      USING ERRCODE = 'P0002';
  END IF;

  RETURN QUERY
  SELECT applied.user_id, applied.workspace_id, applied.active
  FROM public.scim_apply_user(p_token, v_email, p_active) AS applied;
END;
$$;

REVOKE ALL ON FUNCTION public.scim_apply_user_id(text, uuid, boolean)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.scim_apply_user_id(text, uuid, boolean)
  TO service_role;

COMMIT;
