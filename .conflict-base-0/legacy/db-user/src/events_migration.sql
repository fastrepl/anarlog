CREATE TABLE IF NOT EXISTS events (
  id TEXT PRIMARY KEY,
  tracking_id TEXT NOT NULL UNIQUE,
  user_id TEXT NOT NULL,
  calendar_id TEXT DEFAULT NULL,
  name TEXT NOT NULL,
  note TEXT NOT NULL,
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  google_event_url TEXT DEFAULT NULL,
  FOREIGN KEY (user_id) REFERENCES humans(id),
  FOREIGN KEY (calendar_id) REFERENCES calendars(id) ON DELETE
  SET
    NULL
);
