-- Share folders, templates, and automation recipes with individual guests or
-- an entire paid Team workspace. Direct guests stay resource-scoped: sharing a
-- second resource with the same person requires adding them to a Team.

BEGIN;

SET LOCAL lock_timeout = '30s';

CREATE TABLE public.shared_resources (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  resource_type text NOT NULL CHECK (
    resource_type IN ('folder', 'template', 'automation')
  ),
  source_id text NOT NULL CHECK (
    source_id = btrim(source_id)
    AND source_id <> ''
    AND source_id !~ '[[:cntrl:]]'
    AND octet_length(source_id) <= 256
  ),
  title text NOT NULL CHECK (
    title = btrim(title)
    AND title <> ''
    AND title !~ '[[:cntrl:]]'
    AND octet_length(title) <= 512
  ),
  payload jsonb NOT NULL CHECK (
    jsonb_typeof(payload) = 'object'
    AND octet_length(payload::text) <= 2097152
  ),
  general_workspace_id uuid REFERENCES public.workspaces(id),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  CONSTRAINT shared_resources_owner_type_source_key
    UNIQUE (owner_user_id, resource_type, source_id)
);

CREATE INDEX shared_resources_workspace_active_idx
ON public.shared_resources (general_workspace_id, resource_type, updated_at DESC)
WHERE deleted_at IS NULL AND general_workspace_id IS NOT NULL;

CREATE TABLE public.shared_resource_guests (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.shared_resources(id) ON DELETE CASCADE,
  invitee_email text NOT NULL CHECK (
    invitee_email = lower(btrim(invitee_email))
    AND invitee_email ~ '^[^[:space:]@]+@[^[:space:]@]+\.[^[:space:]@]+$'
    AND octet_length(invitee_email) <= 320
  ),
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz,
  CONSTRAINT shared_resource_guests_share_email_key
    UNIQUE (share_id, invitee_email)
);

CREATE INDEX shared_resource_guests_email_active_idx
ON public.shared_resource_guests (invitee_email, share_id)
WHERE revoked_at IS NULL;

ALTER TABLE public.shared_resources ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.shared_resource_guests ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.shared_resources FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.shared_resource_guests FROM PUBLIC, anon, authenticated;

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
    WHEN paid_features.has_team THEN ARRAY[
      'team.shared_notes',
      'team.shared_resources',
      'team.manage_workspace',
      'team.manage_members'
    ]::text[]
    ELSE ARRAY[]::text[]
  END
  FROM paid_features;
$$;

CREATE OR REPLACE FUNCTION private.require_shared_resource_owner(
  p_share_id uuid
)
RETURNS public.shared_resources
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_share public.shared_resources%ROWTYPE;
BEGIN
  SELECT resource.*
  INTO v_share
  FROM public.shared_resources AS resource
  WHERE resource.id = p_share_id
    AND resource.owner_user_id = v_actor_id
    AND resource.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'shared resource operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_share;
END;
$$;

