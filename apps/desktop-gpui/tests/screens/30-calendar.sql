-- A busy day two weeks back: an all-day event plus more meetings than a month cell can
-- show, so the calendar renders all-day chips, "+N more" and full event details.
INSERT INTO
  events (
    id,
    calendar_id,
    title,
    started_at,
    ended_at,
    is_all_day,
    location,
    meeting_link,
    description,
    provider
  )
VALUES
  (
    'evt-offsite',
    'cal-work',
    'Company offsite',
    date('now', '-13 days'),
    date('now', '-12 days'),
    1,
    '',
    '',
    '',
    'apple'
  ),
  (
    'evt-roadmap',
    'cal-work',
    'Roadmap review',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+16 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+17 hours'
    ),
    0,
    '',
    '',
    '',
    'apple'
  ),
  (
    'evt-onboarding',
    'cal-work',
    'Customer onboarding',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+18 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+19 hours'
    ),
    0,
    'Room 4B',
    'https://meet.example.com/abc-defg-hij',
    'Walk through the import flow and calendar sync.',
    'apple'
  ),
  (
    'evt-hiring',
    'cal-work',
    'Hiring sync',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+20 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+20 hours',
      '+30 minutes'
    ),
    0,
    '',
    '',
    '',
    'apple'
  ),
  (
    'evt-retro',
    'cal-work',
    'Sprint retro',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+22 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-13 days',
      'start of day',
      '+23 hours'
    ),
    0,
    '',
    '',
    '',
    'apple'
  );
