BEGIN;

ALTER TABLE private.account_deletion_jobs
  ADD COLUMN e2ee_workspace_ids uuid[],
  ADD COLUMN e2ee_purged_at timestamptz,
  ADD COLUMN stripe_customer_id text,
  ADD COLUMN stripe_deleted_at timestamptz;

DO $$
DECLARE
  v_now timestamptz := clock_timestamp();
BEGIN
  LOCK TABLE auth.users, public.profiles, public.workspaces, public.workspace_memberships
    IN SHARE ROW EXCLUSIVE MODE;

  IF EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE NOT EXISTS (
      SELECT 1
      FROM auth.users AS account
      WHERE account.id = deletion.owner_user_id
    ) OR NOT EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = deletion.owner_user_id
        AND workspace.owner_user_id = deletion.owner_user_id
        AND workspace.kind = 'personal'
    )
  ) THEN
    RAISE EXCEPTION 'cannot reconstruct E2EE scope for an incomplete legacy account deletion'
      USING ERRCODE = '55000';
  END IF;

  IF EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    JOIN public.profiles AS profile
      ON profile.id = deletion.owner_user_id
    WHERE profile.stripe_customer_id IS NOT NULL
      AND (
        length(profile.stripe_customer_id) > 255
        OR profile.stripe_customer_id !~ '^cus_[A-Za-z0-9]+$'
      )
  ) THEN
    RAISE EXCEPTION 'cannot snapshot an invalid Stripe customer for an incomplete account deletion'
      USING ERRCODE = '55000';
  END IF;

  UPDATE private.account_deletion_jobs AS deletion
  SET
    e2ee_workspace_ids = (
      SELECT array_agg(scope.workspace_id ORDER BY scope.workspace_id)
      FROM (
        SELECT deletion.owner_user_id AS workspace_id
        UNION
        SELECT workspace.id
        FROM public.workspaces AS workspace
        WHERE workspace.owner_user_id = deletion.owner_user_id
      ) AS scope
    ),
    stripe_customer_id = (
      SELECT profile.stripe_customer_id
      FROM public.profiles AS profile
      WHERE profile.id = deletion.owner_user_id
    );

  UPDATE public.workspaces AS workspace
  SET
    deleted_at = COALESCE(workspace.deleted_at, v_now),
    updated_at = GREATEST(workspace.updated_at, v_now)
  FROM private.account_deletion_jobs AS deletion
  WHERE workspace.owner_user_id = deletion.owner_user_id;

  UPDATE public.workspace_memberships AS membership
  SET
    deleted_at = COALESCE(membership.deleted_at, v_now),
    updated_at = GREATEST(membership.updated_at, v_now)
  WHERE EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = membership.user_id
  ) OR EXISTS (
    SELECT 1
    FROM public.workspaces AS workspace
    JOIN private.account_deletion_jobs AS deletion
      ON deletion.owner_user_id = workspace.owner_user_id
    WHERE workspace.id = membership.workspace_id
  );

  UPDATE private.account_deletion_jobs AS deletion
  SET
    final_sweep_not_before = GREATEST(
      deletion.final_sweep_not_before,
      v_now + interval '24 hours 5 minutes'
    ),
    prefix_swept_at = NULL,
    lease_id = NULL,
    lease_expires_at = NULL,
    updated_at = GREATEST(deletion.updated_at, v_now);
END;
$$;

ALTER TABLE private.account_deletion_jobs
  ALTER COLUMN e2ee_workspace_ids SET NOT NULL,
  ADD CONSTRAINT account_deletion_jobs_e2ee_workspace_ids_check CHECK (
    cardinality(e2ee_workspace_ids) BETWEEN 1 AND 1000
    AND array_position(e2ee_workspace_ids, NULL) IS NULL
    AND owner_user_id = ANY(e2ee_workspace_ids)
  ),
  ADD CONSTRAINT account_deletion_jobs_e2ee_purged_check CHECK (
    e2ee_purged_at IS NULL OR e2ee_purged_at >= requested_at
  ),
  ADD CONSTRAINT account_deletion_jobs_stripe_customer_id_check CHECK (
    stripe_customer_id IS NULL
    OR (
      length(stripe_customer_id) <= 255
      AND stripe_customer_id ~ '^cus_[A-Za-z0-9]+$'
    )
  ),
  ADD CONSTRAINT account_deletion_jobs_stripe_deleted_check CHECK (
    stripe_deleted_at IS NULL OR stripe_deleted_at >= requested_at
  );

