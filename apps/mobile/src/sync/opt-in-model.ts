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

export function resolveCloudSyncOptIn(rows: CloudSyncOptInRow[]): boolean {
  const row = rows[0];
  if (!row || !row.account_user_id) return false;
  const preference = parseJson(row.preference_json);
  if (typeof preference === "boolean") return preference;
  if (row.connected_account) return true;
  if (row.has_connections) return false;
  if (row.binding_json === null) return true;
  const binding = parseJson(row.binding_json);
  if (typeof binding !== "object" || binding === null) return false;
  const { workspace_id, account_user_id } = binding as {
    workspace_id?: unknown;
    account_user_id?: unknown;
  };
  return (
    typeof workspace_id === "string" &&
    workspace_id.length > 0 &&
    (account_user_id === null || account_user_id === row.account_user_id)
  );
}
