ALTER TABLE public.session_share_snapshots
  ADD COLUMN web_editable boolean NOT NULL DEFAULT false,
  ADD COLUMN last_mutation_id uuid,
  ADD COLUMN last_mutation_base_revision bigint,
  ADD COLUMN last_mutation_fingerprint bytea,
  ADD CONSTRAINT session_share_snapshots_last_mutation_check CHECK (
    (
      last_mutation_id IS NULL
      AND last_mutation_base_revision IS NULL
      AND last_mutation_fingerprint IS NULL
    )
    OR (
      last_mutation_id IS NOT NULL
      AND last_mutation_base_revision IS NOT NULL
      AND last_mutation_base_revision >= 0
      AND last_mutation_base_revision = content_revision - 1
      AND last_mutation_fingerprint IS NOT NULL
      AND octet_length(last_mutation_fingerprint) = 32
    )
  );

CREATE TABLE public.session_share_pending_web_edits (
  share_id uuid PRIMARY KEY
    REFERENCES public.session_shares(id) ON DELETE CASCADE,
  base_content_revision bigint NOT NULL,
  base_title text NOT NULL,
  base_body_json jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT session_share_pending_web_edits_revision_check CHECK (
    base_content_revision > 0
  ),
  CONSTRAINT session_share_pending_web_edits_title_check CHECK (
    base_title = btrim(base_title)
    AND octet_length(base_title) <= 4096
  ),
  CONSTRAINT session_share_pending_web_edits_body_check CHECK (
    jsonb_typeof(base_body_json) = 'object'
    AND base_body_json ->> 'type' = 'doc'
    AND octet_length(base_body_json::text) <= 2097152
  ),
  CONSTRAINT session_share_pending_web_edits_time_check CHECK (
    updated_at >= created_at
  )
);

ALTER TABLE public.session_share_pending_web_edits ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.session_share_pending_web_edits
  FROM PUBLIC, anon, authenticated;
GRANT ALL ON TABLE public.session_share_pending_web_edits TO service_role;