CREATE OR REPLACE FUNCTION private.capture_account_deletion_e2ee_scope()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF TG_OP = 'UPDATE' THEN
    IF NEW.owner_user_id IS DISTINCT FROM OLD.owner_user_id
      OR NEW.e2ee_workspace_ids IS DISTINCT FROM OLD.e2ee_workspace_ids
      OR (
        NEW.stripe_customer_id IS DISTINCT FROM OLD.stripe_customer_id
        AND NOT (
          OLD.stripe_customer_id IS NULL
          AND NEW.stripe_customer_id IS NOT NULL
          AND NEW.stripe_deleted_at IS NULL
        )
      )
    THEN
      RAISE EXCEPTION 'account deletion cleanup scope is immutable'
        USING ERRCODE = '55000';
    END IF;
    RETURN NEW;
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(NEW.owner_user_id::text, 170001)
  );

  SELECT array_agg(scope.workspace_id ORDER BY scope.workspace_id)
  INTO NEW.e2ee_workspace_ids
  FROM (
    SELECT NEW.owner_user_id AS workspace_id
    UNION
    SELECT workspace.id
    FROM public.workspaces AS workspace
    WHERE workspace.owner_user_id = NEW.owner_user_id
  ) AS scope;

  SELECT profile.stripe_customer_id
  INTO NEW.stripe_customer_id
  FROM public.profiles AS profile
  WHERE profile.id = NEW.owner_user_id;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.capture_account_deletion_e2ee_scope()
  FROM PUBLIC, anon, authenticated, service_role;

CREATE TRIGGER capture_account_deletion_e2ee_scope
  BEFORE INSERT OR UPDATE OF owner_user_id, e2ee_workspace_ids, stripe_customer_id
  ON private.account_deletion_jobs
  FOR EACH ROW EXECUTE FUNCTION private.capture_account_deletion_e2ee_scope();

CREATE OR REPLACE FUNCTION private.prevent_stripe_customer_for_deleting_profile()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_profile_id uuid;
BEGIN
  IF TG_OP = 'UPDATE' THEN
    FOR v_profile_id IN
      SELECT candidate.profile_id
      FROM (
        VALUES (OLD.id), (NEW.id)
      ) AS candidate(profile_id)
      GROUP BY candidate.profile_id
      ORDER BY candidate.profile_id
    LOOP
      PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(v_profile_id::text, 170001)
      );
    END LOOP;
  ELSE
    PERFORM pg_catalog.pg_advisory_xact_lock(
      pg_catalog.hashtextextended(NEW.id::text, 170001)
    );
  END IF;

  IF EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = NEW.id
      OR (
        TG_OP = 'UPDATE'
        AND deletion.owner_user_id = OLD.id
      )
  ) AND (
    TG_OP = 'INSERT'
    OR NEW.id IS DISTINCT FROM OLD.id
    OR NEW.stripe_customer_id IS DISTINCT FROM OLD.stripe_customer_id
  ) THEN
    RAISE EXCEPTION 'Stripe customer assignment is unavailable during account deletion'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.prevent_stripe_customer_for_deleting_profile()
  FROM PUBLIC, anon, authenticated, service_role;

CREATE TRIGGER prevent_stripe_customer_for_deleting_profile
  BEFORE INSERT OR UPDATE OF id, stripe_customer_id ON public.profiles
  FOR EACH ROW EXECUTE FUNCTION private.prevent_stripe_customer_for_deleting_profile();

REVOKE INSERT, UPDATE, DELETE ON TABLE public.profiles FROM authenticated;

CREATE OR REPLACE FUNCTION public.assign_profile_stripe_customer(
  p_owner_user_id uuid,
  p_stripe_customer_id text
)
RETURNS TABLE (assigned_customer_id text)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_customer_id text;
  v_deletion private.account_deletion_jobs%ROWTYPE;
