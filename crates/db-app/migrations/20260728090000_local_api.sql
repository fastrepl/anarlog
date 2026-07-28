CREATE TABLE IF NOT EXISTS api_keys (
  id            TEXT PRIMARY KEY CHECK (
    id = trim(id)
    AND length(id) > 0
    AND length(id) <= 512
  ),
  name          TEXT NOT NULL DEFAULT '' CHECK (length(name) <= 200),
  key_hash      TEXT NOT NULL UNIQUE CHECK (
    length(key_hash) = 64
    AND key_hash = lower(key_hash)
    AND key_hash NOT GLOB '*[^0-9a-f]*'
  ),
  key_prefix    TEXT NOT NULL CHECK (length(key_prefix) <= 16),
  created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_used_at  TEXT,
  revoked_at    TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS webhook_endpoints (
  id                    TEXT PRIMARY KEY CHECK (
    id = trim(id)
    AND length(id) > 0
    AND length(id) <= 512
  ),
  url                   TEXT NOT NULL CHECK (
    url = trim(url)
    AND length(url) > 0
    AND length(url) <= 2048
  ),
  secret                TEXT NOT NULL CHECK (length(secret) > 0 AND length(secret) <= 200),
  events_json           TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(events_json)),
  active                INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  created_at            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  last_delivery_at      TEXT,
  last_delivery_status  TEXT NOT NULL DEFAULT '' CHECK (length(last_delivery_status) <= 200)
) STRICT;
