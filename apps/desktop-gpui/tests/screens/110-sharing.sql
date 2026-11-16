-- Sharing fixtures: attendees for the shared design critique, a Team the screenshot account
-- belongs to, and notes others shared with that account (the durable shared-note cache).
INSERT INTO
  session_participants (id, session_id, human_id, display_name, email, source)
VALUES
  ('sp-critique-maya', 's-critique', 'h-maya', 'Maya Chen', 'maya@northwind.example', 'manual'),
  ('sp-critique-priya', 's-critique', 'h-priya', 'Priya Patel', 'priya@northwind.example', 'manual');
INSERT INTO
  workspaces (id, owner_user_id, kind, name)
VALUES
  ('5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d', '7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b', 'shared', 'Lumen');
INSERT INTO
  workspace_memberships (id, workspace_id, user_id, role)
VALUES
  ('wm-lumen-alex', '5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d', '7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b', 'owner');
INSERT INTO
  shared_session_cache (
    share_id,
    viewer_user_id,
    workspace_id,
    session_id,
    content_revision,
    title,
    body_json,
    capability,
    manage_access,
    access_version,
    published_at
  )
VALUES
  (
    '2b6e4f1c-8a3d-4c7e-9f0a-5d1b3e7c9a2f',
    '7f1c2a9e-3b4d-4e5f-8a6b-9c0d1e2f3a4b',
    '5d8f2c1a-7b3e-4f6a-9c2d-1e0b7a6f5c4d',
    'remote-roadmap',
    3,
    'Roadmap review',
    '{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Priorities"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Ship offline sync to every workspace."}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Move the calendar import to the background."}]}]}]},{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Owners"}]},{"type":"paragraph","content":[{"type":"text","text":"Maya leads sync; Priya owns the import work."}]}]}',
    'commenter',
    0,
    1,
    '2026-11-10T16:00:00Z'
  );
