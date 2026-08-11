-- Shared workspaces cannot derive their key from one member's recovery key, so
-- the key is minted at random and wrapped once per member against an
-- account-level identity key. The server stores only wrapped ciphertext.

CREATE TABLE public.e2ee_member_identities (
  user_id uuid PRIMARY KEY REFERENCES auth.users(id) ON DELETE CASCADE,
  public_key text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT e2ee_member_identities_public_key_check CHECK (
    public_key ~ '^[A-Za-z0-9_-]{43}$'
  )
);

CREATE TABLE public.workspace_e2ee_keys (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  key_id text NOT NULL,
  created_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  retired_at timestamptz,
  CONSTRAINT workspace_e2ee_keys_key_id_check CHECK (
    key_id ~ '^[A-Za-z0-9_-]{22}$'
  ),
  CONSTRAINT workspace_e2ee_keys_workspace_key_key UNIQUE (workspace_id, key_id)
);

CREATE UNIQUE INDEX workspace_e2ee_keys_active_key
  ON public.workspace_e2ee_keys(workspace_id)
  WHERE retired_at IS NULL;

CREATE TABLE public.workspace_e2ee_key_grants (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  key_id text NOT NULL,
  member_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  ephemeral_public_key text NOT NULL,
  nonce text NOT NULL,
  ciphertext text NOT NULL,
  issued_by_user_id uuid REFERENCES auth.users(id) ON DELETE SET NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT workspace_e2ee_key_grants_shape_check CHECK (
    key_id ~ '^[A-Za-z0-9_-]{22}$'
    AND ephemeral_public_key ~ '^[A-Za-z0-9_-]{43}$'
    AND nonce ~ '^[A-Za-z0-9_-]{32}$'
    AND ciphertext ~ '^[A-Za-z0-9_-]{64}$'
  ),
  CONSTRAINT workspace_e2ee_key_grants_member_key_key UNIQUE (
    workspace_id, key_id, member_user_id
  ),
  CONSTRAINT workspace_e2ee_key_grants_key_fkey FOREIGN KEY (workspace_id, key_id)
    REFERENCES public.workspace_e2ee_keys(workspace_id, key_id) ON DELETE CASCADE
);

CREATE INDEX workspace_e2ee_key_grants_member_idx
  ON public.workspace_e2ee_key_grants(member_user_id, workspace_id);

ALTER TABLE public.e2ee_member_identities ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.workspace_e2ee_keys ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.workspace_e2ee_key_grants ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.e2ee_member_identities FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.workspace_e2ee_keys FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.workspace_e2ee_key_grants FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.e2ee_member_identities TO service_role;
GRANT ALL ON TABLE public.workspace_e2ee_keys TO service_role;
GRANT ALL ON TABLE public.workspace_e2ee_key_grants TO service_role;

CREATE POLICY e2ee_member_identities_service_all
  ON public.e2ee_member_identities
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY workspace_e2ee_keys_service_all
  ON public.workspace_e2ee_keys
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY workspace_e2ee_key_grants_service_all
  ON public.workspace_e2ee_key_grants
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

-- Losing membership must also lose the wrapped key: without this a removed
-- member could re-fetch their grant and keep unwrapping the generation they
-- were cut off from.
CREATE OR REPLACE FUNCTION private.purge_workspace_e2ee_grants_for_membership()
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

  DELETE FROM public.workspace_e2ee_key_grants AS grant_row
  WHERE grant_row.workspace_id = OLD.workspace_id
    AND grant_row.member_user_id = OLD.user_id;

  IF TG_OP = 'DELETE' THEN
    RETURN OLD;
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.purge_workspace_e2ee_grants_for_membership()
  FROM PUBLIC, anon, authenticated;

CREATE TRIGGER on_workspace_membership_e2ee_grants_revoked
  AFTER DELETE OR UPDATE OF deleted_at ON public.workspace_memberships
  FOR EACH ROW EXECUTE FUNCTION private.purge_workspace_e2ee_grants_for_membership();