BEGIN
  IF p_owner_user_id IS NULL
    OR p_stripe_customer_id IS NULL
    OR length(p_stripe_customer_id) > 255
    OR p_stripe_customer_id !~ '^cus_[A-Za-z0-9]+$'
  THEN
    RAISE EXCEPTION 'invalid Stripe customer assignment'
      USING ERRCODE = '22023';
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_owner_user_id::text, 170001)
  );

  SELECT deletion.*
  INTO v_deletion
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  IF FOUND THEN
    IF v_deletion.stripe_customer_id IS NULL
      OR v_deletion.stripe_customer_id = p_stripe_customer_id
    THEN
      UPDATE private.account_deletion_jobs AS deletion
      SET
        stripe_customer_id = p_stripe_customer_id,
        stripe_deleted_at = NULL,
        lease_id = NULL,
        lease_expires_at = NULL,
        updated_at = clock_timestamp()
      WHERE deletion.owner_user_id = p_owner_user_id;
    END IF;

    assigned_customer_id := NULL;
    RETURN NEXT;
    RETURN;
  END IF;

  SELECT profile.stripe_customer_id
  INTO v_customer_id
  FROM public.profiles AS profile
  WHERE profile.id = p_owner_user_id
  FOR UPDATE;

  IF NOT FOUND THEN
    assigned_customer_id := NULL;
    RETURN NEXT;
    RETURN;
  END IF;

  IF v_customer_id IS NULL THEN
    UPDATE public.profiles AS profile
    SET stripe_customer_id = p_stripe_customer_id
    WHERE profile.id = p_owner_user_id;
    v_customer_id := p_stripe_customer_id;
  END IF;

  assigned_customer_id := v_customer_id;
  RETURN NEXT;
END;
$$;

REVOKE ALL ON FUNCTION public.assign_profile_stripe_customer(uuid, text)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.assign_profile_stripe_customer(uuid, text)
  TO service_role;

CREATE OR REPLACE FUNCTION private.prevent_workspace_for_deleting_owner()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_owner_user_id uuid;
BEGIN
  IF TG_OP = 'UPDATE' THEN
    FOR v_owner_user_id IN
      SELECT candidate.owner_user_id
      FROM (
        VALUES (OLD.owner_user_id), (NEW.owner_user_id)
      ) AS candidate(owner_user_id)
      GROUP BY candidate.owner_user_id
      ORDER BY candidate.owner_user_id
    LOOP
      PERFORM pg_catalog.pg_advisory_xact_lock(
        pg_catalog.hashtextextended(v_owner_user_id::text, 170001)
      );
    END LOOP;
  ELSE
    PERFORM pg_catalog.pg_advisory_xact_lock(
      pg_catalog.hashtextextended(NEW.owner_user_id::text, 170001)
    );
  END IF;

  IF EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = NEW.owner_user_id
      OR (
        TG_OP = 'UPDATE'
        AND deletion.owner_user_id = OLD.owner_user_id
      )
  ) THEN
    RAISE EXCEPTION 'workspace owner is pending deletion'
      USING ERRCODE = '55000';
  END IF;
  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.prevent_workspace_for_deleting_owner()
  FROM PUBLIC, anon, authenticated, service_role;

CREATE TRIGGER prevent_workspace_for_deleting_owner
  BEFORE INSERT OR UPDATE OF id, owner_user_id ON public.workspaces
  FOR EACH ROW EXECUTE FUNCTION private.prevent_workspace_for_deleting_owner();

