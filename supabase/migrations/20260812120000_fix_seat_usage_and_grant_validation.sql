-- An invitation stopped holding a seat only when its invitee_user_id matched a
-- seated member, but that column is NULL for anyone invited before they had an
-- account, and accept_workspace_invitation only stamps it after inserting the
-- membership. The invitee was therefore counted twice mid-acceptance, so a team
-- that bought exactly N seats could never seat anyone new.
CREATE OR REPLACE FUNCTION private.workspace_seat_usage(
  p_workspace_id uuid
)
RETURNS TABLE (
  seat_limit integer,
  used_seats integer
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    workspace.seat_limit,
    (
      (
        SELECT count(*)
        FROM public.workspace_memberships AS membership
        WHERE membership.workspace_id = workspace.id
          AND membership.deleted_at IS NULL
      )
      + (
        SELECT count(*)
        FROM public.workspace_invitations AS invitation
        WHERE invitation.workspace_id = workspace.id
          AND invitation.accepted_at IS NULL
          AND invitation.revoked_at IS NULL
          AND invitation.expires_at > now()
          -- Match on email as well as id: the id is still unset while a
          -- brand-new account is being seated.
          AND NOT EXISTS (
            SELECT 1
            FROM public.workspace_memberships AS seated
            LEFT JOIN auth.users AS seated_user
              ON seated_user.id = seated.user_id
            WHERE seated.workspace_id = invitation.workspace_id
              AND seated.deleted_at IS NULL
              AND (
                seated.user_id = invitation.invitee_user_id
                OR lower(btrim(seated_user.email)) = invitation.invitee_email
              )
          )
      )
    )::integer
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_workspace_id;
$$;

REVOKE ALL ON FUNCTION private.workspace_seat_usage(uuid)
  FROM PUBLIC, anon, authenticated;

-- The issuer check cast userId to uuid without the guard the grant loop uses,
-- so a malformed id surfaced a raw 22P02 instead of the validation message.
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
  v_includes_issuer boolean;
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

  BEGIN
    SELECT EXISTS (
      SELECT 1
      FROM jsonb_array_elements(p_grants) AS candidate
      WHERE (candidate ->> 'userId')::uuid = v_actor_id
    )
    INTO v_includes_issuer;
  EXCEPTION WHEN invalid_text_representation THEN
    RAISE EXCEPTION 'E2EE key grants are invalid'
      USING ERRCODE = '22023';
  END;

  -- The issuer must keep their own access, otherwise a rotation could lock the
  -- workspace out of its own data.
  IF NOT v_includes_issuer THEN
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

REVOKE ALL ON FUNCTION private.set_workspace_e2ee_key(uuid, text, jsonb)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.set_workspace_e2ee_key(uuid, text, jsonb)
  TO authenticated;