CREATE POLICY session_share_pending_web_edits_service_all
  ON public.session_share_pending_web_edits
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE OR REPLACE FUNCTION private.require_session_share_editor(
  p_share_id uuid,
  p_actor_user_id uuid
)
RETURNS TABLE (
  manage_access boolean,
  access_version bigint
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_share public.session_shares%ROWTYPE;
  v_manage_access boolean;
  v_has_editor_grant boolean;
BEGIN
  IF p_share_id IS NULL OR p_actor_user_id IS NULL THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  PERFORM 1
  FROM auth.users AS actor
  WHERE actor.id = p_actor_user_id
    AND actor.email_confirmed_at IS NOT NULL
    AND COALESCE(actor.is_anonymous, false) = false
    AND NOT EXISTS (
      SELECT 1
      FROM private.account_deletion_jobs AS deletion
      WHERE deletion.owner_user_id = actor.id
    );

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT share.*
  INTO v_share
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL
  FOR UPDATE OF share;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  SELECT EXISTS (
    SELECT 1
    FROM public.workspace_memberships AS membership
    WHERE membership.workspace_id = v_share.workspace_id
      AND membership.user_id = p_actor_user_id
      AND membership.role IN ('owner', 'admin')
      AND membership.deleted_at IS NULL
  )
  INTO v_manage_access;

  SELECT EXISTS (
    SELECT 1
    FROM public.session_access_grants AS access_grant
    WHERE access_grant.share_id = v_share.id
      AND access_grant.grantee_user_id = p_actor_user_id
      AND access_grant.capability = 'editor'
      AND access_grant.revoked_at IS NULL
  )
  INTO v_has_editor_grant;

  IF NOT v_manage_access AND NOT v_has_editor_grant THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY SELECT v_manage_access, v_share.access_version;
END;
$$;

CREATE OR REPLACE FUNCTION private.apply_session_share_snapshot_cas(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_expected_content_revision bigint,
  p_mutation_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[],
  p_web_editable boolean,
  p_is_web_edit boolean
)
RETURNS TABLE (
  outcome text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_now timestamptz := clock_timestamp();
  v_title text := btrim(p_title);
  v_attachment_ids uuid[] := COALESCE(p_attachment_ids, ARRAY[]::uuid[]);
  v_existing_attachment_ids uuid[] := ARRAY[]::uuid[];
  v_retired_ids uuid[] := ARRAY[]::uuid[];
  v_fingerprint bytea;
  v_snapshot public.session_share_snapshots%ROWTYPE;
  v_had_snapshot boolean;
  v_access_version bigint;
BEGIN
  IF p_share_id IS NULL
    OR p_actor_user_id IS NULL
    OR p_expected_content_revision IS NULL
    OR p_expected_content_revision < 0
    OR p_mutation_id IS NULL
    OR v_title IS NULL
    OR octet_length(v_title) > 4096
    OR p_body_json IS NULL
    OR jsonb_typeof(p_body_json) <> 'object'
    OR p_body_json ->> 'type' <> 'doc'
    OR octet_length(p_body_json::text) > 2097152
    OR cardinality(v_attachment_ids) > 64
    OR EXISTS (
      SELECT 1
      FROM unnest(v_attachment_ids) AS requested(id)
      WHERE requested.id IS NULL
    )
    OR cardinality(v_attachment_ids) <> (
      SELECT count(DISTINCT requested.id)
      FROM unnest(v_attachment_ids) AS requested(id)
    )
    OR p_web_editable IS NULL
    OR p_is_web_edit IS NULL
  THEN
    RAISE EXCEPTION 'invalid session share snapshot mutation'
      USING ERRCODE = '22023';
  END IF;

  SELECT share.access_version
  INTO v_access_version
  FROM public.session_shares AS share
  JOIN public.workspaces AS workspace
    ON workspace.id = share.workspace_id
  WHERE share.id = p_share_id
    AND share.deleted_at IS NULL
    AND workspace.deleted_at IS NULL;

  IF NOT FOUND THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  v_fingerprint := extensions.digest(
    convert_to(
      jsonb_build_object(
        'attachmentIds', to_jsonb(v_attachment_ids),
        'body', p_body_json,
        'isWebEdit', p_is_web_edit,
        'title', v_title,
        'webEditable', p_web_editable
      )::text,
      'UTF8'
    ),
    'sha256'
  );

  SELECT snapshot.*
  INTO v_snapshot
  FROM public.session_share_snapshots AS snapshot
  WHERE snapshot.share_id = p_share_id
  FOR UPDATE;
  v_had_snapshot := FOUND;

  IF v_had_snapshot AND v_snapshot.last_mutation_id = p_mutation_id THEN
    IF v_snapshot.published_by_user_id = p_actor_user_id
      AND v_snapshot.last_mutation_base_revision = p_expected_content_revision
      AND v_snapshot.last_mutation_fingerprint = v_fingerprint
    THEN
      RETURN QUERY SELECT
        'replayed'::text,
        v_snapshot.share_id,
        v_snapshot.schema_version,
        v_snapshot.content_revision,
        v_snapshot.title,
        v_snapshot.body_json,
        private.session_share_attachment_manifest(v_snapshot.share_id),
        v_snapshot.web_editable,
        v_access_version,
        v_snapshot.published_at;
      RETURN;
    END IF;

    RAISE EXCEPTION 'session share mutation id is invalid'
      USING ERRCODE = '22023';
  END IF;

  IF NOT v_had_snapshot AND p_expected_content_revision > 0 THEN
    RAISE EXCEPTION 'session share snapshot is unavailable'
      USING ERRCODE = '40001';
  END IF;

  IF COALESCE(v_snapshot.content_revision, 0)
    <> p_expected_content_revision
  THEN
    RETURN QUERY SELECT
      'conflict'::text,
      p_share_id,
      v_snapshot.schema_version,
      v_snapshot.content_revision,
      v_snapshot.title,
      v_snapshot.body_json,
      private.session_share_attachment_manifest(p_share_id),
      v_snapshot.web_editable,
      v_access_version,
      v_snapshot.published_at;
    RETURN;
  END IF;

  SELECT COALESCE(
    array_agg(binding.attachment_id ORDER BY binding.position),
    ARRAY[]::uuid[]
  )
  INTO v_existing_attachment_ids
  FROM public.session_share_snapshot_attachments AS binding
  WHERE binding.share_id = p_share_id;

  IF p_is_web_edit
    AND v_attachment_ids IS DISTINCT FROM v_existing_attachment_ids
  THEN
    RAISE EXCEPTION 'web snapshot attachments must be preserved'
      USING ERRCODE = '22023';
  END IF;

  PERFORM 1
  FROM public.session_share_attachment_objects AS attachment
  WHERE attachment.id = ANY(v_attachment_ids)
  ORDER BY attachment.id
  FOR UPDATE;

  IF cardinality(v_attachment_ids) <> (
    SELECT count(*)
    FROM public.session_share_attachment_objects AS attachment
    WHERE attachment.id = ANY(v_attachment_ids)
      AND attachment.share_id = p_share_id
      AND attachment.state = 'ready'
      AND attachment.sha256 IS NOT NULL
  ) THEN
    RAISE EXCEPTION 'shared snapshot attachment is unavailable'
      USING ERRCODE = '55000';
  END IF;

  SELECT COALESCE(
    array_agg(existing.id ORDER BY existing.id),
    ARRAY[]::uuid[]
  )
  INTO v_retired_ids
  FROM unnest(v_existing_attachment_ids) AS existing(id)
  WHERE NOT (existing.id = ANY(v_attachment_ids));

  IF p_is_web_edit THEN
    IF NOT v_had_snapshot OR NOT v_snapshot.web_editable THEN
      RAISE EXCEPTION 'session snapshot edit not permitted'
        USING ERRCODE = '42501';
    END IF;

    INSERT INTO public.session_share_pending_web_edits (
      share_id,
      base_content_revision,
      base_title,
      base_body_json,
      created_at,
      updated_at
    ) VALUES (
      p_share_id,
      v_snapshot.content_revision,
      v_snapshot.title,
      v_snapshot.body_json,
      v_now,
      v_now
    )
    ON CONFLICT ON CONSTRAINT session_share_pending_web_edits_pkey
    DO UPDATE SET updated_at = excluded.updated_at;
  END IF;

  IF v_had_snapshot THEN
    UPDATE public.session_share_snapshots AS target_snapshot
    SET
      schema_version = 1,
      content_revision = target_snapshot.content_revision + 1,
      title = v_title,
      body_json = p_body_json,
      published_by_user_id = p_actor_user_id,
      published_at = v_now,
      updated_at = v_now,
      web_editable = p_web_editable,
      last_mutation_id = p_mutation_id,
      last_mutation_base_revision = p_expected_content_revision,
      last_mutation_fingerprint = v_fingerprint
    WHERE target_snapshot.share_id = p_share_id
    RETURNING * INTO v_snapshot;
  ELSE
    INSERT INTO public.session_share_snapshots (
      share_id,
      schema_version,
      content_revision,
      title,
      body_json,
      published_by_user_id,
      published_at,
      updated_at,
      web_editable,
      last_mutation_id,
      last_mutation_base_revision,
      last_mutation_fingerprint
    ) VALUES (
      p_share_id,
      1,
      1,
      v_title,
      p_body_json,
      p_actor_user_id,
      v_now,
      v_now,
      p_web_editable,
      p_mutation_id,
      p_expected_content_revision,
      v_fingerprint
    )
    RETURNING * INTO v_snapshot;
  END IF;

  DELETE FROM public.session_share_snapshot_attachments AS binding
  WHERE binding.share_id = p_share_id;

  INSERT INTO public.session_share_snapshot_attachments (
    share_id,
    attachment_id,
    position,
    created_at
  )
  SELECT
    p_share_id,
    requested.id,
    (requested.ordinality - 1)::smallint,
    v_now
  FROM unnest(v_attachment_ids) WITH ORDINALITY AS requested(id, ordinality);

  UPDATE public.session_share_attachment_objects AS attachment
  SET
    state = 'deleting',
    deletion_requested_at = COALESCE(attachment.deletion_requested_at, v_now),
    gc_lease_id = NULL,
    gc_lease_expires_at = NULL,
    updated_at = v_now
  WHERE attachment.id = ANY(v_retired_ids)
    AND attachment.share_id = p_share_id
    AND attachment.state <> 'deleting';

  IF NOT p_is_web_edit THEN
    DELETE FROM public.session_share_pending_web_edits AS pending
    WHERE pending.share_id = p_share_id;
  END IF;

  PERFORM private.write_session_access_event(
    p_share_id,
    'snapshot_published',
    p_actor_user_id,
    NULL,
    p_mutation_id,
    CASE
      WHEN v_snapshot.content_revision > 1
        THEN (v_snapshot.content_revision - 1)::text
      ELSE NULL
    END,
    v_snapshot.content_revision::text
  );

  RETURN QUERY SELECT
    'applied'::text,
    v_snapshot.share_id,
    v_snapshot.schema_version,
    v_snapshot.content_revision,
    v_snapshot.title,
    v_snapshot.body_json,
    private.session_share_attachment_manifest(v_snapshot.share_id),
    v_snapshot.web_editable,
    v_access_version,
    v_snapshot.published_at;
END;
$$;

CREATE OR REPLACE FUNCTION private.publish_session_share_snapshot_cas(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_expected_content_revision bigint,
  p_mutation_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[],
  p_web_editable boolean
)
RETURNS TABLE (
  outcome text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  PERFORM private.require_session_share_attachment_manager(
    p_share_id,
    p_actor_user_id
  );

  RETURN QUERY
  SELECT *
  FROM private.apply_session_share_snapshot_cas(
    p_share_id,
    p_actor_user_id,
    p_expected_content_revision,
    p_mutation_id,
    p_title,
    p_body_json,
    p_attachment_ids,
    p_web_editable,
    false
  );
END;
$$;

CREATE OR REPLACE FUNCTION private.edit_session_share_snapshot_cas(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_expected_content_revision bigint,
  p_mutation_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[]
)
RETURNS TABLE (
  outcome text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_web_editable boolean;
BEGIN
  PERFORM 1
  FROM private.require_session_share_editor(
    p_share_id,
    p_actor_user_id
  );

  SELECT snapshot.web_editable
  INTO v_web_editable
  FROM public.session_share_snapshots AS snapshot
  WHERE snapshot.share_id = p_share_id
  FOR UPDATE;

  IF NOT FOUND OR NOT v_web_editable THEN
    RAISE EXCEPTION 'session snapshot edit not permitted'
      USING ERRCODE = '42501';
  END IF;

  RETURN QUERY
  SELECT *
  FROM private.apply_session_share_snapshot_cas(
    p_share_id,
    p_actor_user_id,
    p_expected_content_revision,
    p_mutation_id,
    p_title,
    p_body_json,
    p_attachment_ids,
    true,
    true
  );
END;
$$;

CREATE OR REPLACE FUNCTION private.read_my_session_share_snapshot_v2(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz,
  web_edit_base_content_revision bigint,
  web_edit_base_title text,
  web_edit_base_body_json jsonb,
  pending_created_at timestamptz,
  pending_updated_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.web_editable,
    access.capability,
    access.manage_access,
    access.access_version,
    snapshot.published_at,
    CASE WHEN access.manage_access
      THEN pending.base_content_revision
      ELSE NULL::bigint
    END,
    CASE WHEN access.manage_access
      THEN pending.base_title
      ELSE NULL::text
    END,
    CASE WHEN access.manage_access
      THEN pending.base_body_json
      ELSE NULL::jsonb
    END,
    CASE WHEN access.manage_access
      THEN pending.created_at
      ELSE NULL::timestamptz
    END,
    CASE WHEN access.manage_access
      THEN pending.updated_at
      ELSE NULL::timestamptz
    END
  FROM private.resolve_my_session_access(p_share_id) AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id
  LEFT JOIN public.session_share_pending_web_edits AS pending
    ON pending.share_id = snapshot.share_id;
$$;

CREATE OR REPLACE FUNCTION private.list_my_session_share_snapshot_page_v2(
  p_after_share_id uuid,
  p_limit integer
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz,
  web_edit_base_content_revision bigint,
  web_edit_base_title text,
  web_edit_base_body_json jsonb,
  pending_created_at timestamptz,
  pending_updated_at timestamptz
)
LANGUAGE plpgsql
STABLE
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF p_limit IS NULL OR p_limit < 1 OR p_limit > 8 THEN
    RAISE EXCEPTION 'invalid session share snapshot page limit'
      USING ERRCODE = '22023';
  END IF;

  RETURN QUERY
  WITH access_page AS MATERIALIZED (
    SELECT access.*
    FROM private.list_my_accessible_sessions() AS access
    WHERE (p_after_share_id IS NULL OR access.share_id > p_after_share_id)
      AND EXISTS (
        SELECT 1
        FROM public.session_share_snapshots AS available_snapshot
        WHERE available_snapshot.share_id = access.share_id
      )
    ORDER BY access.share_id
    LIMIT p_limit
  )
  SELECT
    access.share_id,
    access.workspace_id,
    access.session_id,
    snapshot.schema_version,
    snapshot.content_revision,
    snapshot.title,
    snapshot.body_json,
    private.session_share_attachment_manifest(snapshot.share_id),
    snapshot.web_editable,
    access.capability,
    access.manage_access,
    access.access_version,
    snapshot.published_at,
    CASE WHEN access.manage_access
      THEN pending.base_content_revision
      ELSE NULL::bigint
    END,
    CASE WHEN access.manage_access
      THEN pending.base_title
      ELSE NULL::text
    END,
    CASE WHEN access.manage_access
      THEN pending.base_body_json
      ELSE NULL::jsonb
    END,
    CASE WHEN access.manage_access
      THEN pending.created_at
      ELSE NULL::timestamptz
    END,
    CASE WHEN access.manage_access
      THEN pending.updated_at
      ELSE NULL::timestamptz
    END
  FROM access_page AS access
  JOIN public.session_share_snapshots AS snapshot
    ON snapshot.share_id = access.share_id
  LEFT JOIN public.session_share_pending_web_edits AS pending
    ON pending.share_id = snapshot.share_id
  ORDER BY access.share_id;
END;
$$;

CREATE OR REPLACE FUNCTION private.acknowledge_session_share_web_edits(
  p_share_id uuid,
  p_expected_content_revision bigint
)
RETURNS TABLE (
  share_id uuid,
  acknowledged_content_revision bigint,
  was_pending boolean
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
DECLARE
  v_content_revision bigint;
  v_was_pending boolean;
BEGIN
  IF p_expected_content_revision IS NULL
    OR p_expected_content_revision <= 0
  THEN
    RAISE EXCEPTION 'invalid web edit acknowledgement'
      USING ERRCODE = '22023';
  END IF;

  PERFORM private.require_session_share_manager(p_share_id);

  SELECT snapshot.content_revision
  INTO v_content_revision
  FROM public.session_share_snapshots AS snapshot
  WHERE snapshot.share_id = p_share_id
  FOR UPDATE;

  IF NOT FOUND OR v_content_revision <> p_expected_content_revision THEN
    RAISE EXCEPTION 'web edit acknowledgement conflicts'
      USING ERRCODE = '40001';
  END IF;

  DELETE FROM public.session_share_pending_web_edits AS pending
  WHERE pending.share_id = p_share_id;
  v_was_pending := FOUND;

  RETURN QUERY SELECT p_share_id, v_content_revision, v_was_pending;
END;
$$;

CREATE OR REPLACE FUNCTION public.publish_session_share_snapshot_cas(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_expected_content_revision bigint,
  p_mutation_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[],
  p_web_editable boolean
)
RETURNS TABLE (
  outcome text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.publish_session_share_snapshot_cas(
    p_share_id,
    p_actor_user_id,
    p_expected_content_revision,
    p_mutation_id,
    p_title,
    p_body_json,
    p_attachment_ids,
    p_web_editable
  );
$$;

CREATE OR REPLACE FUNCTION public.edit_session_share_snapshot_cas(
  p_share_id uuid,
  p_actor_user_id uuid,
  p_expected_content_revision bigint,
  p_mutation_id uuid,
  p_title text,
  p_body_json jsonb,
  p_attachment_ids uuid[]
)
RETURNS TABLE (
  outcome text,
  share_id uuid,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  access_version bigint,
  published_at timestamptz
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.edit_session_share_snapshot_cas(
    p_share_id,
    p_actor_user_id,
    p_expected_content_revision,
    p_mutation_id,
    p_title,
    p_body_json,
    p_attachment_ids
  );
$$;

CREATE OR REPLACE FUNCTION public.read_my_session_share_snapshot_v2(
  p_share_id uuid
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz,
  web_edit_base_content_revision bigint,
  web_edit_base_title text,
  web_edit_base_body_json jsonb,
  pending_created_at timestamptz,
  pending_updated_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.read_my_session_share_snapshot_v2(p_share_id);
$$;

CREATE OR REPLACE FUNCTION public.list_my_session_share_snapshot_page_v2(
  p_after_share_id uuid DEFAULT NULL,
  p_limit integer DEFAULT 8
)
RETURNS TABLE (
  share_id uuid,
  workspace_id uuid,
  session_id text,
  schema_version smallint,
  content_revision bigint,
  title text,
  body_json jsonb,
  attachments_json jsonb,
  web_editable boolean,
  capability text,
  manage_access boolean,
  access_version bigint,
  published_at timestamptz,
  web_edit_base_content_revision bigint,
  web_edit_base_title text,
  web_edit_base_body_json jsonb,
  pending_created_at timestamptz,
  pending_updated_at timestamptz
)
LANGUAGE sql
STABLE
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.list_my_session_share_snapshot_page_v2(
    p_after_share_id,
    p_limit
  );
$$;

CREATE OR REPLACE FUNCTION public.acknowledge_session_share_web_edits(
  p_share_id uuid,
  p_expected_content_revision bigint
)
RETURNS TABLE (
  share_id uuid,
  acknowledged_content_revision bigint,
  was_pending boolean
)
LANGUAGE sql
SECURITY INVOKER
SET search_path = ''
AS $$
  SELECT *
  FROM private.acknowledge_session_share_web_edits(
    p_share_id,
    p_expected_content_revision
  );
$$;

REVOKE ALL ON FUNCTION private.require_session_share_editor(uuid, uuid)
  FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.apply_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[], boolean, boolean
) FROM PUBLIC, anon, authenticated, service_role;
REVOKE ALL ON FUNCTION private.publish_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[], boolean
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.edit_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[]
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.read_my_session_share_snapshot_v2(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.list_my_session_share_snapshot_page_v2(uuid, integer)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION private.acknowledge_session_share_web_edits(uuid, bigint)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION private.publish_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[], boolean
) TO service_role;
GRANT EXECUTE ON FUNCTION private.edit_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[]
) TO service_role;
GRANT EXECUTE ON FUNCTION private.read_my_session_share_snapshot_v2(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.list_my_session_share_snapshot_page_v2(
  uuid,
  integer
)
  TO authenticated;
GRANT EXECUTE ON FUNCTION private.acknowledge_session_share_web_edits(
  uuid,
  bigint
) TO authenticated;

REVOKE ALL ON FUNCTION public.publish_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[], boolean
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.edit_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[]
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.read_my_session_share_snapshot_v2(uuid)
  FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.list_my_session_share_snapshot_page_v2(
  uuid,
  integer
) FROM PUBLIC, anon, authenticated;
REVOKE ALL ON FUNCTION public.acknowledge_session_share_web_edits(uuid, bigint)
  FROM PUBLIC, anon, authenticated;

GRANT EXECUTE ON FUNCTION public.publish_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[], boolean
) TO service_role;
GRANT EXECUTE ON FUNCTION public.edit_session_share_snapshot_cas(
  uuid, uuid, bigint, uuid, text, jsonb, uuid[]
) TO service_role;
GRANT EXECUTE ON FUNCTION public.read_my_session_share_snapshot_v2(uuid)
  TO authenticated;
GRANT EXECUTE ON FUNCTION public.list_my_session_share_snapshot_page_v2(
  uuid,
  integer
) TO authenticated;
GRANT EXECUTE ON FUNCTION public.acknowledge_session_share_web_edits(
  uuid,
  bigint
) TO authenticated;

COMMENT ON FUNCTION public.publish_session_share_snapshot(
  uuid,
  uuid,
  text,
  jsonb
) IS 'Legacy service-only writer for pre-CAS clients; unavailable after a snapshot enters CAS mode.';
COMMENT ON FUNCTION public.publish_session_share_snapshot_with_attachments(
  uuid,
  uuid,
  text,
  jsonb,
  uuid[]
) IS 'Legacy service-only writer for pre-CAS clients; unavailable after a snapshot enters CAS mode.';
