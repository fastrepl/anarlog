import { useLiveQuery } from "@/db";

import {
  DEFAULT_SIDEBAR_ITEM_PREFERENCES,
  parseSidebarPreferences,
  type SidebarItemPreferences,
  type SidebarPreferenceRow,
} from "./sidebar-preferences-model";

export * from "./sidebar-preferences-model";

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
