DROP INDEX IF EXISTS nango_connections_user_integration_idx;

CREATE UNIQUE INDEX nango_connections_integration_connection_idx
  ON nango_connections (integration_id, connection_id);

CREATE INDEX nango_connections_user_integration_idx
  ON nango_connections (user_id, integration_id);
