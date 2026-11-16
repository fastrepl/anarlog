-- Contacts fixtures: the local self contact, a team workspace, pinned and unpinned people and
-- organizations, a duplicate pair, an email-only person, a photo avatar, a saved contact summary
-- and note participants for related notes.
INSERT INTO
  organizations (id, owner_user_id, name, pinned, pin_order, metadata_json, created_at)
VALUES
  (
    'org-team',
    '00000000-0000-0000-0000-000000000000',
    'Lumen Labs',
    0,
    NULL,
    '{"teamWorkspace":true}',
    '2026-01-04T09:00:00.000Z'
  ),
  (
    'org-northwind',
    '00000000-0000-0000-0000-000000000000',
    'Northwind Traders',
    1,
    2,
    '{}',
    '2026-02-11T09:00:00.000Z'
  ),
  (
    'org-contoso',
    '00000000-0000-0000-0000-000000000000',
    'Contoso',
    0,
    NULL,
    '{}',
    '2026-03-02T09:00:00.000Z'
  ),
  (
    'org-fabrikam',
    '00000000-0000-0000-0000-000000000000',
    'Fabrikam Studio',
    0,
    NULL,
    '{}',
    '2026-04-20T09:00:00.000Z'
  );
INSERT INTO
  humans (
    id,
    owner_user_id,
    organization_id,
    name,
    email,
    phone,
    job_title,
    linkedin_username,
    memo,
    pinned,
    pin_order,
    metadata_json,
    created_at
  )
