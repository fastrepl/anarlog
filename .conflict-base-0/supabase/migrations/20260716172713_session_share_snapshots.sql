CREATE TABLE public.session_share_snapshots (
  share_id uuid PRIMARY KEY REFERENCES public.session_shares(id) ON DELETE CASCADE,
  schema_version smallint NOT NULL DEFAULT 1,
  content_revision bigint NOT NULL DEFAULT 1,
  title text NOT NULL DEFAULT '',
  body_json jsonb NOT NULL DEFAULT '{"type":"doc","content":[{"type":"paragraph"}]}'::jsonb,
  published_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  published_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT session_share_snapshots_schema_version_check CHECK (
    schema_version = 1
  ),
  CONSTRAINT session_share_snapshots_content_revision_check CHECK (
    content_revision > 0
  ),
  CONSTRAINT session_share_snapshots_title_check CHECK (
    title = btrim(title)
    AND octet_length(title) <= 4096
  ),
  CONSTRAINT session_share_snapshots_body_check CHECK (
    jsonb_typeof(body_json) = 'object'
    AND body_json ->> 'type' = 'doc'
    AND octet_length(body_json::text) <= 2097152
  )
);

ALTER TABLE public.session_share_snapshots ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_share_snapshots
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.session_share_snapshots TO service_role;
GRANT USAGE ON SCHEMA private TO service_role;

CREATE POLICY session_share_snapshots_service_all
  ON public.session_share_snapshots
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

ALTER TABLE public.session_access_events
  DROP CONSTRAINT session_access_events_type_check,
  ADD CONSTRAINT session_access_events_type_check CHECK (
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
      'request_denied',
      'snapshot_published'
    )
  );

