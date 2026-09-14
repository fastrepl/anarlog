CREATE OR REPLACE FUNCTION private.list_my_managed_share_gallery_page(
  p_query text DEFAULT NULL,
  p_after_published_at timestamptz DEFAULT NULL,
  p_after_share_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 13
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  title text,
  body_json jsonb,
  has_snapshot boolean,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    share.id,
    share.general_scope,
    snapshot.title,
    snapshot.body_json,
    snapshot.share_id IS NOT NULL,
    COALESCE(snapshot.published_at, share.updated_at)
  FROM private.list_my_accessible_sessions() AS access
  JOIN public.session_shares AS share
    ON share.id = access.share_id
  LEFT JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  WHERE access.manage_access
    AND share.deleted_at IS NULL
    AND (
      NULLIF(btrim(p_query), '') IS NULL
      OR strpos(lower(COALESCE(snapshot.title, '')), lower(btrim(p_query))) > 0
    )
    AND (
      (p_after_published_at IS NULL AND p_after_share_id IS NULL)
      OR (
        p_after_published_at IS NOT NULL
        AND p_after_share_id IS NOT NULL
        AND (COALESCE(snapshot.published_at, share.updated_at), share.id)
          < (p_after_published_at, p_after_share_id)
      )
    )
  ORDER BY COALESCE(snapshot.published_at, share.updated_at) DESC, share.id DESC
  LIMIT LEAST(GREATEST(COALESCE(p_limit, 13), 1), 13);
$$;

REVOKE ALL ON FUNCTION private.list_my_managed_share_gallery_page(
  text,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_managed_share_gallery_page(
  text,
  timestamptz,
  uuid,
  integer
) TO authenticated;

CREATE OR REPLACE FUNCTION public.list_my_managed_share_gallery_page(
  p_query text DEFAULT NULL,
  p_after_published_at timestamptz DEFAULT NULL,
  p_after_share_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 13
)
RETURNS TABLE (
  share_id uuid,
  general_scope text,
  title text,
  body_json jsonb,
  has_snapshot boolean,
  published_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT page.*
  FROM private.list_my_managed_share_gallery_page(
    p_query,
    p_after_published_at,
    p_after_share_id,
    p_limit
  ) AS page;
$$;

REVOKE ALL ON FUNCTION public.list_my_managed_share_gallery_page(
  text,
  timestamptz,
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_managed_share_gallery_page(
  text,
  timestamptz,
  uuid,
  integer
) TO authenticated;
