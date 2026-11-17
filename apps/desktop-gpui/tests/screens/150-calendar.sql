-- A weekly design critique. Last week's occurrence has a note written an hour ago, so the
-- timeline must place it at the meeting (a week ago), not at its creation time. Next
-- week's occurrence has no note yet; filing it into a folder offers auto-add.
INSERT INTO
  events (
    id,
    tracking_id_event,
    calendar_id,
    title,
    started_at,
    ended_at,
    recurrence_series_id,
    has_recurrence_rules,
    provider
  )
VALUES
  (
    'evt-crit-past',
    'crit-past',
    'cal-work',
    'Design critique',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-7 days',
      'start of day',
      '+6 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '-7 days',
      'start of day',
      '+7 hours'
    ),
    'series-crit',
    1,
    'apple'
  ),
  (
    'evt-crit-next',
    'crit-next',
    'cal-work',
    'Design critique',
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+7 days',
      'start of day',
      '+6 hours'
    ),
    strftime(
      '%Y-%m-%dT%H:%M:%SZ',
      'now',
      '+7 days',
      'start of day',
      '+7 hours'
    ),
    'series-crit',
    1,
    'apple'
  );
INSERT INTO
  sessions (
    id,
    title,
    created_at,
    event_id,
    series_id,
    event_json,
    folder_path
  )
VALUES
  (
    's-crit',
    'Design critique notes',
    strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-1 hours'),
    'evt-crit-past',
    'series-crit',
    json_object(
      'tracking_id',
      'crit-past',
      'calendar_id',
      'cal-work',
      'title',
      'Design critique',
      'started_at',
      strftime(
        '%Y-%m-%dT%H:%M:%SZ',
        'now',
        '-7 days',
        'start of day',
        '+6 hours'
      ),
      'ended_at',
      strftime(
        '%Y-%m-%dT%H:%M:%SZ',
        'now',
        '-7 days',
        'start of day',
        '+7 hours'
      ),
      'recurrence_series_id',
      'series-crit'
    ),
    ''
  );
