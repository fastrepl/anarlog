CREATE OR REPLACE FUNCTION private.gateway_read_session_share_link_preview(
  p_share_id uuid,
  p_preview_token text
)
RETURNS TABLE (
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  published_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_preview_token IS NULL OR octet_length(p_preview_token) <> 64 THEN
    RETURN;
  END IF;
  IF p_preview_token !~ '^[0-9a-f]{64}$' THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT
    snapshot.share_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    '[]'::jsonb,
    snapshot.published_at
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  JOIN public.session_share_links AS link
    ON link.share_id = share.id
  WHERE share.id = p_share_id
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
    AND link.revoked_at IS NULL
    AND link.token_hash = decode(p_preview_token, 'hex');
END;
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_session_share_link_preview(
  p_share_id uuid,
  p_preview_token text
)
RETURNS TABLE (
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
  FROM private.gateway_read_session_share_link_preview(
    p_share_id,
    p_preview_token
  );
$$;

REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_preview(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_preview(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_preview(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_preview(uuid, text)
  TO service_role;
