import { useMutation } from "@tanstack/react-query";

import { execute, executeTransaction, useLiveQuery } from "@/db";

import {
  DEFAULT_PREFERENCES,
  parsePreferences,
  PREFERENCE_KEYS,
  serializePreference,
  SHARED_PREFERENCE_KEYS,
  type PreferenceKey,
  type PreferenceRow,
  type Preferences,
} from "./preferences-model";

const keys = PREFERENCE_KEYS.map((key) => `'${key}'`).join(", ");
const sharedKeys = SHARED_PREFERENCE_KEYS.map((key) => `'${key}'`).join(", ");
export const PREFERENCES_SQL = `
SELECT id, value_json, 0 AS source_rank FROM app_settings WHERE id IN (${keys})
UNION ALL
SELECT preferences.id, preferences.value_json, 1 AS source_rank
FROM synced_preferences AS preferences
JOIN app_settings AS binding ON binding.id = 'cloudsync_workspace_binding'
WHERE preferences.id IN (${sharedKeys})
  AND json_type(binding.value_json, '$.workspace_id') = 'text'
  AND preferences.workspace_id = json_extract(binding.value_json, '$.workspace_id')
  AND preferences.workspace_id <> ''
ORDER BY id, source_rank
`;

export function usePreferences(): Preferences {
  const { data } = useLiveQuery<PreferenceRow, Preferences>({
    sql: PREFERENCES_SQL,
    mapRows: parsePreferences,
  });
  return data ?? DEFAULT_PREFERENCES;
}

export async function readPreferences(): Promise<Preferences> {
  return parsePreferences(await execute<PreferenceRow>(PREFERENCES_SQL));
}

export async function setPreference<K extends PreferenceKey>(
  key: K,
  value: Preferences[K],
): Promise<void> {
  const json = serializePreference(key, value);
  const now = new Date().toISOString();
  await executeTransaction([
    {
      sql: `INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at`,
      params: [key, json, now],
    },
    ...((SHARED_PREFERENCE_KEYS as readonly string[]).includes(key)
      ? [
          {
            sql: `INSERT INTO synced_preferences (id, workspace_id, value_json, updated_at)
        SELECT ?, json_extract(value_json, '$.workspace_id'), ?, ? FROM app_settings
        WHERE id = 'cloudsync_workspace_binding'
          AND json_type(value_json, '$.workspace_id') = 'text'
          AND json_extract(value_json, '$.workspace_id') <> ''
        ON CONFLICT(id) DO UPDATE SET workspace_id = excluded.workspace_id,
          value_json = excluded.value_json, updated_at = excluded.updated_at`,
            params: [key, json, now],
          },
        ]
      : []),
  ]);
}

export function usePreferenceMutation<K extends PreferenceKey>(key: K) {
  return useMutation({
    mutationFn: (value: Preferences[K]) => setPreference(key, value),
  });
}
