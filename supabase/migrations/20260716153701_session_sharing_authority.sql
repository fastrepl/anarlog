CREATE TABLE public.session_shares (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  session_id text NOT NULL,
  created_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  general_scope text NOT NULL DEFAULT 'restricted',
  general_workspace_id uuid REFERENCES public.workspaces(id) ON DELETE SET NULL,
  public_slug text NOT NULL DEFAULT (
    's_' || encode(extensions.gen_random_bytes(16), 'hex')
  ),
  access_version bigint NOT NULL DEFAULT 1,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  CONSTRAINT session_shares_session_id_check CHECK (
    session_id = btrim(session_id)
    AND session_id <> ''
    AND session_id !~ '[[:cntrl:]]'
    AND octet_length(session_id) <= 128
  ),
  CONSTRAINT session_shares_general_scope_check CHECK (
    general_scope IN ('restricted', 'workspace', 'link', 'public')
  ),
  CONSTRAINT session_shares_general_workspace_check CHECK (
    (general_scope = 'workspace' AND general_workspace_id IS NOT NULL)
    OR (general_scope <> 'workspace' AND general_workspace_id IS NULL)
  ),
  CONSTRAINT session_shares_public_slug_check CHECK (
    public_slug ~ '^s_[0-9a-f]{32}$'
  ),
  CONSTRAINT session_shares_access_version_check CHECK (access_version > 0),
  CONSTRAINT session_shares_workspace_session_key UNIQUE (workspace_id, session_id),
  CONSTRAINT session_shares_public_slug_key UNIQUE (public_slug)
);

CREATE TABLE public.session_share_links (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  token_hash bytea NOT NULL,
  created_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  revoked_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz,
  CONSTRAINT session_share_links_token_hash_check CHECK (
    octet_length(token_hash) = 32
  ),
  CONSTRAINT session_share_links_revocation_check CHECK (
    revoked_by_user_id IS NULL OR revoked_at IS NOT NULL
  ),
  CONSTRAINT session_share_links_token_hash_key UNIQUE (token_hash)
);

CREATE UNIQUE INDEX session_share_links_active_key
  ON public.session_share_links(share_id)
  WHERE revoked_at IS NULL;

CREATE TABLE public.session_access_grants (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  grantee_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  capability text NOT NULL DEFAULT 'viewer',
  granted_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  revoked_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  revoked_at timestamptz,
  CONSTRAINT session_access_grants_capability_check CHECK (
    capability IN ('viewer', 'commenter', 'editor')
  ),
  CONSTRAINT session_access_grants_revocation_check CHECK (
    revoked_by_user_id IS NULL OR revoked_at IS NOT NULL
  ),
  CONSTRAINT session_access_grants_share_user_key UNIQUE (
    share_id,
    grantee_user_id
  )
);

CREATE INDEX session_access_grants_user_active_idx
  ON public.session_access_grants(grantee_user_id, share_id)
  WHERE revoked_at IS NULL;

CREATE TABLE public.session_access_invitations (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  invitee_email text NOT NULL,
  invitee_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  capability text NOT NULL DEFAULT 'viewer',
  token_hash bytea NOT NULL,
  invited_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  accepted_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  revoked_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  accepted_at timestamptz,
  revoked_at timestamptz,
  CONSTRAINT session_access_invitations_email_check CHECK (
    invitee_email = lower(btrim(invitee_email))
    AND invitee_email ~ '^[^[:space:]@]+@[^[:space:]@]+$'
    AND invitee_email !~ '[[:cntrl:]]'
    AND octet_length(invitee_email) <= 320
  ),
  CONSTRAINT session_access_invitations_capability_check CHECK (
    capability IN ('viewer', 'commenter', 'editor')
  ),
  CONSTRAINT session_access_invitations_token_hash_check CHECK (
    octet_length(token_hash) = 32
  ),
  CONSTRAINT session_access_invitations_state_check CHECK (
    NOT (accepted_at IS NOT NULL AND revoked_at IS NOT NULL)
    AND (accepted_by_user_id IS NULL OR accepted_at IS NOT NULL)
    AND (revoked_by_user_id IS NULL OR revoked_at IS NOT NULL)
  ),
  CONSTRAINT session_access_invitations_expiry_check CHECK (
    expires_at > created_at
  ),
  CONSTRAINT session_access_invitations_token_hash_key UNIQUE (token_hash)
);

CREATE UNIQUE INDEX session_access_invitations_pending_key
  ON public.session_access_invitations(share_id, invitee_email)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TABLE public.session_access_requests (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  requester_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  requested_capability text NOT NULL DEFAULT 'viewer',
  status text NOT NULL DEFAULT 'pending',
  reviewed_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  reviewed_at timestamptz,
  CONSTRAINT session_access_requests_capability_check CHECK (
    requested_capability IN ('viewer', 'commenter', 'editor')
  ),
  CONSTRAINT session_access_requests_status_check CHECK (
    status IN ('pending', 'approved', 'denied', 'cancelled')
  ),
  CONSTRAINT session_access_requests_state_check CHECK (
    (
      status IN ('pending', 'cancelled')
      AND reviewed_at IS NULL
      AND reviewed_by_user_id IS NULL
    )
    OR (
      status IN ('approved', 'denied')
      AND reviewed_at IS NOT NULL
    )
  )
);

CREATE UNIQUE INDEX session_access_requests_pending_key
  ON public.session_access_requests(share_id, requester_user_id)
  WHERE status = 'pending';

CREATE TABLE public.session_access_events (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  event_type text NOT NULL,
  actor_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  subject_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  related_entity_id uuid,
  previous_value text,
  new_value text,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT session_access_events_type_check CHECK (
    event_type IN (
      'share_created',
      'share_reactivated',
      'scope_changed',
      'link_enabled',
      'link_rotated',
      'invitation_created',
      'invitation_resent',
      'invitation_revoked',
      'invitation_accepted',
      'grant_changed',
      'grant_revoked',
      'request_created',
      'request_cancelled',
      'request_approved',
      'request_denied'
    )
  )
);

CREATE INDEX session_access_events_share_created_idx
  ON public.session_access_events(share_id, created_at DESC);

ALTER TABLE public.session_shares ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_share_links ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_access_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_access_invitations ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_access_requests ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.session_access_events ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_shares FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_share_links FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_access_grants FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_access_invitations FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_access_requests FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.session_access_events FROM PUBLIC, anon, authenticated;

GRANT ALL ON TABLE public.session_shares TO service_role;
GRANT ALL ON TABLE public.session_share_links TO service_role;
GRANT ALL ON TABLE public.session_access_grants TO service_role;
GRANT ALL ON TABLE public.session_access_invitations TO service_role;
GRANT ALL ON TABLE public.session_access_requests TO service_role;
GRANT ALL ON TABLE public.session_access_events TO service_role;

