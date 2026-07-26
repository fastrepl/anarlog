CREATE TABLE private.account_deletion_jobs (
  owner_user_id uuid PRIMARY KEY,
  requested_at timestamptz NOT NULL DEFAULT now(),
  final_sweep_not_before timestamptz NOT NULL DEFAULT now(),
  prefix_swept_at timestamptz,
  lease_id uuid,
  lease_expires_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT account_deletion_jobs_lease_check CHECK (
    (lease_id IS NULL AND lease_expires_at IS NULL)
    OR (lease_id IS NOT NULL AND lease_expires_at IS NOT NULL)
  ),
  CONSTRAINT account_deletion_jobs_sweep_check CHECK (
    prefix_swept_at IS NULL
    OR prefix_swept_at >= final_sweep_not_before
  ),
  CONSTRAINT account_deletion_jobs_time_check CHECK (
    final_sweep_not_before >= requested_at
    AND updated_at >= requested_at
  )
);

CREATE INDEX account_deletion_jobs_claim_idx
  ON private.account_deletion_jobs(
    final_sweep_not_before,
    lease_expires_at,
    requested_at,
    owner_user_id
  );

ALTER TABLE private.account_deletion_jobs ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE private.account_deletion_jobs
  FROM PUBLIC, anon, authenticated;
GRANT SELECT, INSERT, UPDATE, DELETE
  ON TABLE private.account_deletion_jobs TO service_role;

CREATE POLICY account_deletion_jobs_service_all
  ON private.account_deletion_jobs
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.require_permanent_user()
RETURNS uuid
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid;
BEGIN
  SELECT auth_user.id
  INTO v_user_id
  FROM auth.users AS auth_user
  WHERE auth_user.id = auth.uid()
    AND auth_user.email_confirmed_at IS NOT NULL
    AND COALESCE(auth_user.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = auth_user.id
    );

  IF v_user_id IS NULL THEN
    RAISE EXCEPTION 'session access operation not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN v_user_id;
END;
$$;

REVOKE ALL ON FUNCTION private.require_permanent_user()
  FROM PUBLIC, anon, authenticated;

CREATE OR REPLACE FUNCTION private.require_attachment_backup_owner(
  p_owner_user_id uuid
)
RETURNS void
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal'
    AND workspace.e2ee_key_id IS NOT NULL
    AND workspace.deleted_at IS NULL
    AND NOT EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = p_owner_user_id
    )
  FOR UPDATE;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'active personal E2EE workspace required'
      USING ERRCODE = '42501';
  END IF;
END;
$$;