CREATE OR REPLACE FUNCTION private.publish_e2ee_member_identity(
  p_public_key text
)
RETURNS TABLE (public_key text)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS actor
    WHERE actor.id = v_actor_id
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'E2EE identity operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF p_public_key IS NULL OR p_public_key !~ '^[A-Za-z0-9_-]{43}$' THEN
    RAISE EXCEPTION 'E2EE member identity is invalid'
      USING ERRCODE = '22023';
  END IF;

  INSERT INTO public.e2ee_member_identities AS identity (user_id, public_key)
  VALUES (v_actor_id, p_public_key)
  ON CONFLICT (user_id) DO UPDATE SET
    public_key = excluded.public_key,
    updated_at = now()
  WHERE identity.public_key <> excluded.public_key;

  RETURN QUERY
  SELECT identity.public_key
  FROM public.e2ee_member_identities AS identity
  WHERE identity.user_id = v_actor_id;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_workspace_key_recipients(
  p_workspace_id uuid
)
RETURNS TABLE (
  user_id uuid,
  user_email text,
  role text,
  public_key text,
  granted_key_ids text[]
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor
      ON actor.id = membership.user_id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = auth.uid()
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'E2EE key operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    membership.user_id,
    lower(btrim(member_user.email)),
    membership.role,
    identity.public_key,
    COALESCE(
      (
        SELECT array_agg(grant_row.key_id ORDER BY grant_row.key_id)
        FROM public.workspace_e2ee_key_grants AS grant_row
        WHERE grant_row.workspace_id = p_workspace_id
          AND grant_row.member_user_id = membership.user_id
      ),
      ARRAY[]::text[]
    )
  FROM public.workspace_memberships AS membership
  JOIN auth.users AS member_user
    ON member_user.id = membership.user_id
  LEFT JOIN public.e2ee_member_identities AS identity
    ON identity.user_id = membership.user_id
  WHERE membership.workspace_id = p_workspace_id
    AND membership.deleted_at IS NULL
  ORDER BY membership.created_at, membership.id;
END;
$$;

-- Serves both the first key and every rotation: the caller mints a key, wraps
-- it for each recipient, and hands the wrapped blobs over in one call.
-- The OUT names are prefixed because bare `key_id` would shadow the real column
-- in the ON CONFLICT targets below.
CREATE OR REPLACE FUNCTION private.set_workspace_e2ee_key(
  p_workspace_id uuid,
  p_key_id text,
  p_grants jsonb
)
RETURNS TABLE (
  out_key_id text,
  out_granted_member_count integer
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_actor_id uuid := auth.uid();
  v_grant jsonb;
  v_member_id uuid;
  v_granted integer := 0;
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    JOIN auth.users AS actor
      ON actor.id = membership.user_id
    WHERE workspace.id = p_workspace_id
      AND workspace.kind = 'shared'
      AND workspace.deleted_at IS NULL
      AND membership.user_id = v_actor_id
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
      AND actor.email_confirmed_at IS NOT NULL
      AND COALESCE(actor.is_anonymous, false) = false
  ) THEN
    RAISE EXCEPTION 'E2EE key operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  IF p_key_id IS NULL OR p_key_id !~ '^[A-Za-z0-9_-]{22}$' THEN
    RAISE EXCEPTION 'E2EE key identity is invalid'
      USING ERRCODE = '22023';
  END IF;

  IF p_grants IS NULL OR jsonb_typeof(p_grants) <> 'array' THEN
    RAISE EXCEPTION 'E2EE key grants are invalid'
      USING ERRCODE = '22023';
  END IF;

  -- The issuer must keep their own access, otherwise a rotation could lock the
  -- workspace out of its own data.
  IF NOT EXISTS (
    SELECT 1
    FROM jsonb_array_elements(p_grants) AS candidate
    WHERE (candidate ->> 'userId')::uuid = v_actor_id
  ) THEN
    RAISE EXCEPTION 'E2EE key grants must include the issuing member'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id
  FOR UPDATE;

  UPDATE public.workspace_e2ee_keys
  SET retired_at = now()
  WHERE workspace_e2ee_keys.workspace_id = p_workspace_id
    AND workspace_e2ee_keys.key_id <> p_key_id
    AND workspace_e2ee_keys.retired_at IS NULL;

  INSERT INTO public.workspace_e2ee_keys (workspace_id, key_id, created_by_user_id)
  VALUES (p_workspace_id, p_key_id, v_actor_id)
  ON CONFLICT (workspace_id, key_id) DO UPDATE SET retired_at = NULL;

  FOR v_grant IN SELECT * FROM jsonb_array_elements(p_grants)
  LOOP
    BEGIN
      v_member_id := (v_grant ->> 'userId')::uuid;
    EXCEPTION WHEN invalid_text_representation THEN
      RAISE EXCEPTION 'E2EE key grants are invalid'
        USING ERRCODE = '22023';
    END;

    IF NOT EXISTS (
      SELECT 1
      FROM public.workspace_memberships AS membership
      WHERE membership.workspace_id = p_workspace_id
        AND membership.user_id = v_member_id
        AND membership.deleted_at IS NULL
    ) THEN
      RAISE EXCEPTION 'E2EE key grants must target active members'
        USING ERRCODE = '22023';
    END IF;

    INSERT INTO public.workspace_e2ee_key_grants (
      workspace_id,
      key_id,
      member_user_id,
      ephemeral_public_key,
      nonce,
      ciphertext,
      issued_by_user_id
    ) VALUES (
      p_workspace_id,
      p_key_id,
      v_member_id,
      v_grant ->> 'ephemeralPublicKey',
      v_grant ->> 'nonce',
      v_grant ->> 'ciphertext',
      v_actor_id
    )
    ON CONFLICT (workspace_id, key_id, member_user_id) DO NOTHING;
  END LOOP;

  SELECT count(*)
  INTO v_granted
  FROM public.workspace_e2ee_key_grants AS grant_row
  WHERE grant_row.workspace_id = p_workspace_id
    AND grant_row.key_id = p_key_id;

  RETURN QUERY
  SELECT p_key_id, v_granted;
END;
$$;

CREATE OR REPLACE FUNCTION private.list_my_workspace_e2ee_grants(
  p_workspace_id uuid
)
RETURNS TABLE (
  key_id text,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  is_active boolean,
  created_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN public.workspace_memberships AS membership
      ON membership.workspace_id = workspace.id
    WHERE workspace.id = p_workspace_id
      AND workspace.deleted_at IS NULL
      AND membership.user_id = auth.uid()
      AND membership.deleted_at IS NULL
  ) THEN
    RAISE EXCEPTION 'E2EE key operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT
    grant_row.key_id,
    grant_row.ephemeral_public_key,
    grant_row.nonce,
    grant_row.ciphertext,
    workspace_key.retired_at IS NULL,
    grant_row.created_at
  FROM public.workspace_e2ee_key_grants AS grant_row
  JOIN public.workspace_e2ee_keys AS workspace_key
    ON workspace_key.workspace_id = grant_row.workspace_id
    AND workspace_key.key_id = grant_row.key_id
  WHERE grant_row.workspace_id = p_workspace_id
    AND grant_row.member_user_id = auth.uid()
  ORDER BY workspace_key.created_at, grant_row.created_at;
END;
$$;

REVOKE ALL ON FUNCTION private.publish_e2ee_member_identity(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_workspace_key_recipients(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.set_workspace_e2ee_key(uuid, text, jsonb)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_my_workspace_e2ee_grants(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.publish_e2ee_member_identity(text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_workspace_key_recipients(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.set_workspace_e2ee_key(uuid, text, jsonb)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_workspace_e2ee_grants(uuid)
  TO authenticated;

CREATE OR REPLACE FUNCTION public.publish_e2ee_member_identity(
  p_public_key text
)
RETURNS TABLE (public_key text)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT * FROM private.publish_e2ee_member_identity(p_public_key);
$$;

CREATE OR REPLACE FUNCTION public.list_workspace_key_recipients(
  p_workspace_id uuid
)
RETURNS TABLE (
  user_id uuid,
  user_email text,
  role text,
  public_key text,
  granted_key_ids text[]
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT * FROM private.list_workspace_key_recipients(p_workspace_id);
$$;

CREATE OR REPLACE FUNCTION public.set_workspace_e2ee_key(
  p_workspace_id uuid,
  p_key_id text,
  p_grants jsonb
)
RETURNS TABLE (
  key_id text,
  granted_member_count integer
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT * FROM private.set_workspace_e2ee_key(p_workspace_id, p_key_id, p_grants);
$$;

CREATE OR REPLACE FUNCTION public.list_my_workspace_e2ee_grants(
  p_workspace_id uuid
)
RETURNS TABLE (
  key_id text,
  ephemeral_public_key text,
  nonce text,
  ciphertext text,
  is_active boolean,
  created_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT * FROM private.list_my_workspace_e2ee_grants(p_workspace_id);
$$;

REVOKE ALL ON FUNCTION public.publish_e2ee_member_identity(text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_workspace_key_recipients(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.set_workspace_e2ee_key(uuid, text, jsonb)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_workspace_e2ee_grants(uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.publish_e2ee_member_identity(text)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_workspace_key_recipients(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.set_workspace_e2ee_key(uuid, text, jsonb)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_workspace_e2ee_grants(uuid)
  TO authenticated;
