CREATE TABLE public.api_cloud_settings (
  user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  enabled boolean NOT NULL DEFAULT false,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE public.api_session_snapshots (
  user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  session_id text NOT NULL,
  revision bigint NOT NULL DEFAULT 1,
  title text NOT NULL DEFAULT '',
  series_id text NOT NULL DEFAULT '',
  started_at text NOT NULL DEFAULT '',
  updated_at text NOT NULL DEFAULT '',
  content_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  published_at timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, session_id),
  CONSTRAINT api_session_snapshots_session_id_check CHECK (
    session_id = btrim(session_id)
    AND octet_length(session_id) BETWEEN 1 AND 512
  ),
  CONSTRAINT api_session_snapshots_revision_check CHECK (revision > 0),
  CONSTRAINT api_session_snapshots_title_check CHECK (
    title = btrim(title)
    AND octet_length(title) <= 4096
  ),
  CONSTRAINT api_session_snapshots_content_check CHECK (
    jsonb_typeof(content_json) = 'object'
    AND content_json ->> 'id' = session_id
    AND jsonb_typeof(content_json -> 'transcripts') = 'array'
    AND octet_length(content_json::text) <= 2097152
  )
);

CREATE INDEX api_session_snapshots_user_started_idx
  ON public.api_session_snapshots (user_id, started_at DESC, session_id DESC);
CREATE INDEX api_session_snapshots_user_series_started_idx
  ON public.api_session_snapshots (user_id, series_id, started_at DESC, session_id DESC);

CREATE TABLE public.api_keys (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  name text NOT NULL,
  key_hash text NOT NULL UNIQUE,
  key_prefix text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz,
  revoked_at timestamptz,
  CONSTRAINT api_keys_name_check CHECK (
    name = btrim(name)
    AND octet_length(name) BETWEEN 1 AND 128
  ),
  CONSTRAINT api_keys_hash_check CHECK (key_hash ~ '^[0-9a-f]{64}$'),
  CONSTRAINT api_keys_prefix_check CHECK (key_prefix ~ '^anl_[0-9a-f]{8}$')
);

CREATE INDEX api_keys_user_created_idx
  ON public.api_keys (user_id, created_at DESC)
  WHERE revoked_at IS NULL;

ALTER TABLE public.api_cloud_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.api_session_snapshots ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.api_keys ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE
  public.api_cloud_settings,
  public.api_session_snapshots,
  public.api_keys
FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE
  public.api_cloud_settings,
  public.api_session_snapshots,
  public.api_keys
TO service_role;

