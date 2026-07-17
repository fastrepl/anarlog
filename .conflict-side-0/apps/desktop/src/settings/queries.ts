import { disable, enable } from "@tauri-apps/plugin-autostart";
import { useCallback } from "react";

import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as detectCommands } from "@hypr/plugin-detect";
import { commands as localSttCommands } from "@hypr/plugin-local-stt";
import { commands as trayCommands } from "@hypr/plugin-tray";
import { commands as windowsCommands } from "@hypr/plugin-windows";

import { executeTransaction, liveQueryClient, useLiveQuery } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { normalizeAudioRetention } from "~/services/audio-retention-policy";
import {
  LEGACY_MAIN_VALUES_ID,
  LEGACY_SETTINGS_ID,
} from "~/settings/legacy-snapshots";
import {
  SETTING_DEFINITIONS,
  type SettingKey,
  type SettingValue,
  type SettingValues,
} from "~/settings/schema";
import {
  isConfiguredSttModel,
  isHyprnoteLocalSttModel,
} from "~/stt/capabilities";
import {
  getDefaultSttModel,
  normalizeStoredSttModel,
} from "~/stt/model-selection";

type AppSettingRow = { id: string; value_json: string };

export type StoredSettingValues = {
  values: SettingValues;
  hasValues: Set<SettingKey>;
};

const EMPTY_STORED_SETTINGS: StoredSettingValues = {
  values: {},
  hasValues: new Set(),
};
const JSON_ARRAY_KEYS = new Set<SettingKey>([
  "spoken_languages",
  "personalization_dictionary_terms",
  "ignored_platforms",
  "included_platforms",
]);

export function useStoredSettingValuesQuery() {
  return useLiveQuery<AppSettingRow, StoredSettingValues>({
    sql: `
      SELECT id, value_json
      FROM app_settings
      ORDER BY id
    `,
    mapRows: parseSettingRows,
  });
}

export function useStoredSettingValues(): StoredSettingValues {
  const { data = EMPTY_STORED_SETTINGS } = useStoredSettingValuesQuery();
  return data;
}

export function useSettingsReady(): boolean {
  const { isLoading, error } = useLiveQuery<{ ready: number }, number>({
    sql: `SELECT 1 AS ready FROM app_settings LIMIT 1`,
    mapRows: () => 1,
  });
  return !isLoading && !error;
}

export function useStoredSettingValue<K extends SettingKey>(
  key: K,
): {
  value: SettingValue<K> | undefined;
  hasValue: boolean;
} {
  const { values, hasValues } = useStoredSettingValues();
  return {
    value: values[key] as SettingValue<K> | undefined,
    hasValue: hasValues.has(key),
  };
}

export async function getStoredSettingValues(): Promise<StoredSettingValues> {
  const rows = await liveQueryClient.execute<AppSettingRow>(
    `SELECT id, value_json FROM app_settings ORDER BY id`,
  );
  return parseSettingRows(rows);
}

export async function initializeApplicationSettings(): Promise<void> {
  const stored = await getStoredSettingValues();
  const languageResult = await detectCommands
    .getPreferredLanguages()
    .catch(() => null);
  const updates: SettingValues = {};

  if (languageResult?.status === "ok" && languageResult.data.length > 0) {
    if (!stored.hasValues.has("ai_language")) {
      updates.ai_language = languageResult.data[0];
    }
    if (!stored.hasValues.has("spoken_languages")) {
      updates.spoken_languages = JSON.stringify(languageResult.data);
    }
  }

  if (!stored.values.current_stt_model) {
    const defaultModel = getDefaultSttModel(stored.values.current_stt_provider);
    if (defaultModel) {
      updates.current_stt_model = defaultModel;
    }
  }

  if (Object.keys(updates).length > 0) {
    await setSettingValues(updates);
  }
  const current =
    Object.keys(updates).length > 0 ? await getStoredSettingValues() : stored;
  applySettingSideEffects(current.values);
}

export function setSettingValue<K extends SettingKey>(
  key: K,
  value: SettingValue<K>,
): Promise<void> {
  return setSettingValues({ [key]: value } as SettingValues);
}

export function setSettingValues(values: SettingValues): Promise<void> {
  return enqueueDatabaseWrite("app-settings", () =>
    persistSettingValues(values),
  );
}

export function updateSettingValue<K extends SettingKey>(
  key: K,
  update: (current: SettingValue<K> | undefined) => SettingValue<K>,
): Promise<SettingValue<K>> {
  return enqueueDatabaseWrite("app-settings", async () => {
    const stored = await getStoredSettingValues();
    const definition = SETTING_DEFINITIONS[key];
    const fallback =
      "default" in definition
        ? (definition.default as SettingValue<K>)
        : undefined;
    const current = stored.hasValues.has(key)
      ? (stored.values[key] as unknown as SettingValue<K>)
      : fallback;
    const next = update(current);
    await persistSettingValues({ [key]: next } as SettingValues);
    return next;
  });
}

