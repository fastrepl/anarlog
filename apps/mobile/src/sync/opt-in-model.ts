export type CloudSyncOptInRow = {
  account_user_id: string;
  preference_json: string | null;
  binding_json: string | null;
  connected_account?: number;
  has_connections?: number;
};

export const CLOUD_SYNC_OPT_IN_SQL = `
SELECT
  ? AS account_user_id,
  (SELECT value_json FROM app_settings WHERE id = 'cloud_sync_enabled') AS preference_json,
  (SELECT value_json FROM app_settings WHERE id = 'cloudsync_workspace_binding') AS binding_json,
  EXISTS(SELECT 1 FROM local_library_connections WHERE active = 1 AND account_user_id = ?1) AS connected_account,
  EXISTS(SELECT 1 FROM local_library_connections) AS has_connections
`;

function parseJson(value: string | null): unknown {
  if (value === null) return undefined;
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
}

// Sync is opt-in: signing in must not bind the local database to an account.
// A device that already claimed its workspace for this account keeps syncing
// so existing installs are not switched off by the upgrade.
export function resolveCloudSyncOptIn(rows: CloudSyncOptInRow[]): boolean {
  const row = rows[0];
  if (!row) return false;
  const preference = parseJson(row.preference_json);
  if (typeof preference === "boolean") return preference;
  if (row.connected_account) return true;
  if (row.has_connections) return false;
  const binding = parseJson(row.binding_json);
  if (typeof binding !== "object" || binding === null) return false;
  const { workspace_id, account_user_id } = binding as {
    workspace_id?: unknown;
    account_user_id?: unknown;
  };
  return (
    workspace_id === row.account_user_id &&
    account_user_id === row.account_user_id
  );
}