REVOKE ALL ON FUNCTION private.require_attachment_backup_owner(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.require_attachment_backup_owner(uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION private.account_deletion_auth_user_exists(
  p_owner_user_id uuid
)
RETURNS boolean
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT EXISTS (
    SELECT 1
    FROM auth.users AS account
    WHERE account.id = p_owner_user_id
  )
$$;

REVOKE ALL ON FUNCTION private.account_deletion_auth_user_exists(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.account_deletion_auth_user_exists(uuid)
  TO service_role;

CREATE OR REPLACE FUNCTION public.begin_account_deletion(
  p_owner_user_id uuid
)
RETURNS TABLE (
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  was_created boolean
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_backup_horizon timestamptz;
  v_horizon timestamptz;
  v_was_created boolean;
BEGIN
  IF p_owner_user_id IS NULL THEN
    RAISE EXCEPTION 'invalid account deletion owner'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal'
  FOR UPDATE;

  IF NOT FOUND THEN
    SELECT
      deletion.owner_user_id,
      deletion.final_sweep_not_before
    INTO owner_user_id, final_sweep_not_before
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
    FOR UPDATE;

    IF FOUND THEN
      was_created := false;
      RETURN NEXT;
      RETURN;
    END IF;

    RAISE EXCEPTION 'account deletion owner is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE public.workspaces AS workspace
  SET
    deleted_at = COALESCE(workspace.deleted_at, v_now),
    updated_at = GREATEST(workspace.updated_at, v_now)
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal';

  UPDATE public.workspace_memberships AS membership
  SET
    deleted_at = COALESCE(membership.deleted_at, v_now),
    updated_at = GREATEST(membership.updated_at, v_now)
  WHERE membership.user_id = p_owner_user_id;

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT MAX(backup.cleanup_not_before)
  INTO v_backup_horizon
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id;
  v_horizon := GREATEST(
    v_now + interval '24 hours 5 minutes',
    COALESCE(v_backup_horizon, v_now)
  );

  SELECT NOT EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
  )
  INTO v_was_created;

  INSERT INTO private.account_deletion_jobs (
    owner_user_id,
    requested_at,
    final_sweep_not_before,
    updated_at
  )
  VALUES (
    p_owner_user_id,
    v_now,
    v_horizon,
    v_now
  )
  ON CONFLICT ON CONSTRAINT account_deletion_jobs_pkey DO UPDATE SET
    updated_at = GREATEST(
      private.account_deletion_jobs.updated_at,
      EXCLUDED.updated_at
    )
  RETURNING
    account_deletion_jobs.owner_user_id,
    account_deletion_jobs.final_sweep_not_before
  INTO owner_user_id, final_sweep_not_before;

  was_created := v_was_created;
  RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_account_deletion_leases(
  p_lease_id uuid,
  p_limit integer DEFAULT 10,
  p_lease_seconds integer DEFAULT 900
)
RETURNS TABLE (
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  prefix_swept boolean,
  lease_id uuid,
  lease_expires_at timestamptz
)
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  IF p_lease_id IS NULL
    OR p_limit IS NULL
    OR p_limit < 1
    OR p_limit > 100
    OR p_lease_seconds IS NULL
    OR p_lease_seconds < 30
    OR p_lease_seconds > 3600
  THEN
    RAISE EXCEPTION 'invalid account deletion lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT deletion.owner_user_id
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.final_sweep_not_before <= v_now
      AND (
        deletion.lease_expires_at IS NULL
        OR deletion.lease_expires_at <= v_now
      )
      AND NOT EXISTS (
        SELECT 1
        FROM public.attachment_backup_objects AS backup
        WHERE backup.owner_user_id = deletion.owner_user_id
      )
    ORDER BY
      deletion.final_sweep_not_before,
      deletion.requested_at,
      deletion.owner_user_id
    FOR UPDATE SKIP LOCKED
    LIMIT p_limit
  ), leased AS (
    UPDATE private.account_deletion_jobs AS deletion
    SET
      lease_id = p_lease_id,
      lease_expires_at = v_now + make_interval(secs => p_lease_seconds),
      updated_at = v_now
    FROM candidates
    WHERE deletion.owner_user_id = candidates.owner_user_id
    RETURNING deletion.*
  )
  SELECT
    leased.owner_user_id,
    leased.final_sweep_not_before,
    leased.prefix_swept_at IS NOT NULL,
    leased.lease_id,
    leased.lease_expires_at
  FROM leased
  ORDER BY
    leased.final_sweep_not_before,
    leased.requested_at,
    leased.owner_user_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_account_deletion_prefix_swept(
  p_owner_user_id uuid,
  p_lease_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_deletion private.account_deletion_jobs%ROWTYPE;
BEGIN
  SELECT deletion.*
  INTO v_deletion
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF p_lease_id IS NULL
    OR v_deletion.lease_id IS DISTINCT FROM p_lease_id
    OR v_deletion.lease_expires_at IS NULL
    OR v_deletion.lease_expires_at <= v_now
    OR v_deletion.final_sweep_not_before > v_now
    OR EXISTS (
      SELECT 1
      FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR EXISTS (
      SELECT 1
      FROM storage.objects AS object
      WHERE object.bucket_id IN ('attachment-backups', 'audio-files')
        AND left(object.name, length(p_owner_user_id::text) + 1)
          = p_owner_user_id::text || '/'
    )
  THEN
    RAISE EXCEPTION 'account deletion sweep is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE private.account_deletion_jobs AS deletion
  SET
    prefix_swept_at = COALESCE(deletion.prefix_swept_at, v_now),
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id;

  RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION public.finish_account_deletion(
  p_owner_user_id uuid,
  p_lease_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_deletion private.account_deletion_jobs%ROWTYPE;
BEGIN
  SELECT deletion.*
  INTO v_deletion
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    RETURN false;
  END IF;

  IF p_lease_id IS NULL
    OR v_deletion.lease_id IS DISTINCT FROM p_lease_id
    OR v_deletion.lease_expires_at IS NULL
    OR v_deletion.lease_expires_at <= v_now
    OR v_deletion.prefix_swept_at IS NULL
    OR EXISTS (
      SELECT 1
      FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR private.account_deletion_auth_user_exists(p_owner_user_id)
  THEN
    RAISE EXCEPTION 'account deletion completion is unavailable'
      USING ERRCODE = '55000';
  END IF;

  DELETE FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id;

  RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION public.begin_account_deletion(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.claim_account_deletion_leases(uuid, integer, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_account_deletion_prefix_swept(uuid, uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.finish_account_deletion(uuid, uuid)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.begin_account_deletion(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.claim_account_deletion_leases(uuid, integer, integer)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_account_deletion_prefix_swept(uuid, uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.finish_account_deletion(uuid, uuid)
  TO service_role;

DROP POLICY IF EXISTS audio_files_select_owner ON storage.objects;
CREATE POLICY audio_files_select_owner
  ON storage.objects
  FOR SELECT
  TO authenticated
  USING (
    bucket_id = 'audio-files'
    AND (SELECT auth.uid())::text = (storage.foldername(name))[1]
    AND EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = (SELECT auth.uid())
        AND workspace.owner_user_id = (SELECT auth.uid())
        AND workspace.kind = 'personal'
        AND workspace.deleted_at IS NULL
    )
  );

DROP POLICY IF EXISTS audio_files_insert_authenticated ON storage.objects;
CREATE POLICY audio_files_insert_authenticated
  ON storage.objects
  FOR INSERT
  TO authenticated
  WITH CHECK (
    bucket_id = 'audio-files'
    AND (SELECT auth.uid())::text = (storage.foldername(name))[1]
    AND EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = (SELECT auth.uid())
        AND workspace.owner_user_id = (SELECT auth.uid())
        AND workspace.kind = 'personal'
        AND workspace.deleted_at IS NULL
    )
  );

DROP POLICY IF EXISTS audio_files_update_owner ON storage.objects;
CREATE POLICY audio_files_update_owner
  ON storage.objects
  FOR UPDATE
  TO authenticated
  USING (
    bucket_id = 'audio-files'
    AND (SELECT auth.uid())::text = (storage.foldername(name))[1]
    AND EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = (SELECT auth.uid())
        AND workspace.owner_user_id = (SELECT auth.uid())
        AND workspace.kind = 'personal'
        AND workspace.deleted_at IS NULL
    )
  )
  WITH CHECK (
    bucket_id = 'audio-files'
    AND (SELECT auth.uid())::text = (storage.foldername(name))[1]
    AND EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = (SELECT auth.uid())
        AND workspace.owner_user_id = (SELECT auth.uid())
        AND workspace.kind = 'personal'
        AND workspace.deleted_at IS NULL
    )
  );

DROP POLICY IF EXISTS audio_files_delete_owner ON storage.objects;
CREATE POLICY audio_files_delete_owner
  ON storage.objects
  FOR DELETE
  TO authenticated
  USING (
    bucket_id = 'audio-files'
    AND (SELECT auth.uid())::text = (storage.foldername(name))[1]
    AND EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = (SELECT auth.uid())
        AND workspace.owner_user_id = (SELECT auth.uid())
        AND workspace.kind = 'personal'
        AND workspace.deleted_at IS NULL
    )
  );
