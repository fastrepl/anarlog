-- Templates (a favorite that is also the default, an emoji icon, the default icon) and
-- folders (catalog rows with icons and context, a nested path, materials).
INSERT INTO
  templates (
    id,
    title,
    description,
    pinned,
    pin_order,
    category,
    icon_json,
    targets_json,
    sections_json
  )
VALUES
  (
    't-1-sync',
    'Weekly team sync',
    'Status, decisions and owners for the weekly team sync.',
    1,
    1,
    'meetings',
    '{"type":"icon","value":"users","color":"#5b67d8"}',
    '["Engineering","Product"]',
    '[{"title":"Highlights","description":"Summarize the most important updates in two or three bullets."},{"title":"Decisions","description":"List each decision with the person who made it."},{"title":"Action items","description":"{% for item in action_items %}- {{ item }}\n{% endfor %}"}]'
  ),
  (
    't-2-discovery',
    'Customer discovery',
    'Pain points, current workflow and buying signals.',
    0,
    NULL,
    'sales',
    '{"type":"emoji","value":"🔍"}',
    '["Sales"]',
    '[{"title":"Context","description":"Who the customer is and what they do today."},{"title":"Pain points","description":""}]'
  ),
  (
    't-3-interview',
    'Interview debrief',
    '',
    0,
    NULL,
    NULL,
    '{"type":"icon","value":"user-search","color":"#4ab883"}',
    NULL,
    '[{"title":"Strengths","description":""},{"title":"Concerns","description":""},{"title":"Recommendation","description":""}]'
  ),
  (
    't-4-one-on-one',
    '1:1',
    '',
    0,
    NULL,
    NULL,
    '{"type":"icon","value":"messages","color":"#ef923d"}',
    NULL,
    '[]'
  ),
  (
    't-5-standup',
    'Daily standup',
    '',
    0,
    NULL,
    NULL,
    '{"type":"icon","value":"notebook-tabs","color":"#9ca3af"}',
    NULL,
    '["Yesterday","Today","Blockers"]'
  );
INSERT INTO
  app_settings (id, value_json)
VALUES
  ('selected_template_id', '"t-1-sync"') ON CONFLICT(id) DO
UPDATE
SET
  value_json = excluded.value_json;
INSERT INTO
  folders (id, path, icon_json, instructions)
VALUES
  (
    'f-research',
    'Research',
    '{"type":"icon","value":"lightbulb","color":"#f2bd00"}',
    'Customer interviews, usability studies and market research for the product team.'
  ),
  (
    'f-clients',
    'Clients',
    '{"type":"emoji","value":"💼"}',
    ''
  ),
  (
    'f-northwind',
    'Clients/Northwind',
    '{"type":"icon","value":"folder","color":"#9ca3af"}',
    ''
  ),
  (
    'f-hiring',
    'Hiring',
    '{"type":"icon","value":"users","color":"#5b67d8"}',
    ''
  );
INSERT INTO
  folder_attachments (
    id,
    folder_path,
    filename,
    relative_path,
    content_type,
    size_bytes
  )
VALUES
  (
    'm-guide',
    'Research',
    'interview-guide.pdf',
    'materials/m-guide',
    'application/pdf',
    48213
  ),
  (
    'm-personas',
    'Research',
    'personas.md',
    'materials/m-personas',
    'text/markdown',
    3120
  );