CREATE OR REPLACE FUNCTION private.prepare_account_deletion_extension(
  p_owner_user_id uuid,
  p_now timestamptz
)
RETURNS timestamptz
LANGUAGE plpgsql
SET search_path = ''
AS $$
BEGIN
  RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION private.account_deletion_extension_ledgers_empty(
  p_owner_user_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
BEGIN
  RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION private.account_deletion_extension_prefix_empty(
  p_owner_user_id uuid
)
RETURNS boolean
LANGUAGE plpgsql
STABLE
SET search_path = ''
AS $$
BEGIN
  RETURN true;
END;
$$;

REVOKE ALL ON FUNCTION private.prepare_account_deletion_extension(uuid, timestamptz)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.account_deletion_extension_ledgers_empty(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.account_deletion_extension_prefix_empty(uuid)
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.prepare_account_deletion_extension(uuid, timestamptz)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.account_deletion_extension_ledgers_empty(uuid)
  TO service_role;
GRANT EXECUTE ON FUNCTION private.account_deletion_extension_prefix_empty(uuid)
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
  v_existing_horizon timestamptz;
  v_backup_horizon timestamptz;
  v_extension_horizon timestamptz;
  v_horizon timestamptz;
  v_was_created boolean;
BEGIN
  IF p_owner_user_id IS NULL THEN
    RAISE EXCEPTION 'invalid account deletion owner'
      USING ERRCODE = '22023';
  END IF;

  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(p_owner_user_id::text, 170001)
  );

  SELECT deletion.final_sweep_not_before
  INTO v_existing_horizon
  FROM private.account_deletion_jobs AS deletion
  WHERE deletion.owner_user_id = p_owner_user_id
  FOR UPDATE;

  PERFORM 1
  FROM public.workspaces AS workspace
  WHERE workspace.id = p_owner_user_id
    AND workspace.owner_user_id = p_owner_user_id
    AND workspace.kind = 'personal'
  FOR UPDATE;

  IF NOT FOUND THEN
    IF v_existing_horizon IS NOT NULL THEN
      owner_user_id := p_owner_user_id;
      final_sweep_not_before := v_existing_horizon;
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
  WHERE workspace.owner_user_id = p_owner_user_id;

  UPDATE public.workspace_memberships AS membership
  SET
    deleted_at = COALESCE(membership.deleted_at, v_now),
    updated_at = GREATEST(membership.updated_at, v_now)
  WHERE membership.user_id = p_owner_user_id
    OR EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.id = membership.workspace_id
        AND workspace.owner_user_id = p_owner_user_id
    );

  UPDATE public.attachment_backup_objects AS backup
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(backup.deletion_requested_at, v_now),
    updated_at = GREATEST(backup.updated_at, v_now)
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT max(backup.cleanup_not_before)
  INTO v_backup_horizon
  FROM public.attachment_backup_objects AS backup
  WHERE backup.owner_user_id = p_owner_user_id;

  SELECT private.prepare_account_deletion_extension(p_owner_user_id, v_now)
  INTO v_extension_horizon;

  v_horizon := GREATEST(
    COALESCE(
      v_existing_horizon,
      v_now + interval '24 hours 5 minutes'
    ),
    COALESCE(v_backup_horizon, v_existing_horizon, v_now),
    COALESCE(v_extension_horizon, v_existing_horizon, v_now)
  );

  SELECT NOT EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = p_owner_user_id
  ) INTO v_was_created;

  INSERT INTO private.account_deletion_jobs (
    owner_user_id,
    requested_at,
    final_sweep_not_before,
    updated_at
  ) VALUES (
    p_owner_user_id,
    LEAST(v_now, v_horizon),
    v_horizon,
    v_now
  )
  ON CONFLICT ON CONSTRAINT account_deletion_jobs_pkey DO UPDATE SET
    final_sweep_not_before = GREATEST(
      private.account_deletion_jobs.final_sweep_not_before,
      excluded.final_sweep_not_before
    ),
    prefix_swept_at = CASE
      WHEN excluded.final_sweep_not_before
        > private.account_deletion_jobs.final_sweep_not_before
        THEN NULL
      ELSE private.account_deletion_jobs.prefix_swept_at
    END,
    updated_at = GREATEST(
      private.account_deletion_jobs.updated_at,
      excluded.updated_at
    )
  RETURNING
    account_deletion_jobs.owner_user_id,
    account_deletion_jobs.final_sweep_not_before
  INTO owner_user_id, final_sweep_not_before;

  was_created := v_was_created;
  RETURN NEXT;
END;
$$;