CREATE POLICY api_cloud_settings_service_all
  ON public.api_cloud_settings
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY api_session_snapshots_service_all
  ON public.api_session_snapshots
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY api_keys_service_all
  ON public.api_keys
  FOR ALL TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.cloud_api_user_has_pro(p_user_id uuid)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    EXISTS (
      SELECT 1
      FROM public.profiles AS profile
      JOIN stripe.active_entitlements AS entitlement
        ON entitlement.customer = profile.stripe_customer_id
      WHERE profile.id = p_user_id
        AND entitlement.lookup_key = 'hyprnote_pro'
    )
    OR EXISTS (
      SELECT 1
      FROM public.profiles AS profile
      JOIN stripe.subscriptions AS subscription
        ON subscription.customer = profile.stripe_customer_id
      WHERE profile.id = p_user_id
        AND subscription.status = 'trialing'
        AND (subscription.trial_end #>> '{}')::bigint
          > extract(epoch FROM now())::bigint
    );
$$;

CREATE OR REPLACE FUNCTION public.get_cloud_api_settings(p_actor_user_id uuid)
RETURNS TABLE (enabled boolean, updated_at timestamptz)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    COALESCE(settings.enabled, false),
    settings.updated_at
  FROM (SELECT 1) AS singleton
  LEFT JOIN public.api_cloud_settings AS settings
    ON settings.user_id = p_actor_user_id;
$$;

CREATE OR REPLACE FUNCTION public.set_cloud_api_enabled(
  p_actor_user_id uuid,
  p_enabled boolean
)
RETURNS TABLE (enabled boolean, updated_at timestamptz)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_settings public.api_cloud_settings%ROWTYPE;
BEGIN
  IF p_actor_user_id IS NULL OR p_enabled IS NULL THEN
    RAISE EXCEPTION 'invalid cloud API settings'
      USING ERRCODE = '22023';
  END IF;

  IF p_enabled AND NOT private.cloud_api_user_has_pro(p_actor_user_id) THEN
    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;

  PERFORM pg_advisory_xact_lock(
    hashtextextended('cloud_api:' || p_actor_user_id::text, 0)
  );

  INSERT INTO public.api_cloud_settings (user_id, enabled)
  VALUES (p_actor_user_id, p_enabled)
  ON CONFLICT (user_id) DO UPDATE SET
    enabled = excluded.enabled,
    updated_at = now()
  RETURNING * INTO v_settings;

  IF NOT p_enabled THEN
    DELETE FROM public.api_session_snapshots
    WHERE user_id = p_actor_user_id;
  END IF;

  RETURN QUERY SELECT v_settings.enabled, v_settings.updated_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.publish_cloud_api_snapshot(
  p_actor_user_id uuid,
  p_session_id text,
  p_content_json jsonb
)
RETURNS TABLE (
  session_id text,
  revision bigint,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_session_id text := btrim(p_session_id);
  v_snapshot public.api_session_snapshots%ROWTYPE;
BEGIN
  PERFORM pg_advisory_xact_lock(
    hashtextextended('cloud_api:' || p_actor_user_id::text, 0)
  );

  IF NOT private.cloud_api_user_has_pro(p_actor_user_id) THEN
    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM public.api_cloud_settings AS settings
    WHERE settings.user_id = p_actor_user_id
      AND settings.enabled
  ) THEN
    RAISE EXCEPTION 'cloud API not enabled'
      USING ERRCODE = '42501';
  END IF;

  IF v_session_id IS NULL
    OR octet_length(v_session_id) NOT BETWEEN 1 AND 512
    OR p_content_json IS NULL
    OR jsonb_typeof(p_content_json) <> 'object'
    OR p_content_json ->> 'id' <> v_session_id
    OR jsonb_typeof(p_content_json -> 'transcripts') <> 'array'
    OR octet_length(p_content_json::text) > 2097152
  THEN
    RAISE EXCEPTION 'invalid cloud API snapshot'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.api_session_snapshots (
    user_id,
    session_id,
    revision,
    title,
    series_id,
    started_at,
    updated_at,
    content_json,
    published_at
  ) VALUES (
    p_actor_user_id,
    v_session_id,
    1,
    btrim(COALESCE(p_content_json ->> 'title', '')),
    COALESCE(p_content_json ->> 'series_id', ''),
    COALESCE(p_content_json ->> 'started_at', ''),
    COALESCE(p_content_json ->> 'updated_at', ''),
    p_content_json,
    now()
  )
  ON CONFLICT ON CONSTRAINT api_session_snapshots_pkey DO UPDATE SET
    revision = public.api_session_snapshots.revision + 1,
    title = excluded.title,
    series_id = excluded.series_id,
    started_at = excluded.started_at,
    updated_at = excluded.updated_at,
    content_json = excluded.content_json,
    published_at = excluded.published_at
  RETURNING * INTO v_snapshot;

  RETURN QUERY
  SELECT v_snapshot.session_id, v_snapshot.revision, v_snapshot.published_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.delete_cloud_api_snapshot(
  p_actor_user_id uuid,
  p_session_id text
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  DELETE FROM public.api_session_snapshots
  WHERE user_id = p_actor_user_id
    AND session_id = p_session_id;
  RETURN FOUND;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_cloud_api_snapshots(
  p_user_id uuid,
  p_query text DEFAULT NULL,
  p_series_id text DEFAULT NULL,
  p_limit integer DEFAULT 21,
  p_offset integer DEFAULT 0
)
RETURNS TABLE (content_json jsonb)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT snapshot.content_json
  FROM public.api_session_snapshots AS snapshot
  WHERE snapshot.user_id = p_user_id
    AND (
      NULLIF(btrim(p_query), '') IS NULL
      OR strpos(lower(snapshot.title), lower(btrim(p_query))) > 0
      OR strpos(lower(snapshot.session_id), lower(btrim(p_query))) > 0
    )
    AND (
      NULLIF(btrim(p_series_id), '') IS NULL
      OR snapshot.series_id = btrim(p_series_id)
    )
  ORDER BY
    COALESCE(
      NULLIF(snapshot.started_at, ''),
      NULLIF(snapshot.content_json ->> 'created_at', '')
    ) DESC NULLS LAST,
    snapshot.created_at DESC,
    snapshot.session_id DESC
  LIMIT greatest(1, least(p_limit, 201))
  OFFSET greatest(p_offset, 0);
$$;

CREATE OR REPLACE FUNCTION public.read_cloud_api_snapshot(
  p_user_id uuid,
  p_session_id text
)
RETURNS TABLE (content_json jsonb)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT snapshot.content_json
  FROM public.api_session_snapshots AS snapshot
  WHERE snapshot.user_id = p_user_id
    AND snapshot.session_id = p_session_id;
$$;

CREATE OR REPLACE FUNCTION public.create_cloud_api_key(
  p_actor_user_id uuid,
  p_id uuid,
  p_name text,
  p_key_hash text,
  p_key_prefix text
)
RETURNS TABLE (
  id uuid,
  name text,
  key_prefix text,
  created_at timestamptz,
  last_used_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT private.cloud_api_user_has_pro(p_actor_user_id) THEN
    RAISE EXCEPTION 'hyprnote pro entitlement required'
      USING ERRCODE = '42501';
  END IF;

  IF NOT EXISTS (
    SELECT 1
    FROM public.api_cloud_settings AS settings
    WHERE settings.user_id = p_actor_user_id
      AND settings.enabled
  ) THEN
    RAISE EXCEPTION 'cloud API not enabled'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  INSERT INTO public.api_keys (id, user_id, name, key_hash, key_prefix)
  VALUES (
    p_id,
    p_actor_user_id,
    btrim(p_name),
    p_key_hash,
    p_key_prefix
  )
  RETURNING
    api_keys.id,
    api_keys.name,
    api_keys.key_prefix,
    api_keys.created_at,
    api_keys.last_used_at;
END;
$$;

CREATE OR REPLACE FUNCTION public.list_cloud_api_keys(p_actor_user_id uuid)
RETURNS TABLE (
  id uuid,
  name text,
  key_prefix text,
  created_at timestamptz,
  last_used_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    key.id,
    key.name,
    key.key_prefix,
    key.created_at,
    key.last_used_at
  FROM public.api_keys AS key
  WHERE key.user_id = p_actor_user_id
    AND key.revoked_at IS NULL
  ORDER BY key.created_at DESC, key.id DESC;
$$;

CREATE OR REPLACE FUNCTION public.revoke_cloud_api_key(
  p_actor_user_id uuid,
  p_key_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  UPDATE public.api_keys
  SET revoked_at = now()
  WHERE id = p_key_id
    AND user_id = p_actor_user_id
    AND revoked_at IS NULL;
  RETURN FOUND;
END;
$$;

CREATE OR REPLACE FUNCTION public.verify_cloud_api_key(p_key_hash text)
RETURNS TABLE (user_id uuid, status text)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_key public.api_keys%ROWTYPE;
  v_enabled boolean;
  v_has_pro boolean;
BEGIN
  SELECT * INTO v_key
  FROM public.api_keys AS key
  WHERE key.key_hash = p_key_hash
    AND key.revoked_at IS NULL;

  IF NOT FOUND THEN
    RETURN;
  END IF;

  SELECT COALESCE(settings.enabled, false) INTO v_enabled
  FROM (SELECT 1) AS singleton
  LEFT JOIN public.api_cloud_settings AS settings
    ON settings.user_id = v_key.user_id;
  v_has_pro := private.cloud_api_user_has_pro(v_key.user_id);

  IF NOT v_has_pro THEN
    RETURN QUERY SELECT v_key.user_id, 'subscription_required'::text;
    RETURN;
  END IF;

  IF NOT v_enabled THEN
    RETURN QUERY SELECT v_key.user_id, 'cloud_api_not_enabled'::text;
    RETURN;
  END IF;

  UPDATE public.api_keys
  SET last_used_at = now()
  WHERE id = v_key.id;

  RETURN QUERY SELECT v_key.user_id, 'ok'::text;
END;
$$;

REVOKE ALL ON FUNCTION private.cloud_api_user_has_pro(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.get_cloud_api_settings(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.set_cloud_api_enabled(uuid, boolean)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.publish_cloud_api_snapshot(uuid, text, jsonb)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.delete_cloud_api_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_cloud_api_snapshots(uuid, text, text, integer, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_cloud_api_snapshot(uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.create_cloud_api_key(uuid, uuid, text, text, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_cloud_api_keys(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.revoke_cloud_api_key(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.verify_cloud_api_key(text)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION
  public.get_cloud_api_settings(uuid),
  public.set_cloud_api_enabled(uuid, boolean),
  public.publish_cloud_api_snapshot(uuid, text, jsonb),
  public.delete_cloud_api_snapshot(uuid, text),
  public.list_cloud_api_snapshots(uuid, text, text, integer, integer),
  public.read_cloud_api_snapshot(uuid, text),
  public.create_cloud_api_key(uuid, uuid, text, text, text),
  public.list_cloud_api_keys(uuid),
  public.revoke_cloud_api_key(uuid, uuid),
  public.verify_cloud_api_key(text)
TO service_role;