REVOKE ALL ON FUNCTION private.require_shared_resource_owner(uuid)
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION public.upsert_shared_resource(
  p_resource_type text,
  p_source_id text,
  p_title text,
  p_payload jsonb,
  p_general_workspace_id uuid DEFAULT NULL
)
RETURNS TABLE (
  share_id uuid,
  resource_type text,
  source_id text,
  resource_title text,
  general_workspace_id uuid,
  updated_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_type text := lower(btrim(COALESCE(p_resource_type, '')));
  v_source_id text := btrim(COALESCE(p_source_id, ''));
  v_title text := btrim(COALESCE(p_title, ''));
  v_share public.shared_resources%ROWTYPE;
BEGIN
  IF v_type NOT IN ('folder', 'template', 'automation') THEN
    RAISE EXCEPTION 'invalid shared resource type' USING ERRCODE = '22023';
  END IF;
  IF v_source_id = ''
    OR v_source_id ~ '[[:cntrl:]]'
    OR octet_length(v_source_id) > 256
  THEN
    RAISE EXCEPTION 'invalid shared resource source' USING ERRCODE = '22023';
  END IF;
  IF v_title = ''
    OR v_title ~ '[[:cntrl:]]'
    OR octet_length(v_title) > 512
  THEN
    RAISE EXCEPTION 'invalid shared resource title' USING ERRCODE = '22023';
  END IF;
  IF jsonb_typeof(p_payload) <> 'object'
    OR octet_length(p_payload::text) > 2097152
  THEN
    RAISE EXCEPTION 'invalid shared resource payload' USING ERRCODE = '22023';
  END IF;

  IF p_general_workspace_id IS NULL THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  ELSE
    PERFORM private.require_workspace_member(p_general_workspace_id);
    PERFORM private.require_workspace_capability(
      p_general_workspace_id,
      'team.shared_resources'
    );
  END IF;

  INSERT INTO public.shared_resources AS resource (
    owner_user_id,
    resource_type,
    source_id,
    title,
    payload,
    general_workspace_id
  ) VALUES (
    v_actor_id,
    v_type,
    v_source_id,
    v_title,
    p_payload,
    p_general_workspace_id
  )
  ON CONFLICT ON CONSTRAINT shared_resources_owner_type_source_key DO UPDATE SET
    title = EXCLUDED.title,
    payload = EXCLUDED.payload,
    general_workspace_id = EXCLUDED.general_workspace_id,
    updated_at = now(),
    deleted_at = NULL
  RETURNING resource.* INTO v_share;

  RETURN QUERY SELECT
    v_share.id,
    v_share.resource_type,
    v_share.source_id,
    v_share.title,
    v_share.general_workspace_id,
    v_share.updated_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_shared_resources(
  p_resource_type text
)
RETURNS TABLE (
  share_id uuid,
  resource_type text,
  source_id text,
  resource_title text,
  payload jsonb,
  owner_user_id uuid,
  owner_email text,
  general_workspace_id uuid,
  workspace_name text,
  access_kind text,
  updated_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_type text := lower(btrim(COALESCE(p_resource_type, '')));
BEGIN
  IF v_type NOT IN ('folder', 'template', 'automation') THEN
    RAISE EXCEPTION 'invalid shared resource type' USING ERRCODE = '22023';
  END IF;

  SELECT lower(btrim(account.email))
  INTO v_actor_email
  FROM auth.users AS account
  WHERE account.id = v_actor_id
    AND account.email_confirmed_at IS NOT NULL;

  IF v_actor_email IS NULL THEN
    RAISE EXCEPTION 'permanent user required' USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    resource.id,
    resource.resource_type,
    resource.source_id,
    resource.title,
    resource.payload,
    resource.owner_user_id,
    lower(btrim(owner_account.email)),
    resource.general_workspace_id,
    workspace.name,
    CASE
      WHEN resource.owner_user_id = v_actor_id THEN 'owner'
      WHEN membership.id IS NOT NULL
        AND private.workspace_has_capability(
          resource.general_workspace_id,
          'team.shared_resources'
        )
      THEN 'team'
      ELSE 'guest'
    END,
    resource.updated_at
  FROM public.shared_resources AS resource
  JOIN auth.users AS owner_account
    ON owner_account.id = resource.owner_user_id
  LEFT JOIN public.workspaces AS workspace
    ON workspace.id = resource.general_workspace_id
    AND workspace.kind = 'shared'
    AND workspace.deleted_at IS NULL
  LEFT JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = resource.general_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.deleted_at IS NULL
  LEFT JOIN public.shared_resource_guests AS guest
    ON guest.share_id = resource.id
    AND guest.invitee_email = v_actor_email
    AND guest.revoked_at IS NULL
  WHERE resource.resource_type = v_type
    AND resource.deleted_at IS NULL
    AND (
      resource.owner_user_id = v_actor_id
      OR (
        membership.id IS NOT NULL
        AND private.workspace_has_capability(
          resource.general_workspace_id,
          'team.shared_resources'
        )
      )
      OR (
        guest.id IS NOT NULL
        AND (
          resource.general_workspace_id IS NULL
          OR private.workspace_has_capability(
            resource.general_workspace_id,
            'team.shared_resources'
          )
        )
      )
    )
  ORDER BY resource.updated_at DESC, resource.id;
END;
$$;

CREATE OR REPLACE FUNCTION public.move_shared_resource(
  p_share_id uuid,
  p_source_id text,
  p_title text,
  p_payload jsonb
)
RETURNS TABLE (
  share_id uuid,
  source_id text,
  resource_title text,
  updated_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_source_id text := btrim(COALESCE(p_source_id, ''));
  v_title text := btrim(COALESCE(p_title, ''));
  v_share public.shared_resources%ROWTYPE;
BEGIN
  v_share := private.require_shared_resource_owner(p_share_id);

  IF v_source_id = ''
    OR v_source_id ~ '[[:cntrl:]]'
    OR octet_length(v_source_id) > 256
  THEN
    RAISE EXCEPTION 'invalid shared resource source' USING ERRCODE = '22023';
  END IF;
  IF v_title = ''
    OR v_title ~ '[[:cntrl:]]'
    OR octet_length(v_title) > 512
  THEN
    RAISE EXCEPTION 'invalid shared resource title' USING ERRCODE = '22023';
  END IF;
  IF jsonb_typeof(p_payload) <> 'object'
    OR octet_length(p_payload::text) > 2097152
  THEN
    RAISE EXCEPTION 'invalid shared resource payload' USING ERRCODE = '22023';
  END IF;

  IF v_share.general_workspace_id IS NULL THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  ELSE
    PERFORM private.require_workspace_member(v_share.general_workspace_id);
    PERFORM private.require_workspace_capability(
      v_share.general_workspace_id,
      'team.shared_resources'
    );
  END IF;

  UPDATE public.shared_resources AS resource
  SET
    source_id = v_source_id,
    title = v_title,
    payload = p_payload,
    updated_at = now()
  WHERE resource.id = p_share_id
  RETURNING resource.* INTO v_share;

  RETURN QUERY SELECT
    v_share.id,
    v_share.source_id,
    v_share.title,
    v_share.updated_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_shared_resource_guests(
  p_share_id uuid
)
RETURNS TABLE (
  guest_id uuid,
  invitee_email text,
  created_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_shared_resource_owner(p_share_id);

  RETURN QUERY
  SELECT guest.id, guest.invitee_email, guest.created_at
  FROM public.shared_resource_guests AS guest
  WHERE guest.share_id = p_share_id
    AND guest.revoked_at IS NULL
  ORDER BY guest.created_at, guest.id;
END;
$$;

CREATE OR REPLACE FUNCTION public.grant_shared_resource_access(
  p_share_id uuid,
  p_invitee_email text
)
RETURNS TABLE (
  guest_id uuid,
  invitee_email text,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_owner_email text;
  v_email text := lower(btrim(COALESCE(p_invitee_email, '')));
  v_share public.shared_resources%ROWTYPE;
  v_guest public.shared_resource_guests%ROWTYPE;
  v_was_created boolean;
BEGIN
  v_share := private.require_shared_resource_owner(p_share_id);

  IF v_email !~ '^[^[:space:]@]+@[^[:space:]@]+\.[^[:space:]@]+$'
    OR octet_length(v_email) > 320
  THEN
    RAISE EXCEPTION 'invalid shared resource guest email'
      USING ERRCODE = '22023';
  END IF;

  SELECT lower(btrim(account.email))
  INTO v_owner_email
  FROM auth.users AS account
  WHERE account.id = v_actor_id;

  IF v_email = v_owner_email THEN
    RAISE EXCEPTION 'resource owner already has access'
      USING ERRCODE = '22023';
  END IF;

  IF v_share.general_workspace_id IS NULL THEN
    PERFORM private.require_hyprnote_pro_entitlement();
  ELSE
    PERFORM private.require_workspace_member(v_share.general_workspace_id);
    PERFORM private.require_workspace_capability(
      v_share.general_workspace_id,
      'team.shared_resources'
    );
  END IF;

  IF EXISTS (
    SELECT 1
    FROM public.shared_resource_guests AS existing_guest
    JOIN public.shared_resources AS existing_resource
      ON existing_resource.id = existing_guest.share_id
      AND existing_resource.owner_user_id = v_actor_id
      AND existing_resource.deleted_at IS NULL
    WHERE existing_guest.invitee_email = v_email
      AND existing_guest.revoked_at IS NULL
      AND existing_guest.share_id <> p_share_id
      AND NOT EXISTS (
        SELECT 1
        FROM public.workspace_memberships AS membership
        JOIN auth.users AS member_account
          ON member_account.id = membership.user_id
        WHERE membership.workspace_id = existing_resource.general_workspace_id
          AND membership.deleted_at IS NULL
          AND member_account.email_confirmed_at IS NOT NULL
          AND lower(btrim(member_account.email)) = v_email
      )
  ) THEN
    RAISE EXCEPTION 'multi-resource guest requires Team membership'
      USING ERRCODE = '42501';
  END IF;

  SELECT existing.*
  INTO v_guest
  FROM public.shared_resource_guests AS existing
  WHERE existing.share_id = p_share_id
    AND existing.invitee_email = v_email
  FOR UPDATE;

  v_was_created := NOT FOUND OR v_guest.revoked_at IS NOT NULL;

  INSERT INTO public.shared_resource_guests AS guest (
    share_id,
    invitee_email
  ) VALUES (
    p_share_id,
    v_email
  )
  ON CONFLICT ON CONSTRAINT shared_resource_guests_share_email_key DO UPDATE SET
    updated_at = now(),
    revoked_at = NULL
  RETURNING guest.* INTO v_guest;

  RETURN QUERY SELECT v_guest.id, v_guest.invitee_email, v_was_created;
END;
$$;

CREATE OR REPLACE FUNCTION public.revoke_shared_resource_access(
  p_guest_id uuid
)
RETURNS TABLE (
  guest_id uuid,
  revoked_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share_id uuid;
  v_revoked_at timestamptz;
BEGIN
  SELECT guest.share_id
  INTO v_share_id
  FROM public.shared_resource_guests AS guest
  WHERE guest.id = p_guest_id;

  PERFORM private.require_shared_resource_owner(v_share_id);

  UPDATE public.shared_resource_guests AS guest
  SET updated_at = now(), revoked_at = now()
  WHERE guest.id = p_guest_id
    AND guest.revoked_at IS NULL
  RETURNING guest.revoked_at INTO v_revoked_at;

  IF v_revoked_at IS NULL THEN
    RAISE EXCEPTION 'shared resource access is unavailable'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY SELECT p_guest_id, v_revoked_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.delete_shared_resource(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  deleted_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_deleted_at timestamptz;
BEGIN
  PERFORM private.require_shared_resource_owner(p_share_id);

  UPDATE public.shared_resources AS resource
  SET updated_at = now(), deleted_at = now()
  WHERE resource.id = p_share_id
  RETURNING resource.deleted_at INTO v_deleted_at;

  UPDATE public.shared_resource_guests AS guest
  SET updated_at = now(), revoked_at = COALESCE(guest.revoked_at, now())
  WHERE guest.share_id = p_share_id;

  RETURN QUERY SELECT p_share_id, v_deleted_at;
END;
$$;

REVOKE ALL ON FUNCTION public.upsert_shared_resource(text, text, text, jsonb, uuid)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.list_shared_resources(text)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.move_shared_resource(uuid, text, text, jsonb)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.list_shared_resource_guests(uuid)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.grant_shared_resource_access(uuid, text)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.revoke_shared_resource_access(uuid)
  FROM PUBLIC, anon;
REVOKE ALL ON FUNCTION public.delete_shared_resource(uuid)
  FROM PUBLIC, anon;

GRANT EXECUTE ON FUNCTION public.upsert_shared_resource(text, text, text, jsonb, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_shared_resources(text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.move_shared_resource(uuid, text, text, jsonb)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_shared_resource_guests(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.grant_shared_resource_access(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.revoke_shared_resource_access(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.delete_shared_resource(uuid)
  TO authenticated;

COMMIT;
