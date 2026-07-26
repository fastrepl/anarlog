#!/usr/bin/env bash
set -euo pipefail

# Usage: scripts/nango.sh {create,refresh,delete} INTEGRATION_ID CONNECTION_ID [USER_ID]
#
# create  INTEGRATION_ID CONNECTION_ID USER_ID  - upsert connection as connected
# refresh INTEGRATION_ID CONNECTION_ID          - mark connection as reconnect_required
# delete  INTEGRATION_ID CONNECTION_ID          - delete connection

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="$SCRIPT_DIR/../.env.supabase"
DATABASE_URL=$(grep '^DATABASE_URL=' "$ENV_FILE" | sed "s/^DATABASE_URL=//;s/^['\"]//;s/['\"]$//")

[[ $# -lt 3 ]] && { echo "Usage: $0 {create,refresh,delete} INTEGRATION_ID CONNECTION_ID [USER_ID]"; exit 1; }

OP="$1" INTEGRATION="$2" CONNECTION="$3"

case "$OP" in
  create)
    USER_ID="${4:?Usage: $0 create INTEGRATION_ID CONNECTION_ID USER_ID}"
    psql "$DATABASE_URL" -c "
      INSERT INTO nango_connections (user_id, integration_id, connection_id, provider, status, updated_at)
      VALUES ('$USER_ID', '$INTEGRATION', '$CONNECTION', '$INTEGRATION', 'connected', now())
      ON CONFLICT (integration_id, connection_id) DO UPDATE SET
        status = 'connected',
        last_error_type = NULL,
        last_error_description = NULL,
        last_error_at = NULL,
        updated_at = now();"
    ;;
  refresh)
    psql "$DATABASE_URL" -c "
      UPDATE nango_connections SET
        status = 'reconnect_required',
        last_error_type = 'refresh_token_error',
        last_error_description = 'Token expired',
        last_error_at = now(),
        updated_at = now()
      WHERE integration_id = '$INTEGRATION' AND connection_id = '$CONNECTION';"
    ;;
  delete)
    psql "$DATABASE_URL" -c "
      DELETE FROM nango_connections
      WHERE integration_id = '$INTEGRATION' AND connection_id = '$CONNECTION';"
    ;;
  *)
    echo "Unknown operation: $OP"
    echo "Usage: $0 {create,refresh,delete} INTEGRATION_ID CONNECTION_ID [USER_ID]"
    exit 1
    ;;
esac
