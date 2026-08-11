-- The local HTTP API was removed; api_keys only ever authenticated its
-- requests. Webhook endpoints stay, and cloud API keys live server-side.
DROP TABLE IF EXISTS api_keys;

-- Webhook fanout used to run only while the local API server was enabled, so
-- endpoints belonging to a user who left it off never delivered anything.
-- Dropping the server removes that gate, which would silently start POSTing
-- meeting notes and transcripts to those URLs on first launch after upgrade.
-- Pause them instead; Settings shows the paused state with an Enable action so
-- the user opts back in per endpoint.
UPDATE webhook_endpoints
SET active = 0
WHERE COALESCE(
    (
      SELECT json_extract(value_json, '$.enabled')
      FROM app_settings
      WHERE id = 'local_api'
    ),
    0
  ) = 0;