export function useSetSettingValue<K extends SettingKey>(key: K) {
  return useCallback(
    (value: SettingValue<K>) => {
      void setSettingValue(key, value).catch((error) => {
        console.error(`[settings] failed to update ${key}`, error);
      });
    },
    [key],
  );
}

export function useSetSettingValues() {
  return useCallback((values: SettingValues) => {
    void setSettingValues(values).catch((error) => {
      console.error("[settings] failed to update values", error);
    });
  }, []);
}

export function parseSettingRows(rows: AppSettingRow[]): StoredSettingValues {
  const directRows = new Map(rows.map((row) => [row.id, row.value_json]));
  const legacySettings = parseJsonObject(directRows.get(LEGACY_SETTINGS_ID));
  const legacyMainValues = parseJsonObject(
    directRows.get(LEGACY_MAIN_VALUES_ID),
  );
  const values: SettingValues = {};
  const hasValues = new Set<SettingKey>();

  for (const key of Object.keys(SETTING_DEFINITIONS) as SettingKey[]) {
    const directJson = directRows.get(key);
    const directValue =
      directJson === undefined ? INVALID : parseJsonValue(directJson);
    const normalizedDirect = normalizeSettingValue(key, directValue, true);
    if (normalizedDirect !== INVALID) {
      setParsedSetting(values, key, normalizedDirect);
      hasValues.add(key);
      continue;
    }

    const legacyValue = readLegacySettingValue(
      legacySettings,
      legacyMainValues,
      key,
    );
    const normalizedLegacy = normalizeSettingValue(key, legacyValue, false);
    if (normalizedLegacy !== INVALID) {
      setParsedSetting(values, key, normalizedLegacy);
      hasValues.add(key);
    }
  }

  return { values, hasValues };
}

async function persistSettingValues(values: SettingValues): Promise<void> {
  const now = new Date().toISOString();
  const statements = Object.entries(values).map(([key, value]) => ({
    sql: `
      INSERT INTO app_settings (id, value_json, updated_at)
      VALUES (?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET
        value_json = excluded.value_json,
        updated_at = excluded.updated_at
    `,
    params: [key, JSON.stringify(value), now],
  }));
  if (statements.length > 0) await executeTransaction(statements);
  applySettingSideEffects(values);
}

function readLegacySettingValue(
  settingsDocument: Record<string, unknown>,
  mainValuesDocument: Record<string, unknown>,
  key: SettingKey,
): unknown {
  const definition = SETTING_DEFINITIONS[key];
  let value = getByPath(settingsDocument, definition.path);

  if (value === undefined && key === "ai_language") {
    value = getByPath(settingsDocument, ["general", "ai_language"]);
  } else if (value === undefined && key === "spoken_languages") {
    value = getByPath(settingsDocument, ["general", "spoken_languages"]);
  } else if (key === "audio_retention") {
    value =
      normalizeAudioRetention(value, undefined) ??
      normalizeAudioRetention(
        getByPath(settingsDocument, ["general", "saveAudioAfterMeeting"]),
        undefined,
      ) ??
      normalizeAudioRetention(
        getByPath(settingsDocument, ["general", "save_recordings"]),
        undefined,
      ) ??
      normalizeAudioRetention(mainValuesDocument.audio_retention, undefined) ??
      normalizeAudioRetention(mainValuesDocument.save_recordings, undefined);
  }

  if (value === undefined) value = mainValuesDocument[key];

  if (key === "current_stt_model") {
    value = normalizeStoredSttModel(
      (getByPath(settingsDocument, ["ai", "current_stt_provider"]) ??
        mainValuesDocument.current_stt_provider) as string | undefined,
      value as string | undefined,
    );
  }

  return value;
}

const INVALID = Symbol("invalid-setting-value");

function normalizeSettingValue(
  key: SettingKey,
  value: unknown,
  direct: boolean,
): boolean | number | string | typeof INVALID {
  if (value === INVALID || value === undefined) return INVALID;

  if (key === "audio_retention") {
    return normalizeAudioRetention(value, undefined) ?? INVALID;
  }

  if (JSON_ARRAY_KEYS.has(key)) {
    if (Array.isArray(value)) return JSON.stringify(value);
    if (typeof value !== "string") return INVALID;
    try {
      return Array.isArray(JSON.parse(value)) ? value : INVALID;
    } catch {
      if (!direct && value.includes(",")) {
        return JSON.stringify(
          value
            .split(",")
            .map((entry) => entry.trim())
            .filter(Boolean),
        );
      }
      return INVALID;
    }
  }

  const expectedType = SETTING_DEFINITIONS[key].type;
  if (expectedType === "boolean" && typeof value === "boolean") return value;
  if (expectedType === "number" && typeof value === "number") return value;
  if (expectedType === "string" && typeof value === "string") return value;
  return INVALID;
}

