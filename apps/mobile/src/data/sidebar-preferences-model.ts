export type SidebarItemPreferences = {
  showFolder: boolean;
  showTags: boolean;
};

export type SidebarPreferenceRow = {
  id: string;
  value_json: string;
  source_rank: number;
};

export const DEFAULT_SIDEBAR_ITEM_PREFERENCES: SidebarItemPreferences = {
  showFolder: true,
  showTags: false,
};

export function parseSidebarPreferences(
  rows: SidebarPreferenceRow[],
): SidebarItemPreferences {
  const preferences = { ...DEFAULT_SIDEBAR_ITEM_PREFERENCES };
  for (const row of rows) {
    let value: unknown;
    try {
      value = JSON.parse(row.value_json);
    } catch {
      continue;
    }
    if (typeof value !== "boolean") continue;
    if (row.id === "sidebar_show_folder") preferences.showFolder = value;
    if (row.id === "sidebar_show_tags") preferences.showTags = value;
  }
  return preferences;
}
