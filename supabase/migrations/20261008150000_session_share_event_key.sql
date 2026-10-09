ALTER TABLE public.session_share_preview_metadata
  ADD COLUMN event_key text NOT NULL DEFAULT '',
  ADD CONSTRAINT session_share_preview_metadata_event_key_check CHECK (
    char_length(event_key) <= 512
  );

CREATE OR REPLACE FUNCTION private.set_session_share_event_key(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_event_key text
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

  UPDATE public.session_share_preview_metadata
  SET event_key = COALESCE(p_event_key, '')
  WHERE share_id = p_share_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.set_session_share_event_key(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_event_key text
)
RETURNS void
LANGUAGE sql
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT private.set_session_share_event_key(
    p_share_id,
    p_actor_user_id,
    p_event_key
  );
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_event_keys()
RETURNS TABLE (
  share_id uuid,
  event_key text
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT access.share_id, metadata.event_key
  FROM private.list_my_accessible_sessions() AS access
  JOIN public.session_share_preview_metadata AS metadata
    ON metadata.share_id = access.share_id
  WHERE metadata.event_key <> ''
  ORDER BY access.share_id;
$$;

REVOKE ALL ON FUNCTION private.set_session_share_event_key(uuid, uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.set_session_share_event_key(uuid, uuid, text)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION public.list_my_session_share_event_keys()
  FROM PUBLIC, anon, service_role;

GRANT EXECUTE ON FUNCTION private.set_session_share_event_key(uuid, uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.set_session_share_event_key(uuid, uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_event_keys()
  TO authenticated;
