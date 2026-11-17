-- An app excluded from meeting detection, shown as a badge under Notifications.
INSERT INTO
  app_settings (id, value_json)
VALUES
  ('ignored_platforms', '"[\"us.zoom.xos\"]"');