CREATE POLICY session_shares_service_all
  ON public.session_shares
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_share_links_service_all
  ON public.session_share_links
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_access_grants_service_all
  ON public.session_access_grants
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_access_invitations_service_all
  ON public.session_access_invitations
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_access_requests_service_all
  ON public.session_access_requests
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY session_access_events_service_all
  ON public.session_access_events
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.bump_session_share_versions_for_membership()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_old_workspace_id uuid;
  v_new_workspace_id uuid;
BEGIN
  IF TG_OP IN ('UPDATE', 'DELETE') THEN
    v_old_workspace_id := OLD.workspace_id;
  END IF;

  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    v_new_workspace_id := NEW.workspace_id;
  END IF;

  UPDATE public.session_shares AS share
  SET
    access_version = share.access_version + 1,
    updated_at = now()
  WHERE share.deleted_at IS NULL
    AND (
      share.workspace_id IN (v_old_workspace_id, v_new_workspace_id)
      OR share.general_workspace_id IN (v_old_workspace_id, v_new_workspace_id)
    );

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION private.restrict_shares_for_unavailable_workspace()
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

  UPDATE public.session_shares AS source_share
  SET
    access_version = source_share.access_version + 1,
    updated_at = now()
  WHERE source_share.workspace_id = OLD.id
    AND source_share.general_workspace_id IS DISTINCT FROM OLD.id
    AND source_share.deleted_at IS NULL;

  WITH restricted_shares AS (
    UPDATE public.session_shares AS share
    SET
      general_scope = 'restricted',
      general_workspace_id = NULL,
      access_version = share.access_version + 1,
      updated_at = now()
    WHERE share.general_workspace_id = OLD.id
      AND share.deleted_at IS NULL
    RETURNING share.id
  )
  INSERT INTO public.session_access_events (
    share_id,
    event_type,
    actor_user_id,
    previous_value,
    new_value
  )
  SELECT
    restricted_share.id,
    'scope_changed',
    auth.uid(),
    'workspace:' || OLD.id::text,
    'restricted'
  FROM restricted_shares AS restricted_share;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.bump_session_share_versions_for_membership()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.restrict_shares_for_unavailable_workspace()
  FROM PUBLIC, anon, authenticated;

CREATE TRIGGER on_workspace_membership_session_access_changed
  AFTER INSERT OR DELETE OR UPDATE OF workspace_id, user_id, role, deleted_at
  ON public.workspace_memberships
  FOR EACH ROW EXECUTE FUNCTION private.bump_session_share_versions_for_membership();

CREATE TRIGGER on_session_share_target_workspace_deleted
  BEFORE DELETE ON public.workspaces
  FOR EACH ROW EXECUTE FUNCTION private.restrict_shares_for_unavailable_workspace();

CREATE TRIGGER on_session_share_target_workspace_soft_deleted
  BEFORE UPDATE OF deleted_at ON public.workspaces
  FOR EACH ROW EXECUTE FUNCTION private.restrict_shares_for_unavailable_workspace();

GRANT USAGE ON SCHEMA private TO anon, authenticated;

CREATE OR REPLACE FUNCTION private.session_capability_rank(
  p_capability text
)
RETURNS integer
LANGUAGE sql
IMMUTABLE
SET search_path = ''
AS $$
  SELECT CASE p_capability
    WHEN 'viewer' THEN 1
    WHEN 'commenter' THEN 2
    WHEN 'editor' THEN 3
    ELSE 0
  END;
$$;

CREATE OR REPLACE FUNCTION private.require_permanent_user()
RETURNS uuid
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid;
BEGIN
  SELECT auth_user.id
  INTO v_user_id
  FROM auth.users AS auth_user
  WHERE auth_user.id = auth.uid()
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false;

  IF v_user_id IS NULL THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_user_id;
END;
$$;

CREATE OR REPLACE FUNCTION private.is_session_share_manager(
  p_share_id uuid,
  p_user_id uuid
)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor
      ON actor.id = membership.user_id
    WHERE share.id = p_share_id
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
      AND membership.user_id = p_user_id
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  );
$$;

CREATE OR REPLACE FUNCTION private.require_session_share_manager(
  p_share_id uuid
)
RETURNS public.session_shares
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_workspace_id uuid;
  v_share public.session_shares%ROWTYPE;
BEGIN
  SELECT share.workspace_id
  INTO v_workspace_id
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM public.workspace_memberships AS membership
  WHERE membership.workspace_id = v_workspace_id
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.workspace_id = v_workspace_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_share;
END;
$$;

CREATE OR REPLACE FUNCTION private.write_session_access_event(
  p_share_id uuid,
  p_event_type text,
  p_actor_user_id uuid,
  p_subject_user_id uuid DEFAULT NULL,
  p_related_entity_id uuid DEFAULT NULL,
  p_previous_value text DEFAULT NULL,
  p_new_value text DEFAULT NULL
)
RETURNS void
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$
  INSERT INTO public.session_access_events (
    share_id,
    event_type,
    actor_user_id,
    subject_user_id,
    related_entity_id,
    previous_value,
    new_value
  ) VALUES (
    p_share_id,
    p_event_type,
    p_actor_user_id,
    p_subject_user_id,
    p_related_entity_id,
    p_previous_value,
    p_new_value
  );
$$;

