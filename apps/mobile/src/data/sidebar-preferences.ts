import { useLiveQuery, executeTransaction } from "@/db";

import {
  DEFAULT_SIDEBAR_ITEM_PREFERENCES,
  parseSidebarPreferences,
  type SidebarItemPreferences,
  type SidebarPreferenceRow,
} from "./sidebar-preferences-model";

export * from "./sidebar-preferences-model";

export type SidebarPreferenceKey = "sidebar_show_folder" | "sidebar_show_tags";

const SIDEBAR_PREFERENCES_SQL = `
SELECT id, value_json, 0 AS source_rank
FROM app_settings
WHERE id IN ('sidebar_show_folder', 'sidebar_show_tags')
UNION ALL
SELECT preferences.id, preferences.value_json, 1 AS source_rank
FROM synced_preferences AS preferences
JOIN app_settings AS binding ON binding.id = 'cloudsync_workspace_binding'
WHERE preferences.id IN ('sidebar_show_folder', 'sidebar_show_tags')
  AND json_type(binding.value_json, '$.workspace_id') = 'text'
  AND preferences.workspace_id = json_extract(binding.value_json, '$.workspace_id')
  AND preferences.workspace_id <> ''
ORDER BY id, source_rank
`;

export function useSidebarItemPreferences(): SidebarItemPreferences {
  const { data } = useLiveQuery<SidebarPreferenceRow, SidebarItemPreferences>({
    sql: SIDEBAR_PREFERENCES_SQL,
    mapRows: parseSidebarPreferences,
  });
  return data ?? DEFAULT_SIDEBAR_ITEM_PREFERENCES;
}

export async function setSidebarItemPreference(
  key: SidebarPreferenceKey,
  value: boolean,
): Promise<void> {
  const valueJson = JSON.stringify(value);
  const updatedAt = new Date().toISOString();
  await executeTransaction([
    {
      sql: `
        INSERT INTO app_settings (id, value_json, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          value_json = excluded.value_json,
          updated_at = excluded.updated_at
      `,
      params: [key, valueJson, updatedAt],
    },
    {
      sql: `
        INSERT INTO synced_preferences (id, workspace_id, value_json, updated_at)
        SELECT
          ?,
          json_extract(value_json, '$.workspace_id'),
          ?,
          ?
        FROM app_settings
        WHERE id = 'cloudsync_workspace_binding'
          AND COALESCE(json_extract(value_json, '$.workspace_id'), '') <> ''
        ON CONFLICT(id) DO UPDATE SET
          workspace_id = excluded.workspace_id,
          value_json = excluded.value_json,
          updated_at = excluded.updated_at
      `,
      params: [key, valueJson, updatedAt],
    },
  ]);
}