CREATE OR REPLACE FUNCTION public.claim_account_deletion_leases_v2(
  p_lease_id uuid,
  p_limit integer DEFAULT 10,
  p_lease_seconds integer DEFAULT 900
)
RETURNS TABLE (
  owner_user_id uuid,
  final_sweep_not_before timestamptz,
  stripe_customer_id text,
  stripe_deleted boolean,
  cleanup_ready boolean,
  prefix_swept boolean,
  e2ee_workspace_ids uuid[],
  e2ee_purged boolean,
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
    OR p_limit NOT BETWEEN 1 AND 100
    OR p_lease_seconds IS NULL
    OR p_lease_seconds NOT BETWEEN 30 AND 3600
  THEN
    RAISE EXCEPTION 'invalid account deletion lease'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH candidates AS (
    SELECT deletion.owner_user_id
    FROM private.account_deletion_jobs AS deletion
    WHERE (
        deletion.lease_expires_at IS NULL
        OR deletion.lease_expires_at <= v_now
      )
      AND (
        deletion.stripe_deleted_at IS NULL
        OR (
          deletion.final_sweep_not_before <= v_now
          AND NOT EXISTS (
            SELECT 1
            FROM public.attachment_backup_objects AS backup
            WHERE backup.owner_user_id = deletion.owner_user_id
          )
          AND private.account_deletion_extension_ledgers_empty(
            deletion.owner_user_id
          )
        )
      )
    ORDER BY
      (deletion.stripe_deleted_at IS NOT NULL),
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
    leased.stripe_customer_id,
    leased.stripe_deleted_at IS NOT NULL,
    leased.final_sweep_not_before <= v_now
      AND NOT EXISTS (
        SELECT 1
        FROM public.attachment_backup_objects AS backup
        WHERE backup.owner_user_id = leased.owner_user_id
      )
      AND private.account_deletion_extension_ledgers_empty(
        leased.owner_user_id
      ),
    leased.prefix_swept_at IS NOT NULL,
    leased.e2ee_workspace_ids,
    leased.e2ee_purged_at IS NOT NULL,
    leased.lease_id,
    leased.lease_expires_at
  FROM leased
  ORDER BY
    (leased.stripe_deleted_at IS NOT NULL),
    leased.final_sweep_not_before,
    leased.requested_at,
    leased.owner_user_id;
END;
$$;

CREATE OR REPLACE FUNCTION public.mark_account_deletion_stripe_deleted(
  p_owner_user_id uuid,
  p_lease_id uuid,
  p_stripe_customer_id text
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
    OR p_stripe_customer_id IS DISTINCT FROM v_deletion.stripe_customer_id
  THEN
    RAISE EXCEPTION 'account deletion Stripe checkpoint is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE private.account_deletion_jobs AS deletion
  SET
    stripe_deleted_at = COALESCE(deletion.stripe_deleted_at, v_now),
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id;

  RETURN true;
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
    OR v_deletion.stripe_deleted_at IS NULL
    OR EXISTS (
      SELECT 1
      FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR NOT private.account_deletion_extension_ledgers_empty(p_owner_user_id)
    OR EXISTS (
      SELECT 1
      FROM storage.objects AS object
      WHERE object.bucket_id IN (
        'attachment-backups',
        'audio-files'
      )
        AND left(object.name, length(p_owner_user_id::text) + 1)
          = p_owner_user_id::text || '/'
    )
    OR NOT private.account_deletion_extension_prefix_empty(p_owner_user_id)
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

CREATE OR REPLACE FUNCTION public.mark_account_deletion_e2ee_purged(
  p_owner_user_id uuid,
  p_lease_id uuid,
  p_workspace_ids uuid[]
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
    OR v_deletion.stripe_deleted_at IS NULL
    OR v_deletion.prefix_swept_at IS NULL
    OR p_workspace_ids IS DISTINCT FROM v_deletion.e2ee_workspace_ids
    OR EXISTS (
      SELECT 1 FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR NOT private.account_deletion_extension_ledgers_empty(p_owner_user_id)
    OR EXISTS (
      SELECT 1
      FROM storage.objects AS object
      WHERE object.bucket_id IN (
        'attachment-backups',
        'audio-files'
      )
        AND left(object.name, length(p_owner_user_id::text) + 1)
          = p_owner_user_id::text || '/'
    )
    OR NOT private.account_deletion_extension_prefix_empty(p_owner_user_id)
  THEN
    RAISE EXCEPTION 'account deletion E2EE checkpoint is unavailable'
      USING ERRCODE = '55000';
  END IF;

  UPDATE private.account_deletion_jobs AS deletion
  SET
    e2ee_purged_at = COALESCE(deletion.e2ee_purged_at, v_now),
    updated_at = GREATEST(deletion.updated_at, v_now)
  WHERE deletion.owner_user_id = p_owner_user_id;

  RETURN true;
END;
$$;

CREATE OR REPLACE FUNCTION private.prevent_unpurged_account_auth_deletion()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM pg_catalog.pg_advisory_xact_lock(
    pg_catalog.hashtextextended(OLD.id::text, 170001)
  );

  IF (
    EXISTS (
      SELECT 1
      FROM public.workspaces AS workspace
      WHERE workspace.owner_user_id = OLD.id
    )
    OR EXISTS (
      SELECT 1
      FROM public.profiles AS profile
      WHERE profile.id = OLD.id
        AND profile.stripe_customer_id IS NOT NULL
    )
  ) AND NOT EXISTS (
    SELECT 1
    FROM private.account_deletion_jobs AS deletion
    WHERE deletion.owner_user_id = OLD.id
      AND deletion.stripe_deleted_at IS NOT NULL
      AND deletion.prefix_swept_at IS NOT NULL
      AND deletion.e2ee_purged_at IS NOT NULL
  ) THEN
    RAISE EXCEPTION 'account durable cleanup is incomplete'
      USING ERRCODE = '55000';
  END IF;
  RETURN OLD;
END;
$$;

REVOKE ALL ON FUNCTION private.prevent_unpurged_account_auth_deletion()
  FROM PUBLIC, anon, authenticated, service_role;

CREATE TRIGGER prevent_unpurged_account_auth_deletion
  BEFORE DELETE ON auth.users
  FOR EACH ROW EXECUTE FUNCTION private.prevent_unpurged_account_auth_deletion();

CREATE OR REPLACE FUNCTION public.can_start_trial()
RETURNS boolean
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_user_id uuid := auth.uid();
  v_customer_id text;
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM auth.users AS auth_user
    WHERE auth_user.id = v_user_id
      AND COALESCE(auth_user.is_anonymous, false) = false
      AND NOT EXISTS (
        SELECT 1
        FROM private.account_deletion_jobs AS deletion
        WHERE deletion.owner_user_id = auth_user.id
      )
  ) THEN
    RETURN false;
  END IF;

  SELECT profile.stripe_customer_id
  INTO v_customer_id
  FROM public.profiles AS profile
  WHERE profile.id = v_user_id;

  IF v_customer_id IS NULL THEN
    RETURN true;
  END IF;

  RETURN NOT EXISTS (
    SELECT 1
    FROM stripe.subscriptions AS subscription
    WHERE subscription.customer = v_customer_id
      AND (
        subscription.status IN ('active', 'trialing')
        OR (
          subscription.trial_start IS NOT NULL
          AND (subscription.trial_start #>> '{}')::bigint
            > extract(epoch from now() - interval '3 months')
        )
      )
  );
END;
$$;

REVOKE ALL ON FUNCTION public.can_start_trial() FROM PUBLIC, anon;
GRANT EXECUTE ON FUNCTION public.can_start_trial() TO authenticated;

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
    OR v_deletion.stripe_deleted_at IS NULL
    OR v_deletion.prefix_swept_at IS NULL
    OR v_deletion.e2ee_purged_at IS NULL
    OR EXISTS (
      SELECT 1 FROM public.attachment_backup_objects AS backup
      WHERE backup.owner_user_id = p_owner_user_id
    )
    OR NOT private.account_deletion_extension_ledgers_empty(p_owner_user_id)
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

REVOKE ALL ON FUNCTION public.claim_account_deletion_leases_v2(uuid, integer, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_account_deletion_stripe_deleted(uuid, uuid, text)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.mark_account_deletion_e2ee_purged(uuid, uuid, uuid[])
  FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION public.claim_account_deletion_leases_v2(uuid, integer, integer)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_account_deletion_stripe_deleted(uuid, uuid, text)
  TO service_role;
GRANT EXECUTE ON FUNCTION public.mark_account_deletion_e2ee_purged(uuid, uuid, uuid[])
  TO service_role;

COMMIT;
