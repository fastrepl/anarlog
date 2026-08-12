CREATE OR REPLACE FUNCTION private.gateway_read_session_share_link_preview_by_id(
  p_link_id uuid
)
RETURNS TABLE (
  share_id uuid,
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
    share.id,
    snapshot.title,
    snapshot.body_json,
    COALESCE(metadata.participants, ARRAY[]::text[]),
    COALESCE(metadata.meeting_at, snapshot.published_at)
  FROM public.session_share_links AS link
  JOIN public.session_shares AS share
    ON share.id = link.share_id
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  LEFT JOIN public.session_share_preview_metadata AS metadata
    ON metadata.share_id = share.id
  WHERE link.id = p_link_id
    AND link.revoked_at IS NULL
    AND share.general_scope = 'link'
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_session_share_link_preview_by_id(
  p_link_id uuid
)
RETURNS TABLE (
  share_id uuid,
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
  FROM private.gateway_read_session_share_link_preview_by_id(p_link_id);
$$;

REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_preview_by_id(uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_preview_by_id(uuid)
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_preview_by_id(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_preview_by_id(uuid)
  TO service_role;
