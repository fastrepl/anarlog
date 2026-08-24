CREATE OR REPLACE FUNCTION private.gateway_read_stable_session_share_snapshot_v2(
  p_share_id uuid
)
RETURNS TABLE (
  general_scope text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    share.general_scope,
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.published_at
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE share.id = p_share_id
    AND share.general_scope IN ('link', 'public')
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND (
      share.general_scope = 'public'
      OR EXISTS (
        SELECT 1
        FROM public.session_share_links AS link
        WHERE link.share_id = share.id
          AND link.revoked_at IS NULL
      )
    );
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_stable_session_share_snapshot_v2(
  p_share_id uuid
)
RETURNS TABLE (
  general_scope text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_stable_session_share_snapshot_v2(p_share_id);
$$;

CREATE OR REPLACE FUNCTION private.gateway_read_stable_session_share_preview(
  p_share_id uuid
)
RETURNS TABLE (
  title text,
  body_json jsonb,
  participants text[],
  meeting_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    snapshot.title,
    snapshot.body_json,
    COALESCE(metadata.participants, ARRAY[]::text[]),
    COALESCE(metadata.meeting_at, snapshot.published_at)
  FROM private.gateway_read_stable_session_share_snapshot_v2(
    p_share_id
  ) AS snapshot
  LEFT JOIN public.session_share_preview_metadata AS metadata
    ON metadata.share_id = snapshot.share_id;
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_stable_session_share_preview(
  p_share_id uuid
)
RETURNS TABLE (
  title text,
  body_json jsonb,
  participants text[],
  meeting_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.gateway_read_stable_session_share_preview(p_share_id);
$$;

CREATE OR REPLACE FUNCTION private.gateway_create_stable_session_share_handoff(
  p_share_id uuid,
  p_source_hash text
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
  v_link_id uuid;
BEGIN
  IF p_source_hash IS NULL
    OR octet_length(p_source_hash) <> 64
    OR p_source_hash !~ '^[0-9a-f]{64}$'
  THEN
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
    AND share.general_scope IN ('link', 'public')
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  IF v_share.general_scope = 'link' THEN
    SELECT link.id
    INTO v_link_id
    FROM public.session_share_links AS link
    WHERE link.share_id = v_share.id
      AND link.revoked_at IS NULL;

    IF NOT FOUND THEN
      RETURN;
    END IF;
  END IF;

  RETURN QUERY
  SELECT handoff.request_id, handoff.expires_at
  FROM private.issue_session_share_handoff(
    v_share.id,
    v_share.general_scope,
    v_link_id,
    v_share.access_version,
    decode(p_source_hash, 'hex')
  ) AS handoff;
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_create_stable_session_share_handoff(
  p_share_id uuid,
  p_source_hash text
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
  FROM private.gateway_create_stable_session_share_handoff(
    p_share_id,
    p_source_hash
  );
$$;

CREATE OR REPLACE FUNCTION public.gateway_prepare_stable_session_share_attachment_download(
  p_share_id uuid,
  p_attachment_id uuid,
  p_download_expires_at timestamptz
)
RETURNS TABLE (
  share_id uuid,
  attachment_id uuid,
  object_key text,
  filename text,
  content_type text,
  size_bytes bigint,
  sha256 text,
  access_version bigint,
  cleanup_not_before timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  PERFORM 1
  FROM public.session_shares AS share
  WHERE share.id = p_share_id
    AND share.general_scope IN ('link', 'public')
    AND share.deleted_at IS NULL
    AND (
      share.general_scope = 'public'
      OR EXISTS (
        SELECT 1
        FROM public.session_share_links AS link
        WHERE link.share_id = share.id
          AND link.revoked_at IS NULL
      )
    )
  FOR SHARE OF share;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT *
  FROM private.prepare_session_share_attachment_download(
    p_share_id,
    p_attachment_id,
    p_download_expires_at
  );
END;
$$;

REVOKE ALL ON FUNCTION private.gateway_read_stable_session_share_snapshot_v2(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_stable_session_share_snapshot_v2(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_read_stable_session_share_preview(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_stable_session_share_preview(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_create_stable_session_share_handoff(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_create_stable_session_share_handoff(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_prepare_stable_session_share_attachment_download(
  uuid,
  uuid,
  timestamptz
) FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.gateway_read_stable_session_share_snapshot_v2(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_stable_session_share_snapshot_v2(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_stable_session_share_preview(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_stable_session_share_preview(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_create_stable_session_share_handoff(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_create_stable_session_share_handoff(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_prepare_stable_session_share_attachment_download(
  uuid,
  uuid,
  timestamptz
) TO service_role;