VALUES
  (
    '00000000-0000-0000-0000-000000000000',
    '00000000-0000-0000-0000-000000000000',
    'org-team',
    'Alex Rivera',
    'alex@lumen.example',
    '+1 (415) 555-0110',
    'Product Lead',
    'alexrivera',
    '',
    0,
    NULL,
    '{}',
    '2026-01-04T09:00:00.000Z'
  ),
  (
    'h-maya',
    '00000000-0000-0000-0000-000000000000',
    'org-northwind',
    'Maya Chen',
    'maya@northwind.example',
    '+1 (206) 555-0142',
    'Head of Operations',
    'mayachen',
    'Prefers async updates before calls.',
    1,
    1,
    json_object(
      'additionalEmails',
      json_array('maya.chen@mail.example'),
      'contactSummary',
      json_object(
        'facts',
        json_array(
          'You agreed to send Maya the revised rollout plan before the next weekly sync.',
          'Northwind is piloting the product with two warehouse teams through the end of the quarter.',
          'Maya owns the vendor budget and wants pricing options compared side by side.'
        ),
        'sourceHash',
        'seed',
        'promptKey',
        'seed',
        'generatedAt',
        '2026-11-01T09:00:00.000Z',
        'sources',
        json_array()
      )
    ),
    '2026-02-11T10:00:00.000Z'
  ),
  (
    'h-jordan',
    '00000000-0000-0000-0000-000000000000',
    'org-contoso',
    'Jordan Lee',
    'jordan@contoso.example',
    '',
    'Design Manager',
    '',
    '',
    0,
    NULL,
    '{}',
    '2026-03-02T10:00:00.000Z'
  ),
  (
    'h-jordan-2',
    '00000000-0000-0000-0000-000000000000',
    '',
    'Jordan Lee',
    'jlee@mail.example',
    '',
    '',
    '',
    '',
    0,
    NULL,
    '{}',
    '2026-06-15T10:00:00.000Z'
  ),
  (
    'h-priya',
    '00000000-0000-0000-0000-000000000000',
    'org-northwind',
    'Priya Patel',
    'priya@northwind.example',
    '',
    'Staff Engineer',
    'https://www.linkedin.com/in/priyapatel/',
    '',
    0,
    NULL,
    json_object('avatarDataUrl', 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/2wBDAQcHBwoIChMKChMoGhYaKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCj/wAARCABAAEADASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDiFWpVWlVamVa+oqSPCoyGqtTKtXtE0m61i/jtLGPdI3JJ4VF7sx7Af55r1HRfh3pltDG2pmS7uCmHUOVjDZ/hxg8dOTz1wO3j4vF06Oknqezhac6mq2PJVWpVWvXtQ8AaNcR4tVmtJADgo5YE9iQ2c49iK861zQrzRLpYb1VIYZSRCSj+uD7dx/iK4VioVvhPSjGUNzLValVaVVqVVrnqSPQoyMJVqVVpVWpVWvqKkj88oyPV/hXpcdtoZv2jjNxcuwWQZLCMHG32+YE8dePTjt65H4ZXiXHhpbcbRJayMhG7JIJ3BsdupH/ATXXV8ZjHJ15c3f8A4Y+zwlvYx5ewVz/jjTY9Q0C4cohnt1MsbtwVA5bH1APHTOPSugrH8XXa2nh29ZtpMiGFVLYyW449cAk49qwptqSsdEttTx1VqVVpVWpVWuypIKMjBValVaVVp0jxwRNLKwSNRkk19RUkfntGVzV8OarPomordwKrjGyRG/jUkZGe3Qc+3fpXqOk+LNKv4UL3CW05Xc8cx2hT0xuPB/njtXzpf6/PNlLUeTH03dWPX8v881kTTSTMGmkeRgMZdiTivLxGDjXd3oz6LB1qtBW6dj6pvfEmk2ke5r2KUkEhYT5hOO3HA/HFeeeItbm1y5jd41ihiBEcYOcZxkk984H5V4xFI8UgeJ2Rx0ZTgj8a2dN8R3lphZv9Ji9HPzDr/F/jmuf+zvZ6xd2dcsXKej0R3arUqrUGm3cF/arPbNuQ8EHqp9D71eVa82q2nZnbRncwVWuS8SXpuL1oEY+TCduOxbuf6fh712arXmtfUzd2fGZVFSbk+gUUUVJ7YUUUUAanhzU20zUo3ZyLdyFlXsR6/h1/TvXqirXi9ewaCv8AxJdP/wCveP8A9BFeLmsErTW514WbTsf/2Q=='),
    '2026-02-12T10:00:00.000Z'
  ),
  (
    'h-sam',
    '00000000-0000-0000-0000-000000000000',
    'org-fabrikam',
    'Sam Okafor',
    '',
    '',
    'Founder',
    '',
    '',
    0,
    NULL,
    '{}',
    '2026-04-20T10:00:00.000Z'
  ),
  (
    'h-taylor',
    '00000000-0000-0000-0000-000000000000',
    '',
    '',
    'taylor@fabrikam.example',
    '',
    '',
    '',
    '',
    0,
    NULL,
    '{}',
    '2026-05-08T10:00:00.000Z'
  );
INSERT INTO
  session_participants (id, session_id, human_id, display_name, email, source)
VALUES
  ('sp-pricing-maya', 's-pricing', 'h-maya', 'Maya Chen', 'maya@northwind.example', 'manual'),
  ('sp-hiring-maya', 's-hiring', 'h-maya', 'Maya Chen', 'maya@northwind.example', 'manual'),
  ('sp-offsite-maya', 's-offsite', 'h-maya', 'Maya Chen', 'maya@northwind.example', 'manual'),
  ('sp-hiring-priya', 's-hiring', 'h-priya', 'Priya Patel', 'priya@northwind.example', 'manual'),
  ('sp-offsite-jordan', 's-offsite', 'h-jordan', 'Jordan Lee', 'jordan@contoso.example', 'manual'),
  ('sp-pricing-jordan', 's-pricing', 'h-jordan', 'Jordan Lee', 'jordan@contoso.example', 'excluded'),
  ('sp-hiring-self', 's-hiring', '00000000-0000-0000-0000-000000000000', 'Alex Rivera', 'alex@lumen.example', 'manual');