CREATE OR REPLACE FUNCTION private.publish_session_share_snapshot(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_title text,
  p_body_json jsonb
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_title text := btrim(p_title);
  v_snapshot public.session_share_snapshots%ROWTYPE;
BEGIN
  PERFORM 1
  FROM public.session_shares AS share
  JOIN public.workspaces AS source_workspace
    ON source_workspace.id = share.workspace_id
  JOIN public.workspace_memberships AS membership
    ON membership.workspace_id = source_workspace.id
  JOIN auth.users AS actor
    ON actor.id = membership.user_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND source_workspace.deleted_at IS NULL
    AND membership.user_id = p_actor_user_id
    AND membership.role IN ('owner', 'admin')
    AND membership.deleted_at IS NULL
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
  FOR UPDATE OF share, source_workspace, membership;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot publication not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF v_title IS NULL
    OR octet_length(v_title) > 4096
    OR p_body_json IS NULL
    OR jsonb_typeof(p_body_json) <> 'object'
    OR p_body_json ->> 'type' <> 'doc'
    OR octet_length(p_body_json::text) > 2097152
  THEN
    RAISE EXCEPTION 'invalid session share snapshot'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.session_share_snapshots (
    share_id,
    schema_version,
    content_revision,
    title,
    body_json,
    published_by_user_id,
    published_at,
    updated_at
  ) VALUES (
    p_share_id,
    1,
    1,
    v_title,
    p_body_json,
    p_actor_user_id,
    now(),
    now()
  )
  ON CONFLICT ON CONSTRAINT session_share_snapshots_pkey DO UPDATE SET
    schema_version = 1,
    content_revision = public.session_share_snapshots.content_revision + 1,
    title = excluded.title,
    body_json = excluded.body_json,
    published_by_user_id = excluded.published_by_user_id,
    published_at = excluded.published_at,
    updated_at = excluded.updated_at
  RETURNING * INTO v_snapshot;

  PERFORM private.write_session_access_event(
    p_share_id,
    'snapshot_published',
    p_actor_user_id,
    NULL,
    p_share_id,
    CASE
      WHEN v_snapshot.content_revision > 1
        THEN (v_snapshot.content_revision - 1)::text
      ELSE NULL
    END,
    v_snapshot.content_revision::text
  );

  RETURN QUERY
  SELECT
    v_snapshot.share_id,
    v_snapshot.schema_version,
    v_snapshot.content_revision,
    v_snapshot.title,
    v_snapshot.body_json,
    v_snapshot.published_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.read_my_session_share_snapshot(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    access.capability,
    access.manage_access,
    access.access_version,
    snapshot.published_at
  FROM private.resolve_my_session_access(p_share_id) AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id;
$$;

CREATE OR REPLACE FUNCTION private.read_session_share_link_snapshot(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    access.capability,
    CASE
      WHEN auth.uid() IS NULL THEN false
      ELSE private.is_session_share_manager(access.share_id, auth.uid())
    END,
    access.access_version,
    snapshot.published_at
  FROM private.resolve_session_share_link(p_share_id, p_link_token) AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id;
$$;

CREATE OR REPLACE FUNCTION private.read_public_session_share_snapshot(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    access.capability,
    CASE
      WHEN auth.uid() IS NULL THEN false
      ELSE private.is_session_share_manager(access.share_id, auth.uid())
    END,
    access.access_version,
    snapshot.published_at
  FROM private.resolve_public_session_share(p_public_slug) AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id;
$$;

CREATE OR REPLACE FUNCTION private.list_my_session_share_snapshots()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    access.capability,
    access.manage_access,
    access.access_version,
    snapshot.published_at
  FROM private.list_my_accessible_sessions() AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id;
$$;

REVOKE ALL ON FUNCTION private.publish_session_share_snapshot(
  uuid,
  uuid,
  text,
  jsonb
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.read_my_session_share_snapshot(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.read_session_share_link_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.read_public_session_share_snapshot(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_my_session_share_snapshots()
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.publish_session_share_snapshot(
  uuid,
  uuid,
  text,
  jsonb
) TO service_role;
GRANT EXECUTE ON FUNCTION private.read_my_session_share_snapshot(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.read_session_share_link_snapshot(uuid, text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION private.read_public_session_share_snapshot(text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_session_share_snapshots()
  TO authenticated;

CREATE OR REPLACE FUNCTION public.publish_session_share_snapshot(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_title text,
  p_body_json jsonb
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.publish_session_share_snapshot(
    p_share_id,
    p_actor_user_id,
    p_title,
    p_body_json
  );
$$;

CREATE OR REPLACE FUNCTION public.read_my_session_share_snapshot(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.read_my_session_share_snapshot(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.read_session_share_link_snapshot(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.read_session_share_link_snapshot(p_share_id, p_link_token);
$$;

CREATE OR REPLACE FUNCTION public.read_public_session_share_snapshot(
  p_public_slug text
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.read_public_session_share_snapshot(p_public_slug);
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_snapshots()
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT snapshot.*
  FROM private.list_my_session_share_snapshots() AS snapshot
  ORDER BY snapshot.share_id;
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_snapshot_page(
  p_after_share_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 100
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT snapshot.*
  FROM private.list_my_session_share_snapshots() AS snapshot
  WHERE p_after_share_id IS NULL OR snapshot.share_id > p_after_share_id
  ORDER BY snapshot.share_id
  LIMIT LEAST(GREATEST(COALESCE(p_limit, 100), 1), 100);
$$;

REVOKE ALL ON FUNCTION public.publish_session_share_snapshot(
  uuid,
  uuid,
  text,
  jsonb
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_my_session_share_snapshot(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_session_share_link_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_public_session_share_snapshot(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_session_share_snapshots()
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_session_share_snapshot_page(uuid, integer)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.publish_session_share_snapshot(
  uuid,
  uuid,
  text,
  jsonb
) TO service_role;
GRANT EXECUTE ON FUNCTION public.read_my_session_share_snapshot(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.read_session_share_link_snapshot(uuid, text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.read_public_session_share_snapshot(text)
  TO anon, authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_snapshots()
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_snapshot_page(uuid, integer)
  TO authenticated;