function setParsedSetting<K extends SettingKey>(
  values: SettingValues,
  key: K,
  value: boolean | number | string,
): void {
  (values as Record<string, unknown>)[key] = value as SettingValue<K>;
}

function getByPath(
  document: Record<string, unknown>,
  path: readonly [string, string],
): unknown {
  const section = document[path[0]];
  return section && typeof section === "object"
    ? (section as Record<string, unknown>)[path[1]]
    : undefined;
}

function parseJsonObject(value: string | undefined): Record<string, unknown> {
  const parsed = parseJsonValue(value);
  return parsed && typeof parsed === "object" && !Array.isArray(parsed)
    ? (parsed as Record<string, unknown>)
    : {};
}

function parseJsonValue(value: string | undefined): unknown {
  if (value === undefined) return INVALID;
  try {
    return JSON.parse(value);
  } catch {
    return INVALID;
  }
}

function applySettingSideEffects(values: SettingValues): void {
  if (values.autostart !== undefined) {
    void (values.autostart ? enable() : disable()).catch(console.error);
  }
  if (values.respect_dnd !== undefined) {
    void detectCommands
      .setRespectDoNotDisturb(values.respect_dnd)
      .catch(console.error);
  }
  if (values.ignored_platforms !== undefined) {
    void detectCommands
      .setIgnoredBundleIds(parseStringArray(values.ignored_platforms))
      .catch(console.error);
  }
  if (values.included_platforms !== undefined) {
    void detectCommands
      .setIncludedBundleIds(parseStringArray(values.included_platforms))
      .catch(console.error);
  }
  if (values.mic_active_threshold !== undefined) {
    void detectCommands
      .setMicActiveThreshold(values.mic_active_threshold)
      .catch(console.error);
  }
  if (values.telemetry_consent !== undefined) {
    void analyticsCommands
      .setDisabled(!values.telemetry_consent)
      .catch(console.error);
  }
  if (values.show_app_in_dock !== undefined) {
    void windowsCommands
      .setShowAppInDock(values.show_app_in_dock)
      .catch(console.error);
  }
  if (values.show_tray_icon !== undefined) {
    void trayCommands
      .setTrayIconVisible(values.show_tray_icon)
      .catch(console.error);
  }
  if (
    values.current_stt_provider !== undefined ||
    values.current_stt_model !== undefined
  ) {
    void syncLocalSttServer().catch(console.error);
  }
  if (
    values.spoken_languages !== undefined ||
    values.current_stt_provider !== undefined ||
    values.current_stt_model !== undefined ||
    values.current_llm_provider !== undefined ||
    values.current_llm_model !== undefined
  ) {
    void syncAnalyticsSettingProperties().catch(console.error);
  }
}

async function syncLocalSttServer(): Promise<void> {
  const { values } = await getStoredSettingValues();
  const provider = values.current_stt_provider;
  let model = values.current_stt_model;

  if (
    provider === "hyprnote" &&
    model &&
    !isConfiguredSttModel(provider, model)
  ) {
    model = "";
    await executeTransaction([
      {
        sql: `
          INSERT INTO app_settings (id, value_json, updated_at)
          VALUES ('current_stt_model', ?, ?)
          ON CONFLICT(id) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at
        `,
        params: [JSON.stringify(model), new Date().toISOString()],
      },
    ]);
  }

  if (isHyprnoteLocalSttModel(provider, model)) {
    await localSttCommands.startServer(model);
  } else {
    await localSttCommands.stopServer(null);
  }
}

async function syncAnalyticsSettingProperties(): Promise<void> {
  const { values } = await getStoredSettingValues();
  await analyticsCommands.setProperties({
    set: {
      spoken_languages: parseStringArray(values.spoken_languages ?? "[]"),
      current_stt_provider: values.current_stt_provider ?? null,
      current_stt_model: values.current_stt_model ?? null,
      current_llm_provider: values.current_llm_provider ?? null,
      current_llm_model: values.current_llm_model ?? null,
    },
  });
}

function parseStringArray(value: string): string[] {
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((entry): entry is string => typeof entry === "string")
      : [];
  } catch {
    return [];
  }
}
