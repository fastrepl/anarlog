CREATE TABLE public.session_share_preview_metadata (
  share_id uuid PRIMARY KEY REFERENCES public.session_shares(id) ON DELETE CASCADE,
  participants text[] NOT NULL DEFAULT ARRAY[]::text[],
  meeting_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT session_share_preview_metadata_participants_check CHECK (
    cardinality(participants) <= 32
    AND array_position(participants, NULL) IS NULL
  )
);

ALTER TABLE public.session_share_preview_metadata ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_share_preview_metadata
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.session_share_preview_metadata TO service_role;

CREATE POLICY session_share_preview_metadata_service_all
  ON public.session_share_preview_metadata
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.upsert_session_share_preview_metadata(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_participants text[],
  p_meeting_at timestamptz
)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  IF p_participants IS NULL
    OR p_meeting_at IS NULL
    OR cardinality(p_participants) > 32
    OR EXISTS (
      SELECT 1
      FROM unnest(p_participants) AS participant(name)
      WHERE participant.name IS NULL
        OR participant.name <> btrim(participant.name)
        OR participant.name = ''
        OR char_length(participant.name) > 100
    )
  THEN
    RAISE EXCEPTION 'invalid session share preview metadata'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.session_share_preview_metadata (
    share_id,
    participants,
    meeting_at
  ) VALUES (
    p_share_id,
    p_participants,
    p_meeting_at
  )
  ON CONFLICT (share_id) DO UPDATE SET
    participants = excluded.participants,
    meeting_at = excluded.meeting_at,
    updated_at = now();
END;
$$;

CREATE OR REPLACE FUNCTION public.upsert_session_share_preview_metadata(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_participants text[],
  p_meeting_at timestamptz
)
RETURNS void
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT private.upsert_session_share_preview_metadata(
    p_share_id,
    p_actor_user_id,
    p_participants,
    p_meeting_at
  );
$$;

DROP FUNCTION public.gateway_read_session_share_link_preview(uuid, text);
DROP FUNCTION private.gateway_read_session_share_link_preview(uuid, text);

CREATE OR REPLACE FUNCTION private.gateway_read_session_share_link_preview(
  p_share_id uuid,
  p_preview_token text
)
RETURNS TABLE (
  title text,
  body_json jsonb,
  participants text[],
  meeting_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_preview_token IS NULL
    OR octet_length(p_preview_token) <> 64
    OR p_preview_token !~ '^[0-9a-f]{64}$'
  THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT
    snapshot.title,
    snapshot.body_json,
    COALESCE(metadata.participants, ARRAY[]::text[]),
    COALESCE(metadata.meeting_at, snapshot.published_at)
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = share.id
  JOIN public.session_share_links AS link
    ON link.share_id = share.id
  LEFT JOIN public.session_share_preview_metadata AS metadata
    ON metadata.share_id = share.id
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
  FROM private.gateway_read_session_share_link_preview(
    p_share_id,
    p_preview_token
  );
$$;

CREATE OR REPLACE FUNCTION private.gateway_read_public_session_share_preview(
  p_public_slug text
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
  FROM private.gateway_read_public_session_share_snapshot(p_public_slug) AS snapshot
  LEFT JOIN public.session_share_preview_metadata AS metadata
    ON metadata.share_id = snapshot.share_id;
$$;

CREATE OR REPLACE FUNCTION public.gateway_read_public_session_share_preview(
  p_public_slug text
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
  FROM private.gateway_read_public_session_share_preview(p_public_slug);
$$;

REVOKE ALL ON FUNCTION private.upsert_session_share_preview_metadata(
  uuid, uuid, text[], timestamptz
) FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.upsert_session_share_preview_metadata(
  uuid, uuid, text[], timestamptz
) FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_read_session_share_link_preview(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_session_share_link_preview(uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.gateway_read_public_session_share_preview(text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.gateway_read_public_session_share_preview(text)
  FROM PUBLIC, anon, authenticated, service_role;

GRANT EXECUTE ON FUNCTION private.upsert_session_share_preview_metadata(
  uuid, uuid, text[], timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION public.upsert_session_share_preview_metadata(
  uuid, uuid, text[], timestamptz
) TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_session_share_link_preview(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_session_share_link_preview(uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.gateway_read_public_session_share_preview(text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.gateway_read_public_session_share_preview(text)
  TO service_role;
