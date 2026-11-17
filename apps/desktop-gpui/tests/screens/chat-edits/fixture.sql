-- Applied after the other captures (it is outside `tests/screens/*.sql`): pending memo and
-- summary proposals from the CLI and MCP, and a saved chat whose replies used tools.
-- A transcription provider hides the "provider needed" toast, as in the full run.
INSERT
  OR IGNORE INTO app_settings (id, value_json)
VALUES
  ('current_stt_provider', '"deepgram"'),
  ('current_stt_model', '"nova-3"');
INSERT INTO
  session_proposals (
    id,
    session_id,
    kind,
    target_id,
    base_updated_at,
    current_markdown,
    proposed_markdown,
    status,
    source
  )
VALUES
  (
    'p-sync-memo',
    's-sync',
    'memo_replace',
    's-sync',
    (
      SELECT
        updated_at
      FROM
        session_documents
      WHERE
        id = 's-sync'
    ),
    '## Agenda

- Release status and open blockers
- Onboarding funnel: activation up 4%
- Next quarter themes

## Notes

- Share the release checklist
- Draft the pricing FAQ',
    '## Agenda

- Release status and open blockers
- Onboarding funnel: activation up 4%
- Pricing FAQ review
- Next quarter themes

## Notes

- Share the release checklist by Thursday
- Draft the pricing FAQ',
    'pending',
    'cli'
  ),
  (
    'p-critique-summary',
    's-critique',
    'summary_replace',
    's-critique-summary',
    (
      SELECT
        updated_at
      FROM
        session_documents
      WHERE
        id = 's-critique-summary'
    ),
    '## Decisions

- Ship the simplified onboarding in the next release.
- Keep the calendar step optional.

## Follow-ups

Mina shares updated mocks by Friday.',
    '## Decisions

- Ship the simplified onboarding next release.
- The calendar step stays optional.

## Follow-ups

Mina shares updated mocks by Friday.',
    'pending',
    'mcp'
  );
INSERT INTO
  chat_groups (id, title, created_at, updated_at)
VALUES
  (
    'g-edits',
    'Tighten the critique summary',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-3 days'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-3 days')
  );
INSERT INTO
  chat_messages (
    id,
    chat_group_id,
    role,
    content,
    metadata_json,
    parts_json,
    created_at
  )
VALUES
  (
    'm-edits-1',
    'g-edits',
    'user',
    'Find the design critique and tighten its summary.',
    '{}',
    '[{"type":"text","text":"Find the design critique and tighten its summary."}]',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-3 days')
  ),
  (
    'm-edits-2',
    'g-edits',
    'assistant',
    '',
    '{}',
    '[{"type":"step-start"},{"type":"reasoning","text":"Looking for the design critique note.","state":"done"},{"type":"tool-search_meetings","toolCallId":"call-search","state":"output-available","input":{"query":"design critique","limit":5},"output":{"results":[{"id":"s-critique","title":"Design critique","excerpt":"Ship the simplified onboarding in the next release. Keep the calendar step optional.","score":3,"created_at":1762000000000},{"id":"s-crit","title":"Design critique notes","excerpt":"","score":2,"created_at":1761400000000},{"id":"s-standup","title":"Design standup","excerpt":"Blocked on the onboarding copy review.","score":1,"created_at":1760800000000}]}},{"type":"tool-get_meeting","toolCallId":"call-get","state":"output-available","input":{"meeting_id":"s-critique"},"output":{"id":"s-critique","title":"Design critique","folder_path":"Research"}},{"type":"step-start"},{"type":"tool-edit_summary","toolCallId":"call-edit-summary","state":"output-available","input":{"content":"## Decisions\n\n- Ship the simplified onboarding next release.\n- The calendar step stays optional."},"output":{"status":"applied"}},{"type":"step-start"},{"type":"text","state":"done","text":"I tightened the **Design critique** summary."}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-3 days',
      '+20 seconds'
    )
  ),
  (
    'm-edits-3',
    'g-edits',
    'user',
    'Draft a memo agenda for next week.',
    '{}',
    '[{"type":"text","text":"Draft a memo agenda for next week."}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-3 days',
      '+2 minutes'
    )
  ),
  (
    'm-edits-4',
    'g-edits',
    'assistant',
    '',
    '{}',
    '[{"type":"step-start"},{"type":"tool-edit_memo","toolCallId":"call-edit-memo","state":"output-available","input":{"content":"## Agenda\n\n- Onboarding results\n- Pricing FAQ"},"output":{"status":"declined"}},{"type":"step-start"},{"type":"text","state":"done","text":"Okay, I left the memo unchanged."}]',
    strftime(
      '%Y-%m-%dT%H:%M:%fZ',
      'now',
      '-3 days',
      '+2 minutes',
      '+10 seconds'
    )
  );
