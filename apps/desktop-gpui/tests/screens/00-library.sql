-- Fictional library for visual comparison. Times are relative to `now` so the timeline
-- buckets (Tomorrow, Today, 6 days ago, ...) render the same way on every run.
INSERT INTO
  calendars (id, name, enabled, provider, source, color)
VALUES
  (
    'cal-work',
    'Work',
    1,
    'apple',
    'apple',
    '#4f46e5'
  );
INSERT INTO
  events (
    id,
    calendar_id,
    title,
    started_at,
    ended_at,
    provider
  )
VALUES
  (
    'evt-review',
    'cal-work',
    'Design review',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+2 hours'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '+3 hours'),
    'apple'
  ),
  (
    'evt-standup',
    'cal-work',
    'Team standup',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+1 day',
      'start of day',
      '+9 hours',
      '+30 minutes'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+1 day',
      'start of day',
      '+9 hours',
      '+45 minutes'
    ),
    'apple'
  ),
  (
    'evt-planning',
    'cal-work',
    'Quarterly planning',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+1 day',
      'start of day',
      '+18 hours',
      '+30 minutes'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+1 day',
      'start of day',
      '+19 hours',
      '+30 minutes'
    ),
    'apple'
  );
INSERT INTO
  sessions (
    id,
    title,
    created_at,
    started_at,
    ended_at,
    folder_path
  )
VALUES
  (
    's-welcome',
    'Welcome to Anarlog',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-90 minutes'),
    '',
    '',
    ''
  ),
  (
    's-sync',
    'Weekly product sync',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 hours'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-5 hours'),
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-4 hours'),
    'Research'
  ),
  (
    's-korean',
    '주간 회의록',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-6 days'),
    '',
    '',
    ''
  ),
  (
    's-customer',
    'Customer call with Northwind',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-7 days',
      '-3 hours'
    ),
    '',
    '',
    'Research'
  ),
  (
    's-hiring',
    'Hiring debrief',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-19 days'),
    '',
    '',
    ''
  ),
  (
    's-pricing',
    'Pricing experiment readout',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-21 days',
      '-2 hours'
    ),
    '',
    '',
    ''
  ),
  (
    's-offsite',
    'Offsite logistics',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-40 days'),
    '',
    '',
    ''
  );
INSERT INTO
  session_documents (id, session_id, kind, body_format, body)
VALUES
  (
    's-sync',
    's-sync',
    'note',
    'prosemirror_json',
    '{"type":"doc","content":[
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Agenda"}]},
    {"type":"bulletList","content":[
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Release status and open blockers"}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Onboarding funnel: "},{"type":"text","marks":[{"type":"bold"}],"text":"activation up 4%"}]}]},
      {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Next quarter themes"}]}]}]},
    {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Notes"}]},
    {"type":"paragraph","content":[{"type":"text","text":"Search should feel instant on large libraries. Typing 한글 and emoji 🦀 must go through the platform input method. "},{"type":"text","marks":[{"type":"italic"}],"text":"Follow up with design."}]},
    {"type":"taskList","content":[
      {"type":"taskItem","attrs":{"checked":true},"content":[{"type":"paragraph","content":[{"type":"text","text":"Share the release checklist"}]}]},
      {"type":"taskItem","attrs":{"checked":false},"content":[{"type":"paragraph","content":[{"type":"text","text":"Draft the pricing FAQ"}]}]}]}]}'
  ),
  (
    's-korean',
    's-korean',
    'note',
    'markdown',
    '## 안건

- 출시 일정 확인
- 온보딩 개선안 검토

## 메모

다음 주까지 초안을 공유합니다.'
  ),
  (
    's-customer',
    's-customer',
    'note',
    'markdown',
    'Northwind wants calendar sync for shared rooms and an export to their wiki.'
  );
INSERT INTO
  tags (id, name)
VALUES
  ('tag-product', 'product');
INSERT INTO
  session_tags (id, session_id, tag_id)
VALUES
  ('st-sync', 's-sync', 'tag-product');