REVOKE ALL ON FUNCTION private.session_capability_rank(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.require_permanent_user()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.is_session_share_manager(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.require_session_share_manager(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.write_session_access_event(
  uuid,
  text,
  uuid,
  uuid,
  uuid,
  text,
  text
) FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.create_session_share(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  public_slug text,
  access_version bigint,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_session_id text := btrim(p_session_id);
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF v_session_id IS NULL
    OR v_session_id = ''
    OR v_session_id ~ '[[:cntrl:]]'
    OR octet_length(v_session_id) > 128
  THEN
    RAISE EXCEPTION 'invalid session id'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = workspace.id
  WHERE workspace.id = p_workspace_id
    AND workspace.deleted_at IS NULL
    AND membership.user_id = v_actor_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
  FOR UPDATE OF workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  INSERT INTO public.session_shares (
    workspace_id,
    session_id,
    created_by_user_id
  ) VALUES (
    p_workspace_id,
    v_session_id,
    v_actor_id
  )
  ON CONFLICT (workspace_id, session_id) DO NOTHING
  RETURNING * INTO v_share;

  IF FOUND THEN
    PERFORM private.write_session_access_event(
      v_share.id,
      'share_created',
      v_actor_id
    );

    RETURN QUERY
    SELECT
      v_share.id,
      v_share.general_scope,
      v_share.public_slug,
      v_share.access_version,
      true;
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  WHERE share.workspace_id = p_workspace_id
    AND share.session_id = v_session_id
  FOR UPDATE;

  IF v_share.deleted_at IS NOT NULL THEN
    UPDATE public.session_share_links AS target_link
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now()
    WHERE target_link.share_id = v_share.id
      AND target_link.revoked_at IS NULL;

    UPDATE public.session_access_grants AS target_grant
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now(),
      updated_at = now()
    WHERE target_grant.share_id = v_share.id
      AND target_grant.revoked_at IS NULL;

    UPDATE public.session_access_invitations AS target_invitation
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now(),
      updated_at = now()
    WHERE target_invitation.share_id = v_share.id
      AND target_invitation.accepted_at IS NULL
      AND target_invitation.revoked_at IS NULL;

    UPDATE public.session_access_requests AS target_request
    SET
      status = 'cancelled',
      updated_at = now()
    WHERE target_request.share_id = v_share.id
      AND target_request.status = 'pending';

    UPDATE public.session_shares AS target_share
    SET
      general_scope = 'restricted',
      general_workspace_id = NULL,
      access_version = target_share.access_version + 1,
      updated_at = now(),
      deleted_at = NULL
    WHERE target_share.id = v_share.id
    RETURNING * INTO v_share;

    PERFORM private.write_session_access_event(
      v_share.id,
      'share_reactivated',
      v_actor_id
    );
  END IF;

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.general_scope,
    v_share.public_slug,
    v_share.access_version,
    false;
END;
$$;

CREATE OR REPLACE FUNCTION private.get_session_share_management(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  general_scope text,
  general_workspace_id uuid,
  public_slug text,
  has_active_link boolean,
  access_version bigint
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
  SELECT
    share.id,
    share.workspace_id,
    share.session_id,
    share.general_scope,
    share.general_workspace_id,
    share.public_slug,
    EXISTS (
      SELECT 1
      FROM public.session_share_links AS link
      WHERE link.share_id = share.id
        AND link.revoked_at IS NULL
    ),
    share.access_version
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL;
END;
$$;

CREATE OR REPLACE FUNCTION private.set_session_share_scope(
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
  v_actor_id uuid := private.require_permanent_user();
  v_share public.session_shares%ROWTYPE;
  v_previous_value text;
BEGIN
  IF p_general_scope NOT IN ('restricted', 'workspace', 'public') THEN
    RAISE EXCEPTION 'invalid general access scope'
      USING ERRCODE = '22023';
  END IF;

  IF (p_general_scope = 'workspace') <> (p_general_workspace_id IS NOT NULL) THEN
    RAISE EXCEPTION 'invalid general workspace'
      USING ERRCODE = '22023';
  END IF;

  v_share := private.require_session_share_manager(p_share_id);

  IF p_general_scope = 'workspace' AND NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    WHERE workspace.id = p_general_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = v_actor_id
      AND membership.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'general workspace not available'
      USING ERRCODE = '42501';
  END IF;

  IF v_share.general_scope = p_general_scope
    AND v_share.general_workspace_id IS NOT DISTINCT FROM p_general_workspace_id
  THEN
    RETURN QUERY
    SELECT
      v_share.id,
      v_share.general_scope,
      v_share.general_workspace_id,
      v_share.public_slug,
      v_share.access_version;
    RETURN;
  END IF;

  v_previous_value := CASE
    WHEN v_share.general_scope = 'workspace'
      THEN v_share.general_scope || ':' || v_share.general_workspace_id::text
    ELSE v_share.general_scope
  END;

  UPDATE public.session_share_links AS target_link
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now()
  WHERE target_link.share_id = v_share.id
    AND target_link.revoked_at IS NULL;

  UPDATE public.session_shares AS target_share
  SET
    general_scope = p_general_scope,
    general_workspace_id = p_general_workspace_id,
    access_version = target_share.access_version + 1,
    updated_at = now()
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;

  PERFORM private.write_session_access_event(
    v_share.id,
    'scope_changed',
    v_actor_id,
    NULL,
    NULL,
    v_previous_value,
    CASE
      WHEN v_share.general_scope = 'workspace'
        THEN v_share.general_scope || ':' || v_share.general_workspace_id::text
      ELSE v_share.general_scope
    END
  );

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.general_scope,
    v_share.general_workspace_id,
    v_share.public_slug,
    v_share.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.issue_session_share_link(
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
  v_actor_id uuid := private.require_permanent_user();
  v_share public.session_shares%ROWTYPE;
  v_link public.session_share_links%ROWTYPE;
  v_token text;
  v_had_active_link boolean := false;
BEGIN
  v_share := private.require_session_share_manager(p_share_id);

  SELECT link.*
  INTO v_link
  FROM public.session_share_links AS link
  WHERE link.share_id = v_share.id
    AND link.revoked_at IS NULL
  FOR UPDATE;

  v_had_active_link := FOUND;

  IF v_had_active_link
    AND NOT p_force_rotate
    AND v_share.general_scope = 'link'
  THEN
    RETURN QUERY
    SELECT
      v_share.id,
      v_link.id,
      NULL::text,
      v_share.access_version,
      false;
    RETURN;
  END IF;

  IF v_had_active_link THEN
    UPDATE public.session_share_links
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now()
    WHERE id = v_link.id;
  END IF;

  v_token := rtrim(
    translate(encode(extensions.gen_random_bytes(32), 'base64'), '+/', '-_'),
    '='
  );

  INSERT INTO public.session_share_links (
    share_id,
    token_hash,
    created_by_user_id
  ) VALUES (
    v_share.id,
    extensions.digest(v_token, 'sha256'),
    v_actor_id
  )
  RETURNING * INTO v_link;

  UPDATE public.session_shares AS target_share
  SET
    general_scope = 'link',
    general_workspace_id = NULL,
    access_version = target_share.access_version + 1,
    updated_at = now()
  WHERE target_share.id = v_share.id
  RETURNING * INTO v_share;

  PERFORM private.write_session_access_event(
    v_share.id,
    CASE WHEN v_had_active_link THEN 'link_rotated' ELSE 'link_enabled' END,
    v_actor_id,
    NULL,
    v_link.id,
    NULL,
    'link'
  );

  RETURN QUERY
  SELECT
    v_share.id,
    v_link.id,
    v_token,
    v_share.access_version,
    true;
END;
$$;

REVOKE ALL ON FUNCTION private.create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.get_session_share_management(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.set_session_share_scope(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.issue_session_share_link(uuid, boolean)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.get_session_share_management(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.set_session_share_scope(uuid, text, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.issue_session_share_link(uuid, boolean)
  TO authenticated;

CREATE OR REPLACE FUNCTION private.upsert_session_access_grant(
  p_share_id uuid,
  p_grantee_user_id uuid,
  p_capability text,
  p_actor_user_id uuid
)
RETURNS uuid
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_grant public.session_access_grants%ROWTYPE;
  v_previous_capability text;
  v_capability text := p_capability;
BEGIN
  IF private.session_capability_rank(p_capability) = 0 OR NOT EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = p_grantee_user_id
      AND auth_user.email_confirmed_at IS NOT NULL
      AND COALESCE(auth_user.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'invalid session access grant'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.share_id = p_share_id
    AND access_grant.grantee_user_id = p_grantee_user_id
  FOR UPDATE;

  IF FOUND THEN
    v_previous_capability := CASE
      WHEN v_grant.revoked_at IS NULL THEN v_grant.capability
      ELSE NULL
    END;

    IF v_grant.revoked_at IS NULL
      AND private.session_capability_rank(v_grant.capability)
        > private.session_capability_rank(v_capability)
    THEN
      v_capability := v_grant.capability;
    END IF;

    UPDATE public.session_access_grants
    SET
      capability = v_capability,
      granted_by_user_id = p_actor_user_id,
      revoked_by_user_id = NULL,
      revoked_at = NULL,
      updated_at = now()
    WHERE id = v_grant.id
    RETURNING * INTO v_grant;
  ELSE
    INSERT INTO public.session_access_grants (
      share_id,
      grantee_user_id,
      capability,
      granted_by_user_id
    ) VALUES (
      p_share_id,
      p_grantee_user_id,
      v_capability,
      p_actor_user_id
    )
    RETURNING * INTO v_grant;
  END IF;

  UPDATE public.session_shares
  SET
    access_version = access_version + 1,
    updated_at = now()
  WHERE id = p_share_id;

  PERFORM private.write_session_access_event(
    p_share_id,
    'grant_changed',
    p_actor_user_id,
    p_grantee_user_id,
    v_grant.id,
    v_previous_capability,
    v_grant.capability
  );

  RETURN v_grant.id;
END;
$$;

CREATE OR REPLACE FUNCTION private.create_session_access_invitation(
  p_share_id uuid,
  p_invitee_email text,
  p_capability text
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
  v_actor_id uuid := private.require_permanent_user();
  v_email text := lower(btrim(p_invitee_email));
  v_invitee_user_id uuid;
  v_existing public.session_access_invitations%ROWTYPE;
  v_invitation public.session_access_invitations%ROWTYPE;
  v_token text;
  v_expires_at timestamptz;
BEGIN
  IF v_email IS NULL
    OR v_email !~ '^[^[:space:]@]+@[^[:space:]@]+$'
    OR v_email ~ '[[:cntrl:]]'
    OR octet_length(v_email) > 320
    OR private.session_capability_rank(p_capability) = 0
  THEN
    RAISE EXCEPTION 'invalid session access invitation'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_session_share_manager(p_share_id);

  SELECT auth_user.id
  INTO v_invitee_user_id
  FROM auth.users AS auth_user
  WHERE lower(btrim(auth_user.email)) = v_email
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false
  ORDER BY auth_user.created_at, auth_user.id
  LIMIT 1;

  IF v_invitee_user_id = v_actor_id
    OR (
      v_invitee_user_id IS NOT NULL
      AND private.is_session_share_manager(p_share_id, v_invitee_user_id)
    )
    OR EXISTS (
      SELECT 1
      FROM public.session_access_grants AS access_grant
      WHERE access_grant.share_id = p_share_id
        AND access_grant.grantee_user_id = v_invitee_user_id
        AND access_grant.revoked_at IS NULL
        AND private.session_capability_rank(access_grant.capability)
          >= private.session_capability_rank(p_capability)
    )
  THEN
    RAISE EXCEPTION 'session access invitation not needed'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_existing
  FROM public.session_access_invitations AS invitation
  WHERE invitation.share_id = p_share_id
    AND invitation.invitee_email = v_email
    AND invitation.accepted_at IS NULL
    AND invitation.revoked_at IS NULL
  FOR UPDATE;

  IF FOUND
    AND v_existing.expires_at > now()
    AND v_existing.capability = p_capability
  THEN
    RETURN QUERY
    SELECT
      v_existing.id,
      NULL::text,
      v_existing.expires_at,
      false;
    RETURN;
  END IF;

  IF FOUND THEN
    UPDATE public.session_access_invitations
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = now(),
      updated_at = now()
    WHERE id = v_existing.id;

    PERFORM private.write_session_access_event(
      p_share_id,
      'invitation_revoked',
      v_actor_id,
      v_existing.invitee_user_id,
      v_existing.id,
      v_existing.capability,
      NULL
    );
  END IF;

  v_token := rtrim(
    translate(encode(extensions.gen_random_bytes(32), 'base64'), '+/', '-_'),
    '='
  );
  v_expires_at := now() + interval '30 days';

  INSERT INTO public.session_access_invitations (
    share_id,
    invitee_email,
    invitee_user_id,
    capability,
    token_hash,
    invited_by_user_id,
    expires_at
  ) VALUES (
    p_share_id,
    v_email,
    v_invitee_user_id,
    p_capability,
    extensions.digest(v_token, 'sha256'),
    v_actor_id,
    v_expires_at
  )
  RETURNING * INTO v_invitation;

  PERFORM private.write_session_access_event(
    p_share_id,
    'invitation_created',
    v_actor_id,
    v_invitee_user_id,
    v_invitation.id,
    NULL,
    p_capability
  );

  RETURN QUERY
  SELECT v_invitation.id, v_token, v_expires_at, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.resend_session_access_invitation(
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
  v_actor_id uuid := private.require_permanent_user();
  v_existing public.session_access_invitations%ROWTYPE;
  v_invitation public.session_access_invitations%ROWTYPE;
  v_token text;
  v_expires_at timestamptz;
BEGIN
  SELECT invitation.*
  INTO v_existing
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_existing.share_id);

  SELECT invitation.*
  INTO v_existing
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id
  FOR UPDATE;

  IF v_existing.accepted_at IS NOT NULL OR v_existing.revoked_at IS NOT NULL THEN
    RAISE EXCEPTION 'session access invitation is unavailable'
      USING ERRCODE = '22023';
  END IF;

  UPDATE public.session_access_invitations
  SET
    revoked_by_user_id = v_actor_id,
    revoked_at = now(),
    updated_at = now()
  WHERE id = v_existing.id;

  v_token := rtrim(
    translate(encode(extensions.gen_random_bytes(32), 'base64'), '+/', '-_'),
    '='
  );
  v_expires_at := now() + interval '30 days';

  INSERT INTO public.session_access_invitations (
    share_id,
    invitee_email,
    invitee_user_id,
    capability,
    token_hash,
    invited_by_user_id,
    expires_at
  ) VALUES (
    v_existing.share_id,
    v_existing.invitee_email,
    v_existing.invitee_user_id,
    v_existing.capability,
    extensions.digest(v_token, 'sha256'),
    v_actor_id,
    v_expires_at
  )
  RETURNING * INTO v_invitation;

  PERFORM private.write_session_access_event(
    v_invitation.share_id,
    'invitation_resent',
    v_actor_id,
    v_invitation.invitee_user_id,
    v_invitation.id,
    v_existing.id::text,
    v_invitation.capability
  );

  RETURN QUERY
  SELECT v_invitation.id, v_token, v_expires_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.accept_session_access_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  share_id uuid,
  grant_id uuid,
  capability text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_actor_email text;
  v_invitation public.session_access_invitations%ROWTYPE;
  v_grant public.session_access_grants%ROWTYPE;
  v_grant_id uuid;
BEGIN
  SELECT lower(btrim(auth_user.email))
  INTO v_actor_email
  FROM auth.users AS auth_user
  WHERE auth_user.id = v_actor_id;

  IF p_invite_token IS NULL
    OR p_invite_token !~ '^[A-Za-z0-9_-]{43}$'
  THEN
    RAISE EXCEPTION 'session access invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_invitation
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id
    AND invitation.token_hash = extensions.digest(p_invite_token, 'sha256');

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = v_invitation.share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT invitation.*
  INTO v_invitation
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id
    AND invitation.token_hash = extensions.digest(p_invite_token, 'sha256')
  FOR UPDATE;

  IF NOT FOUND
    OR v_invitation.revoked_at IS NOT NULL
    OR v_invitation.expires_at <= now()
    OR v_invitation.invitee_email <> v_actor_email
    OR (
      v_invitation.invitee_user_id IS NOT NULL
      AND v_invitation.invitee_user_id <> v_actor_id
    )
  THEN
    RAISE EXCEPTION 'session access invitation is invalid or unavailable'
      USING ERRCODE = '22023';
  END IF;

  IF v_invitation.accepted_at IS NOT NULL THEN
    SELECT access_grant.*
    INTO v_grant
    FROM public.session_access_grants AS access_grant
    WHERE access_grant.share_id = v_invitation.share_id
      AND access_grant.grantee_user_id = v_actor_id
      AND access_grant.revoked_at IS NULL;

    IF NOT FOUND OR v_invitation.accepted_by_user_id <> v_actor_id THEN
      RAISE EXCEPTION 'session access invitation is invalid or unavailable'
        USING ERRCODE = '22023';
    END IF;

    RETURN QUERY
    SELECT v_invitation.share_id, v_grant.id, v_grant.capability;
    RETURN;
  END IF;

  v_grant_id := private.upsert_session_access_grant(
    v_invitation.share_id,
    v_actor_id,
    v_invitation.capability,
    COALESCE(v_invitation.invited_by_user_id, v_actor_id)
  );

  UPDATE public.session_access_invitations
  SET
    invitee_user_id = v_actor_id,
    accepted_by_user_id = v_actor_id,
    accepted_at = now(),
    updated_at = now()
  WHERE id = v_invitation.id;

  PERFORM private.write_session_access_event(
    v_invitation.share_id,
    'invitation_accepted',
    v_actor_id,
    v_actor_id,
    v_invitation.id,
    NULL,
    v_invitation.capability
  );

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = v_grant_id;

  RETURN QUERY
  SELECT v_invitation.share_id, v_grant.id, v_grant.capability;
END;
$$;

CREATE OR REPLACE FUNCTION private.revoke_session_access_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  revoked_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_invitation public.session_access_invitations%ROWTYPE;
  v_revoked_at timestamptz;
BEGIN
  SELECT invitation.*
  INTO v_invitation
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_invitation.share_id);

  SELECT invitation.*
  INTO v_invitation
  FROM public.session_access_invitations AS invitation
  WHERE invitation.id = p_invitation_id
  FOR UPDATE;

  IF v_invitation.accepted_at IS NOT NULL THEN
    RAISE EXCEPTION 'accepted invitations require grant revocation'
      USING ERRCODE = '22023';
  END IF;

  IF v_invitation.revoked_at IS NULL THEN
    v_revoked_at := now();

    UPDATE public.session_access_invitations
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = v_revoked_at,
      updated_at = v_revoked_at
    WHERE id = v_invitation.id;

    PERFORM private.write_session_access_event(
      v_invitation.share_id,
      'invitation_revoked',
      v_actor_id,
      v_invitation.invitee_user_id,
      v_invitation.id,
      v_invitation.capability,
      NULL
    );
  ELSE
    v_revoked_at := v_invitation.revoked_at;
  END IF;

  RETURN QUERY
  SELECT v_invitation.id, v_revoked_at;
END;
$$;

REVOKE ALL ON FUNCTION private.upsert_session_access_grant(
  uuid,
  uuid,
  text,
  uuid
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.create_session_access_invitation(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.resend_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.accept_session_access_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.revoke_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.create_session_access_invitation(uuid, text, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.resend_session_access_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.accept_session_access_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.revoke_session_access_invitation(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION private.update_session_access_grant(
  p_grant_id uuid,
  p_capability text
)
RETURNS TABLE (
  grant_id uuid,
  capability text,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_grant public.session_access_grants%ROWTYPE;
  v_access_version bigint;
BEGIN
  IF private.session_capability_rank(p_capability) = 0 THEN
    RAISE EXCEPTION 'invalid session access capability'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_grant.share_id);

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id
  FOR UPDATE;

  IF v_grant.revoked_at IS NOT NULL THEN
    RAISE EXCEPTION 'session access grant is unavailable'
      USING ERRCODE = '22023';
  END IF;

  IF v_grant.capability <> p_capability THEN
    UPDATE public.session_access_grants
    SET
      capability = p_capability,
      updated_at = now()
    WHERE id = v_grant.id;

    UPDATE public.session_shares AS target_share
    SET
      access_version = target_share.access_version + 1,
      updated_at = now()
    WHERE target_share.id = v_grant.share_id
    RETURNING target_share.access_version INTO v_access_version;

    PERFORM private.write_session_access_event(
      v_grant.share_id,
      'grant_changed',
      v_actor_id,
      v_grant.grantee_user_id,
      v_grant.id,
      v_grant.capability,
      p_capability
    );
  ELSE
    SELECT share.access_version
    INTO v_access_version
    FROM public.session_shares AS share
    WHERE share.id = v_grant.share_id;
  END IF;

  RETURN QUERY
  SELECT v_grant.id, p_capability, v_access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.revoke_session_access_grant(
  p_grant_id uuid
)
RETURNS TABLE (
  grant_id uuid,
  revoked_at timestamptz,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_grant public.session_access_grants%ROWTYPE;
  v_revoked_at timestamptz;
  v_access_version bigint;
BEGIN
  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_grant.share_id);

  SELECT access_grant.*
  INTO v_grant
  FROM public.session_access_grants AS access_grant
  WHERE access_grant.id = p_grant_id
  FOR UPDATE;

  IF v_grant.revoked_at IS NULL THEN
    v_revoked_at := now();

    UPDATE public.session_access_grants
    SET
      revoked_by_user_id = v_actor_id,
      revoked_at = v_revoked_at,
      updated_at = v_revoked_at
    WHERE id = v_grant.id;

    UPDATE public.session_shares AS target_share
    SET
      access_version = target_share.access_version + 1,
      updated_at = now()
    WHERE target_share.id = v_grant.share_id
    RETURNING target_share.access_version INTO v_access_version;

    PERFORM private.write_session_access_event(
      v_grant.share_id,
      'grant_revoked',
      v_actor_id,
      v_grant.grantee_user_id,
      v_grant.id,
      v_grant.capability,
      NULL
    );
  ELSE
    v_revoked_at := v_grant.revoked_at;

    SELECT share.access_version
    INTO v_access_version
    FROM public.session_shares AS share
    WHERE share.id = v_grant.share_id;
  END IF;

  RETURN QUERY
  SELECT v_grant.id, v_revoked_at, v_access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.request_session_access(
  p_share_id uuid,
  p_requested_capability text
)
RETURNS TABLE (
  request_id uuid,
  requested_capability text,
  was_created boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_request public.session_access_requests%ROWTYPE;
  v_existing_capability text;
  v_manage_access boolean;
BEGIN
  IF private.session_capability_rank(p_requested_capability) = 0 OR NOT EXISTS (
    SELECT 1
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    WHERE share.id = p_share_id
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT access.capability, access.manage_access
  INTO v_existing_capability, v_manage_access
  FROM private.resolve_my_session_access(p_share_id) AS access;

  IF COALESCE(v_manage_access, false)
    OR private.session_capability_rank(v_existing_capability)
      >= private.session_capability_rank(p_requested_capability)
  THEN
    RAISE EXCEPTION 'session access request not needed'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.session_access_requests (
    share_id,
    requester_user_id,
    requested_capability
  ) VALUES (
    p_share_id,
    v_actor_id,
    p_requested_capability
  )
  ON CONFLICT (share_id, requester_user_id)
    WHERE status = 'pending'
    DO NOTHING
  RETURNING * INTO v_request;

  IF NOT FOUND THEN
    SELECT access_request.*
    INTO v_request
    FROM public.session_access_requests AS access_request
    WHERE access_request.share_id = p_share_id
      AND access_request.requester_user_id = v_actor_id
      AND access_request.status = 'pending';

    RETURN QUERY
    SELECT v_request.id, v_request.requested_capability, false;
    RETURN;
  END IF;

  PERFORM private.write_session_access_event(
    p_share_id,
    'request_created',
    v_actor_id,
    v_actor_id,
    v_request.id,
    NULL,
    p_requested_capability
  );

  RETURN QUERY
  SELECT v_request.id, v_request.requested_capability, true;
END;
$$;

CREATE OR REPLACE FUNCTION private.cancel_session_access_request(
  p_request_id uuid
)
RETURNS TABLE (
  request_id uuid,
  status text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_request public.session_access_requests%ROWTYPE;
BEGIN
  SELECT access_request.*
  INTO v_request
  FROM public.session_access_requests AS access_request
  WHERE access_request.id = p_request_id
    AND access_request.requester_user_id = v_actor_id;

  IF NOT FOUND OR v_request.status <> 'pending' THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = v_request.share_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_request.*
  INTO v_request
  FROM public.session_access_requests AS access_request
  WHERE access_request.id = p_request_id
    AND access_request.requester_user_id = v_actor_id
  FOR UPDATE;

  IF NOT FOUND OR v_request.status <> 'pending' THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  UPDATE public.session_access_requests
  SET
    status = 'cancelled',
    updated_at = now()
  WHERE id = v_request.id;

  PERFORM private.write_session_access_event(
    v_request.share_id,
    'request_cancelled',
    v_actor_id,
    v_actor_id,
    v_request.id,
    v_request.requested_capability,
    'cancelled'
  );

  RETURN QUERY
  SELECT v_request.id, 'cancelled'::text;
END;
$$;

CREATE OR REPLACE FUNCTION private.review_session_access_request(
  p_request_id uuid,
  p_decision text,
  p_capability text DEFAULT NULL
)
RETURNS TABLE (
  request_id uuid,
  status text,
  grant_id uuid,
  capability text
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
  v_request public.session_access_requests%ROWTYPE;
  v_grant_id uuid;
  v_capability text;
BEGIN
  IF p_decision NOT IN ('approved', 'denied')
    OR (
      p_decision = 'approved'
      AND private.session_capability_rank(p_capability) = 0
    )
    OR (p_decision = 'denied' AND p_capability IS NOT NULL)
  THEN
    RAISE EXCEPTION 'invalid access request decision'
      USING ERRCODE = '22023';
  END IF;

  SELECT access_request.*
  INTO v_request
  FROM public.session_access_requests AS access_request
  WHERE access_request.id = p_request_id;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM private.require_session_share_manager(v_request.share_id);

  SELECT access_request.*
  INTO v_request
  FROM public.session_access_requests AS access_request
  WHERE access_request.id = p_request_id
  FOR UPDATE;

  IF v_request.status <> 'pending' OR v_request.requester_user_id IS NULL THEN
    RAISE EXCEPTION 'session access request is unavailable'
      USING ERRCODE = '22023';
  END IF;

  IF p_decision = 'approved' THEN
    v_grant_id := private.upsert_session_access_grant(
      v_request.share_id,
      v_request.requester_user_id,
      p_capability,
      v_actor_id
    );

    SELECT access_grant.capability
    INTO v_capability
    FROM public.session_access_grants AS access_grant
    WHERE access_grant.id = v_grant_id;
  END IF;

  UPDATE public.session_access_requests
  SET
    status = p_decision,
    reviewed_by_user_id = v_actor_id,
    reviewed_at = now(),
    updated_at = now()
  WHERE id = v_request.id;

  PERFORM private.write_session_access_event(
    v_request.share_id,
    CASE
      WHEN p_decision = 'approved' THEN 'request_approved'
      ELSE 'request_denied'
    END,
    v_actor_id,
    v_request.requester_user_id,
    v_request.id,
    v_request.requested_capability,
    COALESCE(v_capability, 'denied')
  );

  RETURN QUERY
  SELECT v_request.id, p_decision, v_grant_id, v_capability;
END;
$$;

REVOKE ALL ON FUNCTION private.update_session_access_grant(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.revoke_session_access_grant(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.request_session_access(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.cancel_session_access_request(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.review_session_access_request(uuid, text, text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.update_session_access_grant(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.revoke_session_access_grant(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.request_session_access(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.cancel_session_access_request(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.review_session_access_request(uuid, text, text)
  TO authenticated;

CREATE OR REPLACE FUNCTION private.resolve_my_session_access(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
BEGIN
  RETURN QUERY
  WITH access_candidates AS (
    SELECT 3 AS capability_rank, true AS can_manage
    FROM public.session_shares AS candidate_share
    JOIN public.workspaces AS source_workspace
      ON source_workspace.id = candidate_share.workspace_id
    JOIN public.workspace_memberships AS source_membership
      ON source_membership.workspace_id = source_workspace.id
    WHERE candidate_share.id = p_share_id
      AND candidate_share.deleted_at IS NULL
      AND source_workspace.deleted_at IS NULL
      AND source_membership.user_id = v_actor_id
      AND source_membership.role IN ('owner', 'admin')
      AND source_membership.deleted_at IS NULL

    UNION ALL

    SELECT private.session_capability_rank(access_grant.capability), false
    FROM public.session_access_grants AS access_grant
    WHERE access_grant.share_id = p_share_id
      AND access_grant.grantee_user_id = v_actor_id
      AND access_grant.revoked_at IS NULL

    UNION ALL

    SELECT 1, false
    FROM public.session_shares AS candidate_share
    JOIN public.workspaces AS target_workspace
      ON target_workspace.id = candidate_share.general_workspace_id
    JOIN public.workspace_memberships AS target_membership
      ON target_membership.workspace_id = target_workspace.id
    WHERE candidate_share.id = p_share_id
      AND candidate_share.general_scope = 'workspace'
      AND candidate_share.deleted_at IS NULL
      AND target_workspace.deleted_at IS NULL
      AND target_membership.user_id = v_actor_id
      AND target_membership.deleted_at IS NULL

    UNION ALL

    SELECT 1, false
    FROM public.session_shares AS candidate_share
    WHERE candidate_share.id = p_share_id
      AND candidate_share.general_scope = 'public'
      AND candidate_share.deleted_at IS NULL
  ), effective_access AS (
    SELECT
      max(capability_rank) AS capability_rank,
      bool_or(can_manage) AS can_manage
    FROM access_candidates
  )
  SELECT
    share.id,
    share.workspace_id,
    share.session_id,
    CASE effective_access.capability_rank
      WHEN 1 THEN 'viewer'
      WHEN 2 THEN 'commenter'
      WHEN 3 THEN 'editor'
    END,
    effective_access.can_manage,
    share.access_version
  FROM public.session_shares AS share
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  CROSS JOIN effective_access
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND source_workspace.deleted_at IS NULL
    AND effective_access.capability_rank IS NOT NULL;
END;
$$;

CREATE OR REPLACE FUNCTION private.resolve_session_share_link(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  access_version bigint
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_identity_capability text;
  v_effective_capability text := 'viewer';
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.session_share_links AS link
    ON link.share_id = share.id
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND link.revoked_at IS NULL
    AND p_link_token ~ '^[A-Za-z0-9_-]{43}$'
    AND link.token_hash = extensions.digest(p_link_token, 'sha256');

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = auth.uid()
      AND auth_user.email_confirmed_at IS NOT NULL
      AND COALESCE(auth_user.is_anonymous, false) = false
  ) THEN
    SELECT access.capability
    INTO v_identity_capability
    FROM private.resolve_my_session_access(v_share.id) AS access;

    IF private.session_capability_rank(v_identity_capability) > 1 THEN
      v_effective_capability := v_identity_capability;
    END IF;
  END IF;

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.workspace_id,
    v_share.session_id,
    v_effective_capability,
    v_share.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.resolve_public_session_share(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  access_version bigint
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_identity_capability text;
  v_effective_capability text := 'viewer';
BEGIN
  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.public_slug = p_public_slug
    AND share.general_scope = 'public'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = auth.uid()
      AND auth_user.email_confirmed_at IS NOT NULL
      AND COALESCE(auth_user.is_anonymous, false) = false
  ) THEN
    SELECT access.capability
    INTO v_identity_capability
    FROM private.resolve_my_session_access(v_share.id) AS access;

    IF private.session_capability_rank(v_identity_capability) > 1 THEN
      v_effective_capability := v_identity_capability;
    END IF;
  END IF;

  RETURN QUERY
  SELECT
    v_share.id,
    v_share.workspace_id,
    v_share.session_id,
    v_effective_capability,
    v_share.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_my_accessible_sessions()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := private.require_permanent_user();
BEGIN
  RETURN QUERY
  WITH access_candidates AS (
    SELECT
      share.id AS candidate_share_id,
      3 AS capability_rank,
      true AS can_manage
    FROM public.session_shares AS share
    JOIN public.workspaces AS source_workspace
      ON source_workspace.id = share.workspace_id
    JOIN public.workspace_memberships AS source_membership
      ON source_membership.workspace_id = source_workspace.id
    WHERE share.deleted_at IS NULL
      AND source_workspace.deleted_at IS NULL
      AND source_membership.user_id = v_actor_id
      AND source_membership.role IN ('owner', 'admin')
      AND source_membership.deleted_at IS NULL

    UNION ALL

    SELECT
      access_grant.share_id,
      private.session_capability_rank(access_grant.capability),
      false
    FROM public.session_access_grants AS access_grant
    JOIN public.session_shares AS share
      ON share.id = access_grant.share_id
    WHERE access_grant.grantee_user_id = v_actor_id
      AND access_grant.revoked_at IS NULL
      AND share.deleted_at IS NULL

    UNION ALL

    SELECT share.id, 1, false
    FROM public.session_shares AS share
    JOIN public.workspaces AS target_workspace
      ON target_workspace.id = share.general_workspace_id
    JOIN public.workspace_memberships AS target_membership
      ON target_membership.workspace_id = target_workspace.id
    WHERE share.general_scope = 'workspace'
      AND share.deleted_at IS NULL
      AND target_workspace.deleted_at IS NULL
      AND target_membership.user_id = v_actor_id
      AND target_membership.deleted_at IS NULL
  ), effective_access AS (
    SELECT
      candidate_share_id,
      max(capability_rank) AS capability_rank,
      bool_or(can_manage) AS can_manage
    FROM access_candidates
    GROUP BY candidate_share_id
  )
  SELECT
    share.id,
    share.workspace_id,
    share.session_id,
    CASE effective_access.capability_rank
      WHEN 1 THEN 'viewer'
      WHEN 2 THEN 'commenter'
      WHEN 3 THEN 'editor'
    END,
    effective_access.can_manage,
    share.access_version
  FROM effective_access
  JOIN public.session_shares AS share
    ON share.id = effective_access.candidate_share_id
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  WHERE share.deleted_at IS NULL
    AND source_workspace.deleted_at IS NULL
  ORDER BY share.updated_at DESC, share.id;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_session_share_access(
  p_share_id uuid
)
RETURNS TABLE (
  entry_type text,
  entry_id uuid,
  user_id uuid,
  user_email text,
  capability text,
  status text,
  created_at timestamptz,
  expires_at timestamptz
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
  SELECT entries.*
  FROM (
    SELECT
      'grant'::text AS entry_type,
      access_grant.id AS entry_id,
      access_grant.grantee_user_id AS user_id,
      lower(btrim(grantee.email)) AS user_email,
      access_grant.capability,
      'active'::text AS status,
      access_grant.created_at,
      NULL::timestamptz AS expires_at
    FROM public.session_access_grants AS access_grant
    JOIN auth.users AS grantee
      ON grantee.id = access_grant.grantee_user_id
    WHERE access_grant.share_id = p_share_id
      AND access_grant.revoked_at IS NULL

    UNION ALL

    SELECT
      'invitation'::text,
      invitation.id,
      invitation.invitee_user_id,
      invitation.invitee_email,
      invitation.capability,
      'pending'::text,
      invitation.created_at,
      invitation.expires_at
    FROM public.session_access_invitations AS invitation
    WHERE invitation.share_id = p_share_id
      AND invitation.accepted_at IS NULL
      AND invitation.revoked_at IS NULL
      AND invitation.expires_at > now()

    UNION ALL

    SELECT
      'request'::text,
      access_request.id,
      access_request.requester_user_id,
      lower(btrim(requester.email)),
      access_request.requested_capability,
      access_request.status,
      access_request.created_at,
      NULL::timestamptz
    FROM public.session_access_requests AS access_request
    LEFT JOIN auth.users AS requester
      ON requester.id = access_request.requester_user_id
    WHERE access_request.share_id = p_share_id
      AND access_request.status = 'pending'
  ) AS entries
  ORDER BY entries.created_at, entries.entry_id;
END;
$$;

REVOKE ALL ON FUNCTION private.resolve_my_session_access(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.resolve_session_share_link(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.resolve_public_session_share(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_my_accessible_sessions()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_session_share_access(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.resolve_my_session_access(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.resolve_session_share_link(uuid, text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION private.resolve_public_session_share(text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_accessible_sessions()
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_session_share_access(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.create_session_share(
  p_workspace_id uuid,
  p_session_id text
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  public_slug text,
  access_version bigint,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.create_session_share(p_workspace_id, p_session_id);
$$;

CREATE OR REPLACE FUNCTION public.get_session_share_management(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  general_scope text,
  general_workspace_id uuid,
  public_slug text,
  has_active_link boolean,
  access_version bigint
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.get_session_share_management(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.set_session_share_scope(
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
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.set_session_share_scope(
    p_share_id,
    p_general_scope,
    p_general_workspace_id
  );
$$;

CREATE OR REPLACE FUNCTION public.enable_session_share_link(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.issue_session_share_link(p_share_id, false);
$$;

CREATE OR REPLACE FUNCTION public.rotate_session_share_link(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  link_id uuid,
  link_token text,
  access_version bigint,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.issue_session_share_link(p_share_id, true);
$$;

CREATE OR REPLACE FUNCTION public.create_session_access_invitation(
  p_share_id uuid,
  p_invitee_email text,
  p_capability text
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.create_session_access_invitation(
    p_share_id,
    p_invitee_email,
    p_capability
  );
$$;

CREATE OR REPLACE FUNCTION public.resend_session_access_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  invite_token text,
  invitation_expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.resend_session_access_invitation(p_invitation_id);
$$;

CREATE OR REPLACE FUNCTION public.accept_session_access_invitation(
  p_invitation_id uuid,
  p_invite_token text
)
RETURNS TABLE (
  share_id uuid,
  grant_id uuid,
  capability text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.accept_session_access_invitation(
    p_invitation_id,
    p_invite_token
  );
$$;

CREATE OR REPLACE FUNCTION public.revoke_session_access_invitation(
  p_invitation_id uuid
)
RETURNS TABLE (
  invitation_id uuid,
  revoked_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.revoke_session_access_invitation(p_invitation_id);
$$;

CREATE OR REPLACE FUNCTION public.update_session_access_grant(
  p_grant_id uuid,
  p_capability text
)
RETURNS TABLE (
  grant_id uuid,
  capability text,
  access_version bigint
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.update_session_access_grant(p_grant_id, p_capability);
$$;

CREATE OR REPLACE FUNCTION public.revoke_session_access_grant(
  p_grant_id uuid
)
RETURNS TABLE (
  grant_id uuid,
  revoked_at timestamptz,
  access_version bigint
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.revoke_session_access_grant(p_grant_id);
$$;

CREATE OR REPLACE FUNCTION public.request_session_access(
  p_share_id uuid,
  p_requested_capability text
)
RETURNS TABLE (
  request_id uuid,
  requested_capability text,
  was_created boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.request_session_access(p_share_id, p_requested_capability);
$$;

CREATE OR REPLACE FUNCTION public.cancel_session_access_request(
  p_request_id uuid
)
RETURNS TABLE (
  request_id uuid,
  status text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.cancel_session_access_request(p_request_id);
$$;

CREATE OR REPLACE FUNCTION public.review_session_access_request(
  p_request_id uuid,
  p_decision text,
  p_capability text DEFAULT NULL
)
RETURNS TABLE (
  request_id uuid,
  status text,
  grant_id uuid,
  capability text
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.review_session_access_request(
    p_request_id,
    p_decision,
    p_capability
  );
$$;

CREATE OR REPLACE FUNCTION public.resolve_my_session_access(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.resolve_my_session_access(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.resolve_session_share_link(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  access_version bigint
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.resolve_session_share_link(p_share_id, p_link_token);
$$;

CREATE OR REPLACE FUNCTION public.resolve_public_session_share(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  access_version bigint
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.resolve_public_session_share(p_public_slug);
$$;

CREATE OR REPLACE FUNCTION public.list_my_accessible_sessions()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  capability text,
  manage_access boolean,
  access_version bigint
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_my_accessible_sessions();
$$;

CREATE OR REPLACE FUNCTION public.list_session_share_access(
  p_share_id uuid
)
RETURNS TABLE (
  entry_type text,
  entry_id uuid,
  user_id uuid,
  user_email text,
  capability text,
  status text,
  created_at timestamptz,
  expires_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_session_share_access(p_share_id);
$$;

REVOKE ALL ON FUNCTION public.create_session_share(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.get_session_share_management(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.set_session_share_scope(uuid, text, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.enable_session_share_link(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.rotate_session_share_link(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.create_session_access_invitation(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.resend_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.accept_session_access_invitation(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.revoke_session_access_invitation(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.update_session_access_grant(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.revoke_session_access_grant(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.request_session_access(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.cancel_session_access_request(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.review_session_access_request(uuid, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.resolve_my_session_access(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.resolve_session_share_link(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.resolve_public_session_share(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_accessible_sessions()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_session_share_access(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.create_session_share(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.get_session_share_management(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.set_session_share_scope(uuid, text, uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.enable_session_share_link(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.rotate_session_share_link(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.create_session_access_invitation(uuid, text, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.resend_session_access_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.accept_session_access_invitation(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.revoke_session_access_invitation(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.update_session_access_grant(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.revoke_session_access_grant(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.request_session_access(uuid, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.cancel_session_access_request(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.review_session_access_request(uuid, text, text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.resolve_my_session_access(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.resolve_session_share_link(uuid, text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.resolve_public_session_share(text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_accessible_sessions()
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_session_share_access(uuid)
  TO authenticated;
