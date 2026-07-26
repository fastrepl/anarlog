import { commands as settingsCommands } from "@hypr/plugin-settings";

import { executeTransaction } from "~/db";
import { commands as desktopCommands } from "~/types/tauri.gen";

export const LEGACY_SETTINGS_ID = "legacy_settings_document";
export const LEGACY_MAIN_VALUES_ID = "legacy_main_values_document";

export async function refreshLegacySettingsSnapshots(): Promise<void> {
  const [settingsResult, valuesResult] = await Promise.allSettled([
    settingsCommands.load(),
    desktopCommands.getTinybaseValues(),
  ]);
  const snapshots: Array<{ id: string; valueJson: string }> = [];

  if (
    settingsResult.status === "fulfilled" &&
    settingsResult.value.status === "ok" &&
    isNonEmptyJsonObject(settingsResult.value.data)
  ) {
    snapshots.push({
      id: LEGACY_SETTINGS_ID,
      valueJson: JSON.stringify(settingsResult.value.data),
    });
  }

  if (
    valuesResult.status === "fulfilled" &&
    valuesResult.value.status === "ok" &&
    valuesResult.value.data
  ) {
    const parsed = parseJsonObject(valuesResult.value.data);
    if (parsed) {
      snapshots.push({
        id: LEGACY_MAIN_VALUES_ID,
        valueJson: JSON.stringify(parsed),
      });
    }
  }

  if (snapshots.length === 0) return;
  const now = new Date().toISOString();
  await executeTransaction(
    snapshots.map((snapshot) => ({
      sql: `
        INSERT INTO app_settings (id, value_json, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
          value_json = excluded.value_json,
          updated_at = excluded.updated_at
        WHERE app_settings.value_json IS NOT excluded.value_json
      `,
      params: [snapshot.id, snapshot.valueJson, now],
    })),
  );
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function isNonEmptyJsonObject(
  value: unknown,
): value is Record<string, unknown> {
  return isJsonObject(value) && Object.keys(value).length > 0;
}

function parseJsonObject(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value);
    return isJsonObject(parsed) ? parsed : null;
  } catch {
    return null;
  }
}
