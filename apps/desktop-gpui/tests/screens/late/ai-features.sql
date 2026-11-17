-- AI features: an upcoming pricing follow-up with Maya, whose earlier readout has a summary
-- (the pre-meeting brief's source), and an attendee known only by email (Enhance contact).
-- Applied and removed by the `ai_features` capture, which also saves it for shipping.
INSERT INTO
  events (
    id,
    tracking_id_event,
    calendar_id,
    title,
    started_at,
    ended_at,
    description,
    participants_json,
    provider
  )
VALUES
  (
    'evt-ai-followup',
    'ai-followup',
    'cal-work',
    'Pricing follow-up',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+3 hours'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+4 hours'),
    'Pricing follow-up between Maya Chen and Sam Ortiz',
    '[{"name":"Maya Chen","email":"maya@northwind.example"},{"name":"Sam Ortiz","email":"sam.ortiz@globex.example"},{"name":"Alex Rivera","email":"alex@lumen.example","is_current_user":true}]',
    'apple'
  );
INSERT INTO
  sessions (id, title, owner_user_id, created_at, event_id, event_json, folder_path)
VALUES
  (
    's-ai-followup',
    'Pricing follow-up',
    '00000000-0000-0000-0000-000000000000',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-10 minutes'),
    'evt-ai-followup',
    json_object(
      'tracking_id',
      'ai-followup',
      'calendar_id',
      'cal-work',
      'title',
      'Pricing follow-up',
      'description',
      'Pricing follow-up between Maya Chen and Sam Ortiz',
      'started_at',
      strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+3 hours'),
      'ended_at',
      strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+4 hours')
    ),
    ''
  );
INSERT INTO
  humans (id, name, email)
VALUES
  ('h-ai-sam', 'sam.ortiz@globex.example', '');
INSERT INTO
  session_participants (id, session_id, human_id, display_name, email, source)
VALUES
  (
    'sp-ai-maya',
    's-ai-followup',
    'h-maya',
    'Maya Chen',
    'maya@northwind.example',
    'auto'
  ),
  (
    'sp-ai-sam',
    's-ai-followup',
    'h-ai-sam',
    '',
    'sam.ortiz@globex.example',
    'auto'
  );
INSERT INTO
  session_documents (id, session_id, kind, title, body_format, body, sort_order)
VALUES
  (
    'd-ai-pricing-summary',
    's-pricing',
    'summary',
    'Summary',
    'markdown',
    '## Decisions
- Keep the annual plan discount at 15% through Q3.
- Maya owns the revised pricing page copy.

## Open questions
- Legal still has to approve the reseller terms.',
    1
  );
