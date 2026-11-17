INSERT INTO session_documents (id, session_id, kind, title, body_format, body, source_hash, sort_order, created_at, updated_at)
VALUES
  ('s-sync:meeting-screen:screen-1', 's-sync', 'meeting_screen', 'Zoom shared screen', 'json',
    '{"id":"s-sync:meeting-screen:screen-1","attachmentId":"screen-1","platform":"zoom","width":1440,"height":900,"capturedAt":"2026-11-17T10:01:00.000Z"}',
    'screen-1', 1, '2026-11-17T10:01:00.000Z', '2026-11-17T10:01:00.000Z'),
  ('s-sync:meeting-screen:screen-2', 's-sync', 'meeting_screen', 'Shared screen', 'json',
    '{"id":"s-sync:meeting-screen:screen-2","attachmentId":"screen-2","platform":null,"width":0,"height":0,"capturedAt":"2026-11-17T10:03:00.000Z"}',
    'screen-2', 2, '2026-11-17T10:03:00.000Z', '2026-11-17T10:03:00.000Z'),
  ('s-sync:meeting-chat:1', 's-sync', 'meeting_chat', 'Zoom chat', 'json',
    '{"id":"m1","platform":"zoom","surface":"native","sender":"Mina Park","timestamp":"10:02","direction":"incoming","text":"Here is the release checklist: https://example.com/checklist","links":["https://example.com/checklist"],"capturedAt":"2026-11-17T10:02:00.000Z"}',
    'c1', 1, '2026-11-17T10:02:00.000Z', '2026-11-17T10:02:00.000Z'),
  ('s-sync:meeting-chat:2', 's-sync', 'meeting_chat', 'Zoom chat', 'json',
    '{"id":"m2","platform":"zoom","surface":"native","sender":"John","timestamp":"10:03","direction":"outgoing","text":"Thanks, I will review it after the call.","links":[],"capturedAt":"2026-11-17T10:03:00.000Z"}',
    'c2', 2, '2026-11-17T10:03:00.000Z', '2026-11-17T10:03:00.000Z'),
  ('s-sync:meeting-chat:3', 's-sync', 'meeting_chat', 'Zoom chat', 'json',
    '{"id":"m3","platform":"zoom","surface":"native","sender":"Alex Kim","timestamp":null,"direction":null,"text":"Two follow-ups:\n1. Pricing FAQ\n2. Onboarding copy","links":[],"capturedAt":"2026-11-17T10:05:00.000Z"}',
    'c3', 3, '2026-11-17T10:05:00.000Z', '2026-11-17T10:05:00.000Z');
