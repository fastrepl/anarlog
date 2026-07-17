CREATE TABLE private.session_share_handoffs (
  request_hash bytea PRIMARY KEY,
  share_id uuid NOT NULL REFERENCES public.session_shares(id) ON DELETE CASCADE,
  slot smallint NOT NULL,
  access_kind text NOT NULL,
  link_id uuid REFERENCES public.session_share_links(id) ON DELETE CASCADE,
  access_version bigint NOT NULL,
  created_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL,
  CONSTRAINT session_share_handoffs_request_hash_check CHECK (
    octet_length(request_hash) = 32
  ),
  CONSTRAINT session_share_handoffs_slot_check CHECK (
    slot BETWEEN 0 AND 3
  ),
  CONSTRAINT session_share_handoffs_access_kind_check CHECK (
    access_kind IN ('link', 'public')
  ),
  CONSTRAINT session_share_handoffs_link_check CHECK (
    (access_kind = 'link' AND link_id IS NOT NULL)
    OR (access_kind = 'public' AND link_id IS NULL)
  ),
  CONSTRAINT session_share_handoffs_access_version_check CHECK (
    access_version > 0
  ),
  CONSTRAINT session_share_handoffs_ttl_check CHECK (
    expires_at = created_at + interval '60 seconds'
  ),
  CONSTRAINT session_share_handoffs_share_slot_key UNIQUE (share_id, slot)
);

ALTER TABLE private.session_share_handoffs ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE private.session_share_handoffs
  FROM PUBLIC, anon, authenticated, service_role;

