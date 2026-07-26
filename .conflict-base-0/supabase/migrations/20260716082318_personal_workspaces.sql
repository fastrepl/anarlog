CREATE TABLE public.workspaces (
  id uuid PRIMARY KEY,
  owner_user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  kind text NOT NULL DEFAULT 'personal',
  name text NOT NULL DEFAULT 'Personal',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  CONSTRAINT workspaces_kind_check CHECK (kind = 'personal'),
  CONSTRAINT workspaces_personal_identity_check CHECK (id = owner_user_id)
);

CREATE TABLE public.workspace_memberships (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES public.workspaces(id) ON DELETE CASCADE,
  user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  role text NOT NULL DEFAULT 'member',
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  CONSTRAINT workspace_memberships_role_check CHECK (
    role IN ('owner', 'admin', 'member')
  ),
  CONSTRAINT workspace_memberships_workspace_user_key UNIQUE (workspace_id, user_id)
);

CREATE UNIQUE INDEX workspaces_personal_owner_key
  ON public.workspaces(owner_user_id)
  WHERE kind = 'personal' AND deleted_at IS NULL;

CREATE INDEX workspace_memberships_user_active_idx
  ON public.workspace_memberships(user_id, workspace_id)
  WHERE deleted_at IS NULL;

ALTER TABLE public.workspaces ENABLE ROW LEVEL SECURITY;
ALTER TABLE public.workspace_memberships ENABLE ROW LEVEL SECURITY;

REVOKE ALL ON TABLE public.workspaces FROM PUBLIC, anon, authenticated;
REVOKE ALL ON TABLE public.workspace_memberships FROM PUBLIC, anon, authenticated;
GRANT SELECT ON TABLE public.workspaces TO authenticated;
GRANT SELECT ON TABLE public.workspace_memberships TO authenticated;
GRANT ALL ON TABLE public.workspaces TO service_role;
GRANT ALL ON TABLE public.workspace_memberships TO service_role;

CREATE POLICY workspaces_select_member
  ON public.workspaces
  FOR SELECT
  TO authenticated
  USING (
    deleted_at IS NULL
    AND id IN (
      SELECT membership.workspace_id
      FROM public.workspace_memberships AS membership
      WHERE membership.user_id = (SELECT auth.uid())
        AND membership.deleted_at IS NULL
    )
  );

CREATE POLICY workspaces_service_all
  ON public.workspaces
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE POLICY workspace_memberships_select_self
  ON public.workspace_memberships
  FOR SELECT
  TO authenticated
  USING (
    user_id = (SELECT auth.uid())
    AND deleted_at IS NULL
  );

CREATE POLICY workspace_memberships_service_all
  ON public.workspace_memberships
  FOR ALL
  TO service_role
  USING (true)
  WITH CHECK (true);

CREATE SCHEMA IF NOT EXISTS private;
REVOKE ALL ON SCHEMA private FROM PUBLIC, anon, authenticated;
GRANT USAGE ON SCHEMA private TO supabase_auth_admin;

CREATE OR REPLACE FUNCTION private.handle_new_user_workspace()
RETURNS trigger
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = ''
AS $$
BEGIN
  IF COALESCE(NEW.is_anonymous, false) THEN
    RETURN NEW;
  END IF;

  INSERT INTO public.workspaces (id, owner_user_id, kind)
  VALUES (NEW.id, NEW.id, 'personal')
  ON CONFLICT (id) DO UPDATE SET
    deleted_at = NULL,
    updated_at = now();

  INSERT INTO public.workspace_memberships (id, workspace_id, user_id, role)
  VALUES (NEW.id, NEW.id, NEW.id, 'owner')
  ON CONFLICT (workspace_id, user_id) DO UPDATE SET
    role = 'owner',
    deleted_at = NULL,
    updated_at = now();

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION private.handle_new_user_workspace() FROM PUBLIC, anon, authenticated;
GRANT EXECUTE ON FUNCTION private.handle_new_user_workspace() TO supabase_auth_admin;

DROP TRIGGER IF EXISTS on_auth_user_workspace_created ON auth.users;
CREATE TRIGGER on_auth_user_workspace_created
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE FUNCTION private.handle_new_user_workspace();

INSERT INTO public.workspaces (
  id,
  owner_user_id,
  kind,
  name,
  created_at,
  updated_at
)
SELECT id, id, 'personal', 'Personal', created_at, created_at
FROM auth.users
WHERE COALESCE(is_anonymous, false) = false
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.workspace_memberships (
  id,
  workspace_id,
  user_id,
  role,
  created_at,
  updated_at
)
SELECT id, id, id, 'owner', created_at, created_at
FROM auth.users
WHERE COALESCE(is_anonymous, false) = false
ON CONFLICT (workspace_id, user_id) DO UPDATE SET
  role = 'owner',
  deleted_at = NULL,
  updated_at = now();
