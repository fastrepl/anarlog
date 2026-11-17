-- Meeting info: a participant on a note without an event, and a past calendar event with a
-- meeting link, location and description on another.
INSERT INTO
  session_participants (
    id,
    session_id,
    human_id,
    display_name,
    email,
    source
  )
VALUES
  (
    'sp-sync-maya',
    's-sync',
    'h-maya',
    'Maya Chen',
    'maya@northwind.example',
    'manual'
  );
UPDATE
  sessions
SET
  event_json = json_object(
    'title',
    'Hiring debrief',
    'started_at',
    created_at,
    'ended_at',
    strftime('%Y-%m-%dT%H:%M:%SZ', created_at, '+45 minutes'),
    'location',
    'Room 4B',
    'meeting_link',
    'https://meet.google.com/abc-defg-hij',
    'description',
    'Scorecards are in https://example.com/scorecards. Bring your notes.'
  )
WHERE
  id = 's-hiring';