CREATE OR REPLACE FUNCTION private.gateway_read_session_share_link_snapshot(
  p_share_id uuid,
  p_link_token text
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
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_link_token IS NULL OR octet_length(p_link_token) <> 43 THEN
    RETURN;
  END IF;
  IF p_link_token !~ '^[A-Za-z0-9_-]{43}$' THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    snapshot.published_at
  FROM private.read_session_share_link_snapshot(
    p_share_id,
    p_link_token
  ) AS snapshot;
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_read_public_session_share_snapshot(
  p_public_slug text
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
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_public_slug IS NULL OR octet_length(p_public_slug) <> 34 THEN
    RETURN;
  END IF;
  IF p_public_slug !~ '^s_[0-9a-f]{32}$' THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    snapshot.published_at
  FROM private.read_public_session_share_snapshot(p_public_slug) AS snapshot;
END;
$$;

CREATE OR REPLACE FUNCTION private.issue_session_share_handoff(
  p_share_id uuid,
  p_access_kind text,
  p_link_id uuid,
  p_access_version bigint
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_created_at timestamptz := clock_timestamp();
  v_request_id text;
  v_slot smallint;
BEGIN
  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_share_id::text, 0)
  );

  SELECT candidate.slot::smallint
  INTO v_slot
  FROM pg_catalog.generate_series(0, 3) AS candidate(slot)
  LEFT JOIN private.session_share_handoffs AS handoff
    ON handoff.share_id = p_share_id
    AND handoff.slot = candidate.slot
  ORDER BY
    (handoff.slot IS NULL) DESC,
    (handoff.expires_at <= v_created_at) DESC,
    handoff.created_at ASC NULLS FIRST,
    candidate.slot
  LIMIT 1;

  v_request_id := gen_random_uuid()::text;

  INSERT INTO private.session_share_handoffs (
    request_hash,
    share_id,
    slot,
    access_kind,
    link_id,
    access_version,
    created_at,
    expires_at
  ) VALUES (
    extensions.digest(v_request_id, 'sha256'),
    p_share_id,
    v_slot,
    p_access_kind,
    p_link_id,
    p_access_version,
    v_created_at,
    v_created_at + interval '60 seconds'
  )
  ON CONFLICT (share_id, slot) DO UPDATE SET
    request_hash = excluded.request_hash,
    access_kind = excluded.access_kind,
    link_id = excluded.link_id,
    access_version = excluded.access_version,
    created_at = excluded.created_at,
    expires_at = excluded.expires_at;

  RETURN QUERY
  SELECT v_request_id, v_created_at + interval '60 seconds';
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_create_session_share_link_handoff(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_link public.session_share_links%ROWTYPE;
BEGIN
  IF p_link_token IS NULL OR octet_length(p_link_token) <> 43 THEN
    RETURN;
  END IF;
  IF p_link_token !~ '^[A-Za-z0-9_-]{43}$' THEN
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.id = p_share_id
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT link.*
  INTO v_link
  FROM public.session_share_links AS link
  WHERE link.share_id = v_share.id
    AND link.revoked_at IS NULL
    AND link.token_hash = extensions.digest(p_link_token, 'sha256');

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT handoff.request_id, handoff.expires_at
  FROM private.issue_session_share_handoff(
    v_share.id,
    'link',
    v_link.id,
    v_share.access_version
  ) AS handoff;
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_create_public_session_share_handoff(
  p_public_slug text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
BEGIN
  IF p_public_slug IS NULL OR octet_length(p_public_slug) <> 34 THEN
    RETURN;
  END IF;
  IF p_public_slug !~ '^s_[0-9a-f]{32}$' THEN
    RETURN;
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.public_slug = p_public_slug
    AND share.general_scope = 'public'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT handoff.request_id, handoff.expires_at
  FROM private.issue_session_share_handoff(
    v_share.id,
    'public',
    NULL,
    v_share.access_version
  ) AS handoff;
END;
$$;

CREATE OR REPLACE FUNCTION private.gateway_claim_session_share_handoff(
  p_request_id text
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
  v_now timestamptz := clock_timestamp();
  v_handoff private.session_share_handoffs%ROWTYPE;
BEGIN
  IF p_request_id IS NULL OR octet_length(p_request_id) <> 36 THEN
    RETURN;
  END IF;
  IF p_request_id !~ '^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$' THEN
    RETURN;
  END IF;

  DELETE FROM private.session_share_handoffs AS handoff
  WHERE handoff.request_hash = extensions.digest(p_request_id, 'sha256')
  RETURNING * INTO v_handoff;

  IF NOT FOUND OR v_handoff.expires_at <= v_now THEN
    RETURN;
  END IF;

  IF v_handoff.access_kind = 'public' THEN
    RETURN QUERY
    SELECT
      share.id,
      snapshot.schema_version,
      snapshot.content_revision,
      snapshot.title,
      snapshot.body_json,
      snapshot.published_at
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    JOIN public.session_share_snapshots AS snapshot
      ON snapshot.share_id = share.id
    WHERE share.id = v_handoff.share_id
      AND share.general_scope = 'public'
      AND share.access_version = v_handoff.access_version
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL;
  ELSE
    RETURN QUERY
    SELECT
      share.id,
      snapshot.schema_version,
      snapshot.content_revision,
      snapshot.title,
      snapshot.body_json,
      snapshot.published_at
    FROM public.session_shares AS share
    JOIN public.workspaces AS workspace
      ON workspace.id = share.workspace_id
    JOIN public.session_share_links AS link
      ON link.id = v_handoff.link_id
      AND link.share_id = share.id
    JOIN public.session_share_snapshots AS snapshot
      ON snapshot.share_id = share.id
    WHERE share.id = v_handoff.share_id
      AND share.general_scope = 'link'
      AND share.access_version = v_handoff.access_version
      AND share.deleted_at IS NULL
      AND workspace.deleted_at IS NULL
      AND link.revoked_at IS NULL;
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_read_public_session_share_snapshot(text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.issue_session_share_handoff(uuid, text, uuid, bigint)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_create_session_share_link_handoff(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_create_public_session_share_handoff(text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_claim_session_share_handoff(text)
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_snapshot(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_public_session_share_snapshot(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_create_session_share_link_handoff(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_create_public_session_share_handoff(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_claim_session_share_handoff(text)
  TO service_role;

CREATE OR REPLACE FUNCTION public.gateway_read_session_share_link_snapshot(
  p_share_id uuid,
  p_link_token text
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
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_session_share_link_snapshot(
    p_share_id,
    p_link_token
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_public_session_share_snapshot(
  p_public_slug text
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
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_public_session_share_snapshot(p_public_slug);
$$;

CREATE OR REPLACE FUNCTION public.gateway_create_session_share_link_handoff(
  p_share_id uuid,
  p_link_token text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_create_session_share_link_handoff(
    p_share_id,
    p_link_token
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_create_public_session_share_handoff(
  p_public_slug text
)
RETURNS TABLE (
  request_id text,
  expires_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_create_public_session_share_handoff(p_public_slug);
$$;

CREATE OR REPLACE FUNCTION public.gateway_claim_session_share_handoff(
  p_request_id text
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
  FROM private.gateway_claim_session_share_handoff(p_request_id);
$$;

REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_public_session_share_snapshot(text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_create_session_share_link_handoff(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_create_public_session_share_handoff(text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_claim_session_share_handoff(text)
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_snapshot(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_public_session_share_snapshot(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_create_session_share_link_handoff(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_create_public_session_share_handoff(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_claim_session_share_handoff(text)
  TO service_role;

REVOKE EXECUTE ON FUNCTION private.resolve_session_share_link(uuid, text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION private.resolve_public_session_share(text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION private.read_session_share_link_snapshot(uuid, text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION private.read_public_session_share_snapshot(text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION public.resolve_session_share_link(uuid, text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION public.resolve_public_session_share(text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION public.read_session_share_link_snapshot(uuid, text)
  FROM anon, authenticated;
REVOKE EXECUTE ON FUNCTION public.read_public_session_share_snapshot(text)
  FROM anon, authenticated;

REVOKE USAGE ON SCHEMA private FROM anon;
